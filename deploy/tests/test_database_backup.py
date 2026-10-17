import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('backup', Path(__file__).resolve().parents[1] / 'database-backup.py')
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)
NEW = '20261016000000-42-1'


class BackupTests(unittest.TestCase):
    def test_connection_url_becomes_individual_libpq_environment_variables(self):
        environment = helper.connection_environment(
            'postgresql://backup:p%40ss%3A%2F%25%27@127.0.0.1:5433/pathnote%5Frust'
            '?sslmode=require&application_name=pre+deploy&options=-c%20statement_timeout%3D0'
        )
        self.assertEqual(environment, {
            'PATH': '/usr/bin:/bin', 'PGCONNECT_TIMEOUT': '15',
            'PGHOST': '127.0.0.1', 'PGPORT': '5433', 'PGUSER': 'backup',
            'PGPASSWORD': "p@ss:/%'", 'PGDATABASE': 'pathnote_rust',
            'PGSSLMODE': 'require', 'PGAPPNAME': 'pre deploy',
            'PGOPTIONS': '-c statement_timeout=0',
        })
        self.assertEqual(helper.connection_environment('postgres://user@[::1]/db')['PGHOST'], '::1')
        for url in ('postgres://host/', 'postgres://one,two/db', 'postgres://host/db#fragment',
                    'postgres://host/db?unknown=1', 'postgres://host/db?sslmode',
                    'postgres://host/db%00', 'postgres://host:wrong/db'):
            with self.subTest(url=url), self.assertRaises(ValueError):
                helper.connection_environment(url)

    def test_env_quotes_comments_and_url_characters_without_shell_execution(self):
        url = 'postgres://user:p%40ss@localhost/db?application_name=backup#fragment'
        for value in (url, f"'{url}'", f'"{url}" # comment', url + ' # comment'):
            self.assertEqual(helper.database_url(f'OTHER=$(bad)\nexport DATABASE_URL = {value}\n'), url)
        for value in ('', 'DATABASE_URL=wrong', 'DATABASE_URL="unfinished', 'DATABASE_URL=postgres://$USER/db',
                      'DATABASE_URL=postgres://host/db\nDATABASE_URL=postgres://other/db'):
            with self.assertRaises(ValueError):
                helper.database_url(value)

    def test_dump_is_private_nonempty_and_only_five_complete_dumps_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'shared').mkdir()
            (root / 'shared/.env').write_text('DATABASE_URL="postgres://localhost/test"\n')
            directory = root / 'backups'
            directory.mkdir(mode=0o700)
            for day in range(10, 16):
                (directory / f'202610{day}000000-1-1.dump').write_bytes(b'old')
            (directory / 'manual.dump').write_bytes(b'keep')

            def dump(args, **kwargs):
                self.assertEqual(args, ['/usr/bin/pg_dump', '--format=custom', '--no-password'])
                self.assertEqual(kwargs['user'], 123)
                self.assertEqual(kwargs['extra_groups'], [])
                self.assertEqual(kwargs['env']['PGDATABASE'], 'test')
                self.assertEqual(kwargs['env']['PGHOST'], 'localhost')
                kwargs['stdout'].write(b'PGDMP-test')

            real_fstat = os.fstat
            with patch.object(helper.os, 'geteuid', return_value=0), patch.object(helper.os, 'fstat', wraps=real_fstat) as stat, \
                    patch.object(helper.pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=123, pw_gid=456)), \
                    patch.object(helper.subprocess, 'run', side_effect=dump):
                actual = os.stat(directory)
                # Directory ownership check also runs on macOS without root.
                stat.side_effect = lambda fd: SimpleNamespace(st_uid=0, st_mode=actual.st_mode) if os.path.isdir(f'/dev/fd/{fd}') else real_fstat(fd)
                helper.backup(NEW, root, directory)
                target = directory / (NEW + '.dump')
                self.assertEqual(target.stat().st_mode & 0o777, 0o600)
                self.assertEqual(target.read_bytes(), b'PGDMP-test')
                self.assertEqual(len([p for p in directory.iterdir() if helper.RELEASE.fullmatch(p.stem)]), 5)
                self.assertTrue((directory / 'manual.dump').exists())
                with self.assertRaises(ValueError):
                    helper.backup('20261001000000-1-1', root, directory)
                self.assertEqual(target.read_bytes(), b'PGDMP-test')
                with self.assertRaises(FileExistsError):
                    helper.backup(NEW, root, directory)
                self.assertFalse((directory / (NEW + '.dump.partial')).exists())

    def test_failed_or_empty_dump_never_publishes_backup(self):
        for error in (None, subprocess.CalledProcessError(1, 'pg_dump')):
            with self.subTest(error=error), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / 'shared').mkdir()
                (root / 'shared/.env').write_text('DATABASE_URL=postgres://localhost/test\n')
                directory = root / 'backups'
                directory.mkdir(mode=0o700)
                with patch.object(helper.os, 'geteuid', return_value=0), \
                        patch.object(helper.os, 'fstat', return_value=SimpleNamespace(st_uid=0, st_mode=0o700, st_size=0)), \
                        patch.object(helper.pwd, 'getpwnam', return_value=SimpleNamespace(pw_uid=123, pw_gid=456)), \
                        patch.object(helper.subprocess, 'run', side_effect=error):
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        helper.backup(NEW, root, directory)
                self.assertEqual(list(directory.iterdir()), [])
