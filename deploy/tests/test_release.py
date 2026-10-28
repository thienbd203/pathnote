import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('release', ROOT / 'deploy/release.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
SHA = 'a' * 40
FRONTEND_SHA = 'b' * 40


class ReleaseManifestTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.manifest = self.root / 'release.json'
        self.write({'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': FRONTEND_SHA, 'breaking_migration': True, 'collaboration_sha': SHA})

    def write(self, data):
        self.manifest.write_text(json.dumps(data))

    def test_pair_uses_exact_revisions_and_supported_release_identifier(self):
        result = release.resolve(self.manifest, SHA, '42', '1')
        self.assertEqual(result['frontend_sha'], FRONTEND_SHA)
        self.assertEqual(result['backend_sha'], SHA)
        self.assertEqual(result['breaking_migration'], 'true')
        self.assertEqual(result['collaboration_sha'], SHA)
        self.assertRegex(result['release_id'], r'^\d{14}-42-1$')

    def test_moving_refs_wrong_repositories_and_unexpected_fields_are_rejected(self):
        for data in (
            {'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': 'master'},
            {'frontend_repository': 'other/private-repository', 'frontend_sha': FRONTEND_SHA},
            {'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': SHA + '\nrelease_id=bad'},
            {'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': True},
            {'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': FRONTEND_SHA, 'backend_sha': SHA},
            [],
        ):
            with self.subTest(data=data):
                self.write(data)
                with self.assertRaises(ValueError):
                    release.resolve(self.manifest, SHA, '42', '1')

    def test_breaking_flag_requires_an_explicit_json_boolean(self):
        for flag in ('true', 'false', 0, 1, None):
            self.write({'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': FRONTEND_SHA, 'breaking_migration': flag, 'collaboration_sha': SHA})
            with self.assertRaises(ValueError):
                release.resolve(self.manifest, SHA, '42', '1')
        self.write({'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': FRONTEND_SHA, 'breaking_migration': False, 'collaboration_sha': SHA})
        self.assertEqual(release.resolve(self.manifest, SHA, '42', '1')['breaking_migration'], 'false')

    def test_collaboration_revision_cannot_be_a_moving_ref(self):
        self.write({'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': FRONTEND_SHA, 'breaking_migration': True, 'collaboration_sha': 'master'})
        with self.assertRaises(ValueError):
            release.resolve(self.manifest, SHA, '42', '1')

    def test_invalid_backend_revision_or_run_id_cannot_enter_action_outputs(self):
        for sha, run_id, attempt in [('master', '42', '1'), (SHA, '42\nbad', '1'), (SHA, '42', '-1')]:
            with self.subTest(sha=sha, run_id=run_id, attempt=attempt), self.assertRaises(ValueError):
                release.resolve(self.manifest, sha, run_id, attempt)

    def test_cli_writes_the_reviewed_pair_and_fails_before_publishing_invalid_outputs(self):
        output, summary = self.root / 'outputs', self.root / 'summary'
        env = {
            'GITHUB_SHA': SHA, 'GITHUB_RUN_ID': '42', 'GITHUB_RUN_ATTEMPT': '1',
            'GITHUB_OUTPUT': str(output), 'GITHUB_STEP_SUMMARY': str(summary),
        }
        with patch.dict(os.environ, env), patch.object(release.sys, 'argv', ['release.py', str(self.manifest)]):
            release.main()
        self.assertIn(f'frontend_sha={FRONTEND_SHA}\n', output.read_text())
        self.assertIn(f'backend_sha={SHA}\n', output.read_text())
        old = output.read_text()
        self.write({'frontend_repository': release.FRONTEND_REPOSITORY, 'frontend_sha': 'master'})
        with patch.dict(os.environ, env), patch.object(release.sys, 'argv', ['release.py', str(self.manifest)]), self.assertRaises(ValueError):
            release.main()
        self.assertEqual(output.read_text(), old)


@unittest.skipUnless(shutil.which('ruby'), 'Ruby standard YAML parser required for workflow topology tests')
class ReleaseWorkflowTests(unittest.TestCase):
    @staticmethod
    def load(path):
        # Use the standard YAML parser, never a handwritten approximation.
        return json.loads(subprocess.check_output([
            'ruby', '-rjson', '-ryaml', '-e', 'puts JSON.generate(YAML.load_file(ARGV.fetch(0)))', str(path),
        ], text=True))

    def setUp(self):
        self.workflow = self.load(ROOT / '.github/workflows/deploy.yml')
        self.jobs = self.workflow['jobs']

    def test_no_deployment_can_start_before_both_builds_succeed(self):
        self.assertEqual(set(self.jobs['deploy-backend']['needs']), {'release', 'check', 'build-backend', 'build-frontend', 'build-collaboration'})
        self.assertEqual(set(self.jobs['deploy-frontend']['needs']), {'release', 'build-frontend', 'deploy-backend', 'deploy-collaboration'})
        for name in ('deploy-backend', 'deploy-collaboration', 'deploy-frontend'):
            job = self.jobs[name]
            self.assertNotIn('if', job)  # Preserve default success() gating; never use always().
            self.assertEqual(job['environment'], 'production')
        # Frontend preparation can run while Rust checks/release compilation run.
        self.assertEqual(self.jobs['build-frontend']['needs'], 'release')
        # Preparation runs in parallel; activation still waits for every gate.
        self.assertEqual(self.jobs['build-backend']['needs'], 'release')
        self.assertFalse(self.workflow['concurrency']['cancel-in-progress'])
        backend = self.jobs['deploy-backend']
        activation = next(step for step in backend['steps'] if step.get('name') == 'Upload and activate release')
        self.assertEqual(activation['env']['BREAKING_MIGRATION'], '${{ needs.release.outputs.breaking_migration }}')
        self.assertIn('\"$BREAKING_MIGRATION\"', activation['run'])

    def test_frontend_checkout_and_activation_use_the_same_pinned_revision(self):
        checkout = self.jobs['build-frontend']['steps'][0]['with']
        self.assertEqual(checkout['ref'], '${{ needs.release.outputs.frontend_sha }}')
        self.assertFalse(checkout['persist-credentials'])
        self.assertEqual(self.jobs['deploy-frontend']['env']['FRONTEND_SHA'], checkout['ref'])
        self.assertNotIn('browser-contract', self.jobs)
        self.assertNotIn('playwright', json.dumps(self.workflow).lower())
        self.assertNotIn('npm run e2e', json.dumps(self.workflow))
        artifacts = [step['with'] for step in self.jobs['deploy-frontend']['steps'] if step.get('uses', '').startswith('actions/download-artifact@')]
        self.assertEqual(artifacts, [{'name': 'pathnote-web-release', 'path': 'web-release'}])

    def test_workflow_shell_steps_are_valid_bash_without_executing_them(self):
        for job_name, job in self.jobs.items():
            for step in job['steps']:
                if 'run' in step:
                    with self.subTest(job=job_name, step=step.get('name', step.get('id'))):
                        result = subprocess.run(['bash', '-n'], input=step['run'], text=True, capture_output=True)
                        self.assertEqual(result.returncode, 0, result.stderr)

    def test_backend_is_version_checked_before_frontend_activation_and_builds_have_no_ssh(self):
        deploy = self.jobs['deploy-frontend']
        self.assertEqual(deploy['env']['BACKEND_SHA'], '${{ needs.release.outputs.backend_sha }}')
        activation = next(step['run'] for step in deploy['steps'] if step.get('name') == 'Upload and activate verified frontend')
        self.assertLess(activation.index('python3 deploy/healthcheck.py'), activation.index('< web-release/deploy.sh'))
        self.assertLess(activation.index('\"$COLLABORATION_SHA\" collaboration'), activation.index('< web-release/deploy.sh'))
        self.assertEqual(deploy['env']['COLLABORATION_SHA'], '${{ needs.release.outputs.collaboration_sha }}')
        for name in ('check', 'build-backend', 'build-frontend', 'build-collaboration'):
            self.assertNotIn('SSH_PRIVATE_KEY', json.dumps(self.jobs[name]))
        guard = self.jobs['release']['if']
        self.assertIn("github.event_name != 'pull_request'", guard)
        self.assertIn("github.ref == 'refs/heads/master'", guard)

    def test_blue_green_is_opt_in_and_transferred_as_a_separate_argument(self):
        job = self.jobs['deploy-backend']
        self.assertEqual(job['env']['DEPLOY_STRATEGY'], "${{ vars.DEPLOY_STRATEGY || 'in-place' }}")
        activation = next(step for step in job['steps'] if step.get('name') == 'Upload and activate release')
        self.assertIn('"$DEPLOY_STRATEGY"', activation['run'])
        build = '\n'.join(step.get('run', '') for step in self.jobs['build-backend']['steps'])
        self.assertIn('deploy/blue-green.py artifact/blue-green.py', build)
        self.assertIn('healthcheck.py blue-green.py deploy-version.txt', build)

    def test_standalone_frontend_workflow_has_no_production_activation(self):
        frontend = ROOT.parent / 'pathnote-web/.github/workflows/deploy.yml'
        if not frontend.exists():
            self.skipTest('Adjacent frontend checkout only available in the local paired workspace')
        workflow = self.load(frontend)
        self.assertNotIn('e2e', workflow['jobs'])
        self.assertNotIn('playwright', json.dumps(workflow).lower())
        for job in workflow['jobs'].values():
            self.assertNotIn('environment', job)
            for step in job['steps']:
                self.assertNotIn('SSH_PRIVATE_KEY', json.dumps(step))
                self.assertNotIn('rsync ', step.get('run', ''))
                self.assertNotIn('bash -s --', step.get('run', ''))


    def test_collaboration_uses_the_pinned_artifact_after_healthy_backend(self):
        build = self.jobs['build-collaboration']
        self.assertEqual(build['needs'], 'release')
        checkout = build['steps'][0]['with']
        self.assertEqual(checkout['repository'], 'thienbd203/pathnote-collaboration')
        self.assertEqual(checkout['ref'], '${{ needs.release.outputs.collaboration_sha }}')
        self.assertFalse(checkout['persist-credentials'])
        commands = '\n'.join(step.get('run', '') for step in build['steps'])
        self.assertIn('npm test', commands)
        self.assertIn('sha256sum deploy.sh', commands)
        # Missing npm dotfiles must fail the checksum, not be silently omitted
        # from the artifact while still present in SHA256SUMS.
        artifact = next(step['with'] for step in build['steps'] if step.get('uses', '').startswith('actions/upload-artifact@'))
        self.assertTrue(artifact['include-hidden-files'])
        self.assertEqual(artifact['path'], 'collaboration-release/')
        self.assertIn('cp -a server node_modules package.json package-lock.json collaboration-release/app/', commands)
        job = self.jobs['deploy-collaboration']
        self.assertEqual(set(job['needs']), {'release', 'build-collaboration', 'deploy-backend'})
        self.assertEqual(job['env']['COLLABORATION_SHA'], checkout['ref'])
        steps = {step.get('name'): step for step in job['steps']}
        guard = steps['Verify artifact and healthy Rust before collaboration']
        self.assertIn('sha256sum --check --strict SHA256SUMS', guard['run'])
        self.assertLess(guard['run'].index('"$BACKEND_SHA"'), guard['run'].index('"$COLLABORATION_SHA" collaboration'))
        for name in ('Configure pinned collaboration SSH connection', 'Upload and activate verified collaboration'):
            self.assertEqual(steps[name]['if'], "steps.running.outputs.current != 'true'")
        activation = steps['Upload and activate verified collaboration']['run']
        self.assertLess(activation.index('"$BACKEND_SHA"'), activation.index('< collaboration-release/deploy.sh'))
        self.assertIn('"$COLLABORATION_SHA"', activation)
        self.assertNotIn('if', steps['Confirm collaboration version after activation or reuse'])

    def test_standalone_collaboration_workflow_cannot_activate_production(self):
        path = ROOT.parent / 'pathnote-collaboration/.github/workflows/deploy.yml'
        if not path.exists():
            self.skipTest('Adjacent collaboration checkout only available locally')
        workflow = self.load(path)
        for job in workflow['jobs'].values():
            self.assertNotIn('environment', job)
            for step in job['steps']:
                self.assertNotIn('SSH_PRIVATE_KEY', json.dumps(step))
                self.assertNotIn('rsync ', step.get('run', ''))
                self.assertNotIn('bash -s --', step.get('run', ''))
