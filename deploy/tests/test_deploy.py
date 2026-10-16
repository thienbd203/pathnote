"""Exercise release activation/rollback in an isolated Linux /opt directory.

No real service, user or database is modified. CI invokes these as root solely
so the fixture can use the same restricted /opt path shape as deployment.
"""
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import unittest
import uuid

SCRIPT = Path(__file__).resolve().parents[2] / '.github/deploy.sh'
SHA = 'a' * 40
NEW = '20261014000000-42-1'
OLD = '20261013000000-41-1'


@unittest.skipUnless(platform.system() == 'Linux' and os.geteuid() == 0, 'Linux root fixture required')
class DeployTests(unittest.TestCase):
    def setUp(self):
        self.root = Path('/opt/pathnote-rust-test-' + uuid.uuid4().hex)
        self.root.mkdir()
        self.addCleanup(shutil.rmtree, self.root)
        (self.root / 'releases').mkdir()
        shared = self.root / 'shared'
        (shared / 'keys').mkdir(parents=True)
        (shared / 'media').mkdir()
        (shared / '.env').write_text('DATABASE_URL=test-only\n')
        (shared / 'keys/access-token.pem').write_text('test-only-key')
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.log = self.root / 'calls'
        self.mock('getent', 'exit 0')
        self.mock('chgrp', 'exit 0')
        self.mock('uname', 'echo x86_64')
        self.mock('sleep', 'exit 0')
        self.mock('pg_dump', 'exit 0')
        self.mock('systemctl', 'echo "$* $(readlink "$FIXTURE_ROOT/current")" >> "$FIXTURE_ROOT/calls"; exit 0')
        self.mock('sudo', 'while [[ "$1" == -* ]]; do if [[ "$1" == -u ]]; then shift 2; else shift; fi; done; if [[ "$1" == /usr/local/sbin/pathnote-db-backup ]]; then echo backup >> "$FIXTURE_ROOT/calls"; exit "${BACKUP_STATUS:-0}"; fi; exec "$@"')
        self.release(OLD)
        self.release(NEW)
        (self.root / 'current').symlink_to(self.root / 'releases' / OLD)
        self.env = dict(os.environ, PATH=str(self.bin) + ':' + os.environ['PATH'], FIXTURE_ROOT=str(self.root))

    def mock(self, name, body):
        file = self.bin / name
        file.write_text('#!/bin/bash\n' + body + '\n')
        file.chmod(0o755)

    def release(self, name, migration_fail=False, health_fail=False, target='x86_64-unknown-linux-musl'):
        path = self.root / 'releases' / name
        path.mkdir(exist_ok=True)
        (path / 'server').write_text('test server')
        (path / 'migration').write_text('#!/bin/bash\necho migration >> "$FIXTURE_ROOT/calls"\nexit ' + ('1' if migration_fail else '0') + '\n')
        (path / 'healthcheck.py').write_text('import sys\nsys.exit(' + ('1' if health_fail else '0') + ')\n')
        (path / 'deploy-version.txt').write_text(SHA + '\n')
        (path / 'build-target.txt').write_text(target + '\n')
        self.manifest(path)

    def manifest(self, path):
        names = ['server', 'migration', 'healthcheck.py', 'deploy-version.txt', 'build-target.txt']
        (path / 'SHA256SUMS').write_text(''.join(hashlib.sha256((path / name).read_bytes()).hexdigest() + '  ' + name + '\n' for name in names))

    def run_deploy(self):
        return subprocess.run(['bash', str(SCRIPT), str(self.root), NEW, 'https://example.com/health', SHA], env=self.env, capture_output=True, text=True)

    def assert_current(self, release):
        self.assertEqual((self.root / 'current').resolve(), self.root / 'releases' / release)

    def test_success_activates_and_preserves_shared_data(self):
        result = self.run_deploy()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_current(NEW)
        self.assertEqual((self.root / 'shared/.env').read_text(), 'DATABASE_URL=test-only\n')
        self.assertTrue((self.root / 'releases' / NEW / '.env').is_symlink())
        self.assertIn('restart pathnote-rust.service', self.log.read_text())
        self.assertLess(self.log.read_text().index('backup'), self.log.read_text().index('migration'))

    def test_backup_failure_never_runs_migration(self):
        self.env['BACKUP_STATUS'] = '1'
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assert_current(OLD)
        self.assertNotIn('migration', self.log.read_text())

    def test_migration_failure_never_switches_release(self):
        self.release(NEW, migration_fail=True)
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assert_current(OLD)
        self.assertNotIn('restart', self.log.read_text())

    def test_failed_health_restores_previous_code(self):
        self.release(NEW, health_fail=True)
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assert_current(OLD)
        self.assertEqual(self.log.read_text().count('restart pathnote-rust.service'), 2)

    def test_failed_first_deployment_stops_service(self):
        (self.root / 'current').unlink()
        self.release(NEW, health_fail=True)
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assertFalse((self.root / 'current').is_symlink())
        self.assertIn('stop pathnote-rust.service', self.log.read_text())

    def test_corrupted_artifact_never_runs_migration(self):
        (self.root / 'releases' / NEW / 'server').write_text('corrupted')
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assert_current(OLD)
        self.assertNotIn('migration', self.log.read_text())

    def test_wrong_architecture_never_runs_migration(self):
        self.release(NEW, target='aarch64-unknown-linux-musl')
        self.assertNotEqual(self.run_deploy().returncode, 0)
        self.assert_current(OLD)
        self.assertNotIn('migration', self.log.read_text())

    def test_pruning_keeps_previous_and_unrelated_directories(self):
        for name in ('20261012000000-40-1', '20261011000000-39-1', 'manual-backup'):
            (self.root / 'releases' / name).mkdir()
        result = self.run_deploy()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / 'releases' / OLD).exists())
        self.assertTrue((self.root / 'releases/20261012000000-40-1').exists())
        self.assertFalse((self.root / 'releases/20261011000000-39-1').exists())
        self.assertTrue((self.root / 'releases/manual-backup').exists())
