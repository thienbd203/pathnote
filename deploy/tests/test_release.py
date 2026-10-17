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
        self.assertEqual(set(self.jobs['deploy-backend']['needs']), {'release', 'build-backend', 'build-frontend', 'browser-contract'})
        self.assertEqual(set(self.jobs['deploy-frontend']['needs']), {'release', 'build-frontend', 'deploy-backend'})
        for name in ('deploy-backend', 'deploy-frontend'):
            job = self.jobs[name]
            self.assertNotIn('if', job)  # Preserve default success() gating; never use always().
            self.assertEqual(job['environment'], 'production')
        # Frontend preparation can run while Rust checks/release compilation run.
        self.assertEqual(self.jobs['build-frontend']['needs'], 'release')
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
        browser = self.jobs['browser-contract']
        self.assertNotIn('if', browser)  # Failed upstream checks must skip this gate.
        self.assertEqual(set(browser['needs']), {'release', 'check'})
        checkouts = [step['with'] for step in browser['steps'] if step.get('uses', '').startswith('actions/checkout@')]
        self.assertEqual(checkouts[1]['repository'], checkout['repository'])
        self.assertEqual(checkouts[1]['ref'], checkout['ref'])
        self.assertFalse(checkouts[1]['persist-credentials'])
        commands = '\n'.join(step.get('run', '') for step in browser['steps'])
        self.assertIn('npm run e2e:build', commands)
        self.assertIn('npm run e2e:integration', commands)
        self.assertNotIn('continue-on-error', json.dumps(browser))
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
        for name in ('check', 'build-backend', 'build-frontend', 'browser-contract'):
            self.assertNotIn('SSH_PRIVATE_KEY', json.dumps(self.jobs[name]))
        guard = self.jobs['release']['if']
        self.assertIn("github.event_name != 'pull_request'", guard)
        self.assertIn("github.ref == 'refs/heads/master'", guard)

    def test_standalone_frontend_workflow_has_no_production_activation(self):
        frontend = ROOT.parent / 'pathnote-web/.github/workflows/deploy.yml'
        if not frontend.exists():
            self.skipTest('Adjacent frontend checkout only available in the local paired workspace')
        workflow = self.load(frontend)
        for job in workflow['jobs'].values():
            self.assertNotIn('environment', job)
            for step in job['steps']:
                self.assertNotIn('SSH_PRIVATE_KEY', json.dumps(step))
                self.assertNotIn('rsync ', step.get('run', ''))
                self.assertNotIn('bash -s --', step.get('run', ''))
