import importlib.util
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

DEPLOY = Path(__file__).parents[1]
sys.path.insert(0, str(DEPLOY))
SPEC = importlib.util.spec_from_file_location('blue_green', DEPLOY / 'blue-green.py')
bg = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bg)
OLD = '20261019000000-1-1'
NEW = '20261020000000-2-1'
OLD_SHA = 'a' * 40
NEW_SHA = 'b' * 40
URL = 'https://example.com/health'
DRAINED = bg.drained


class BlueGreenTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        (self.root / 'slots').mkdir()
        self.previous = self.root / 'releases' / OLD
        self.candidate = self.root / 'releases' / NEW
        for path, sha in ((self.previous, OLD_SHA), (self.candidate, NEW_SHA)):
            path.mkdir(parents=True)
            (path / 'deploy-version.txt').write_text(sha)
        bg.link(self.previous, self.root / 'slots/blue')
        bg.link(self.previous, self.root / 'slots/green')
        bg.link(self.previous, self.root / 'current')
        self.active = 'blue'
        self.calls = []
        self.route_failure = False
        self.rollback_failure = False

        def route(slot=None, sha=None):
            if slot is None:
                return self.active
            self.calls.append(('route', slot))
            if self.rollback_failure and slot == 'blue':
                raise RuntimeError('Rollback failed')
            self.active = slot
            if self.route_failure and slot == 'green':
                raise RuntimeError('Uncertain reload')

        def run(*args, **kwargs):
            self.calls.append(('run', args))
            return subprocess.CompletedProcess(args, 0, stdout='')

        self.run = patch.object(bg, 'run', side_effect=run).start()
        self.health = patch.object(bg, 'health').start()
        self.drain = patch.object(bg, 'drained', return_value=True).start()
        patch.object(bg, 'route', side_effect=route).start()
        self.addCleanup(patch.stopall)

    def activate(self):
        bg.activate(self.root, self.candidate, URL, NEW_SHA)

    def stopped(self, slot):
        return ('run', ('sudo', '-n', 'systemctl', 'stop', f'pathnote-rust@{slot}.service')) in self.calls

    def test_success_starts_new_then_switches_then_stops_old_and_preserves_slots(self):
        self.activate()
        self.assertEqual(self.active, 'green')
        self.assertEqual((self.root / 'current').resolve(), self.candidate)
        start = ('run', ('sudo', '-n', 'systemctl', 'restart', 'pathnote-rust@green.service'))
        stop = ('run', ('sudo', '-n', 'systemctl', 'stop', 'pathnote-rust@blue.service'))
        self.assertLess(self.calls.index(start), self.calls.index(('route', 'green')))
        self.assertLess(self.calls.index(('route', 'green')), self.calls.index(stop))
        self.assertEqual((self.root / 'slots/blue').resolve(), self.previous)
        self.assertTrue(self.previous.exists())
        self.health.assert_any_call('http://127.0.0.1:3001/health', NEW_SHA)
        self.health.assert_any_call(URL + '?release=' + NEW, NEW_SHA)

    def test_next_release_alternates_back_to_blue(self):
        self.activate()
        third = self.root / 'releases/20261021000000-3-1'
        third.mkdir()
        (third / 'deploy-version.txt').write_text('c' * 40)
        bg.activate(self.root, third, URL, 'c' * 40)
        self.assertEqual(self.active, 'blue')
        self.assertEqual((self.root / 'current').resolve(), third)

    def test_new_slot_health_failure_never_switches_or_stops_active(self):
        def unhealthy(url, sha):
            if url == 'http://127.0.0.1:3001/health':
                raise ValueError('new unhealthy')
        self.health.side_effect = unhealthy
        with self.assertRaises(ValueError):
            self.activate()
        self.assertEqual(self.active, 'blue')
        self.assertFalse(self.stopped('blue'))
        self.assertEqual((self.root / 'slots/green').resolve(), self.previous)
        self.assertEqual((self.root / 'current').resolve(), self.previous)

    def test_public_health_failure_routes_back_before_stopping_failed_target(self):
        def unhealthy(url, sha):
            if '?release=' in url:
                raise ValueError('public health')
        self.health.side_effect = unhealthy
        with self.assertRaises(ValueError):
            self.activate()
        self.assertEqual(self.active, 'blue')
        self.assertEqual((self.root / 'current').resolve(), self.previous)
        self.assertFalse(self.stopped('blue'))
        stop = ('run', ('sudo', '-n', 'systemctl', 'stop', 'pathnote-rust@green.service'))
        # The first stop prepares inactive slot; the last retires failed target.
        self.assertLess(self.calls.index(('route', 'blue')), len(self.calls) - 1 - self.calls[::-1].index(stop))

    def test_uncertain_reload_also_rolls_back(self):
        self.route_failure = True
        with self.assertRaises(RuntimeError):
            self.activate()
        self.assertEqual(self.active, 'blue')
        self.assertEqual((self.root / 'current').resolve(), self.previous)

    def test_failed_rollback_keeps_both_services_available_for_operator(self):
        self.route_failure = True
        self.rollback_failure = True
        with self.assertRaises(RuntimeError):
            self.activate()
        self.assertFalse(self.stopped('blue'))
        self.assertEqual(sum(1 for kind, args in self.calls if kind == 'run' and args[2:4] == ('systemctl', 'stop')), 1)

    def test_old_drain_timeout_keeps_old_running_and_new_active(self):
        self.drain.side_effect = [True, False]
        self.activate()
        self.assertFalse(self.stopped('blue'))
        self.assertEqual(self.active, 'green')
        self.assertTrue(self.previous.exists())

    def test_backup_failure_cannot_start_candidate_or_switch(self):
        original = self.run.side_effect
        def fail(*args, **kwargs):
            if '/usr/local/sbin/pathnote-db-backup' in args:
                raise subprocess.CalledProcessError(1, args)
            return original(*args, **kwargs)
        self.run.side_effect = fail
        with self.assertRaises(subprocess.CalledProcessError):
            self.activate()
        self.assertEqual(self.active, 'blue')
        self.assertFalse(any('migration' in str(args) for _, args in self.calls))

    def test_migration_failure_keeps_active_code_and_route(self):
        original = self.run.side_effect
        def fail(*args, **kwargs):
            if str(self.candidate / 'migration') in args:
                raise subprocess.CalledProcessError(1, args)
            return original(*args, **kwargs)
        self.run.side_effect = fail
        with self.assertRaises(subprocess.CalledProcessError):
            self.activate()
        self.assertEqual(self.active, 'blue')
        self.assertFalse(self.stopped('blue'))

    def test_disagreeing_current_link_is_rejected_before_backup(self):
        bg.link(self.candidate, self.root / 'current')
        with self.assertRaises(ValueError):
            self.activate()
        self.run.assert_not_called()

    def test_breaking_migration_and_invalid_inputs_are_rejected(self):
        bg.validate(Path('/opt/pathnote-rust'), NEW, URL, NEW_SHA, 'false')
        for root, release, url, sha, flag in (
            ('/opt/pathnote-rust', NEW, URL, NEW_SHA, 'true'),
            ('/tmp/anything', NEW, URL, NEW_SHA, 'false'),
            ('/opt/pathnote-rust', '../x', URL, NEW_SHA, 'false'),
            ('/opt/pathnote-rust', NEW, 'http://example.com/health', NEW_SHA, 'false'),
            ('/opt/pathnote-rust', NEW, URL, 'master', 'false'),
        ):
            with self.assertRaises(ValueError):
                bg.validate(Path(root), release, url, sha, flag)

    def test_busy_inactive_slot_is_not_reused_and_no_migration_runs(self):
        self.drain.return_value = False
        with self.assertRaises(RuntimeError):
            self.activate()
        self.assertFalse(any('migration' in str(args) for _, args in self.calls))
        self.assertEqual(self.active, 'blue')

    def test_wrong_internal_route_is_detected_before_backup(self):
        def wrong_internal(url, sha):
            if ':3010/' in url:
                raise ValueError('Wrong internal revision')
        self.health.side_effect = wrong_internal
        with self.assertRaises(ValueError):
            self.activate()
        self.assertFalse(any('pathnote-db-backup' in str(args) for _, args in self.calls))

    def test_checksum_manifest_must_cover_all_helpers(self):
        for name in bg.FILES:
            (self.candidate / name).write_text('fixture')
        (self.candidate / 'SHA256SUMS').write_text('')
        with self.assertRaises(ValueError):
            bg.prepare(self.root, NEW, NEW_SHA)
        self.assertFalse((self.candidate / '.env').exists())

    def test_wrong_version_is_rejected_before_permission_or_env_changes(self):
        for name in bg.FILES:
            (self.candidate / name).write_text('fixture')
        (self.candidate / 'SHA256SUMS').write_text(''.join(
            hashlib.sha256((self.candidate / name).read_bytes()).hexdigest() + '  ' + name + '\n'
            for name in bg.FILES))
        with self.assertRaises(ValueError):
            bg.prepare(self.root, NEW, NEW_SHA)
        self.assertFalse((self.candidate / '.env').exists())

    def test_connection_query_errors_are_not_treated_as_drained(self):
        with patch.object(bg.time, 'sleep'), patch.object(bg, 'run', side_effect=OSError('ss failed')):
            with self.assertRaises(OSError):
                DRAINED(3000)
