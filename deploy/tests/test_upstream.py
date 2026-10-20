import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('upstream', Path(__file__).parents[1] / 'upstream.py')
upstream = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(upstream)
SHA = 'a' * 40


class UpstreamTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.path = Path(temp.name) / 'upstream.conf'
        self.path.write_text(upstream.config_for('blue'))

    def test_only_exact_managed_config_and_two_slots_are_accepted(self):
        self.assertEqual(upstream.active(self.path), 'blue')
        for invalid in ('other', 'green; reboot'):
            with self.assertRaises(ValueError):
                upstream.switch(invalid, SHA, self.path)
        self.path.write_text('upstream another_app { server 127.0.0.1:3000; }')
        with self.assertRaises(ValueError):
            upstream.active(self.path)

    def test_health_failure_cannot_change_routing(self):
        with patch.object(upstream, 'healthy', side_effect=ValueError('unhealthy')), \
             patch.object(upstream.subprocess, 'run') as run, self.assertRaises(ValueError):
            upstream.switch('green', SHA, self.path)
        run.assert_not_called()
        self.assertEqual(upstream.active(self.path), 'blue')

    def test_success_checks_config_before_reloading(self):
        with patch.object(upstream, 'healthy'), patch.object(upstream.subprocess, 'run') as run:
            upstream.switch('green', SHA, self.path)
        self.assertEqual(upstream.active(self.path), 'green')
        self.assertEqual(run.call_args_list[0].args[0], ['/usr/sbin/nginx', '-t'])
        self.assertEqual(run.call_args_list[1].args[0], ['/usr/bin/systemctl', 'reload', 'nginx'])

    def test_invalid_config_or_uncertain_reload_restores_file_and_reloads_old_config(self):
        for fail_at in (0, 1):
            with self.subTest(fail_at=fail_at):
                self.path.write_text(upstream.config_for('blue'))
                results = [None] * fail_at + [subprocess.CalledProcessError(1, 'nginx')] + [None, None]
                with patch.object(upstream, 'healthy'), \
                     patch.object(upstream.subprocess, 'run', side_effect=results), self.assertRaises(subprocess.CalledProcessError):
                    upstream.switch('green', SHA, self.path)
                self.assertEqual(upstream.active(self.path), 'blue')

    def test_symlink_config_is_rejected(self):
        target = self.path.with_name('target')
        self.path.rename(target)
        self.path.symlink_to(target)
        with self.assertRaises(ValueError):
            upstream.active(self.path)
