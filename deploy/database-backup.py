#!/usr/bin/python3 -I
"""Root-installed backup helper. Arguments never select a command or a path.

pg_dump provides the backup format; this wrapper only limits privileges, reads
the single-line production connection setting, and publishes completed dumps.
"""
import os
import fcntl
from pathlib import Path
import pwd
import re
import subprocess
import sys

DEPLOY = Path('/opt/pathnote-rust')
# Outside the deploy-owned tree: a replaced symlink cannot redirect root writes.
BACKUPS = Path('/var/backups/pathnote-rust')
RELEASE = re.compile(r'[0-9]{14}-[0-9]+-[0-9]+')


def database_url(text):
    values = []
    for line in text.splitlines():
        match = re.fullmatch(r'\s*(?:export\s+)?DATABASE_URL\s*=\s*(.*?)\s*', line)
        if not match:
            continue
        value = match[1]
        if value.startswith(('"', "'")):
            quote = value[0]
            match = re.fullmatch(re.escape(quote) + r'([^\r\n]*?)' + re.escape(quote) + r'\s*(?:#.*)?', value)
            if not match:
                raise ValueError('DATABASE_URL must be a single-line URL')
            value = match[1]
        else:
            value = re.split(r'\s+#', value, maxsplit=1)[0].strip()
        # No shell evaluation or dotenv interpolation. Encode special characters.
        if not value.startswith(('postgres://', 'postgresql://')) or re.search(r'[\s\\$\x00\'"]', value):
            raise ValueError('DATABASE_URL must be a literal PostgreSQL URL; percent-encode special characters')
        values.append(value)
    if len(values) != 1:
        raise ValueError('Expected exactly one DATABASE_URL')
    return values[0]


def backup(release, deploy=DEPLOY, directory=BACKUPS):
    if not RELEASE.fullmatch(release):
        raise ValueError('Invalid release identifier')
    if os.geteuid() != 0:
        raise ValueError('Backup helper must run as root')
    account = pwd.getpwnam('pathnote')
    url = database_url((deploy / 'shared/.env').read_text())
    # The operator creates this root-only directory; never follow a leaf symlink.
    folder = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        info = os.fstat(folder)
        if info.st_uid != 0 or info.st_mode & 0o077:
            raise ValueError('Backup directory must be root-owned with mode 700')
        fcntl.flock(folder, fcntl.LOCK_EX)
        name = release + '.dump'
        existing = sorted(n for n in os.listdir(folder) if n.endswith('.dump') and RELEASE.fullmatch(n[:-5]))
        if existing and name < existing[-1]:
            raise ValueError('Backup release identifiers must advance; refusing to prune the recovery point')
        partial = name + '.partial'
        fd = os.open(partial, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=folder)
        try:
            with os.fdopen(fd, 'wb') as output:
                subprocess.run(
                    ['/usr/bin/pg_dump', '--format=custom', '--no-password'],
                    stdout=output, stderr=subprocess.DEVNULL, check=True,
                    user=account.pw_uid, group=account.pw_gid, extra_groups=[], cwd='/',
                    env={'PATH': '/usr/bin:/bin', 'PGDATABASE': url, 'PGCONNECT_TIMEOUT': '15'},
                )
                output.flush()
                if os.fstat(output.fileno()).st_size == 0:
                    raise ValueError('pg_dump produced an empty backup')
                os.fsync(output.fileno())
            # A rerun must never overwrite its original recovery point.
            os.link(partial, name, src_dir_fd=folder, dst_dir_fd=folder, follow_symlinks=False)
        finally:
            os.unlink(partial, dir_fd=folder)
        os.fsync(folder)
        names = sorted(n for n in os.listdir(folder) if n.endswith('.dump') and RELEASE.fullmatch(n[:-5]))
        for old in names[:-5]:
            os.unlink(old, dir_fd=folder)
    finally:
        os.close(folder)
    print(f'Backup ready: {deploy}/backups/{release}.dump')


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2:
            raise ValueError('Expected one release identifier')
        backup(sys.argv[1])
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        # Neither connection credentials nor upstream stderr enter CI logs.
        sys.exit('Database backup failed. Check client version, configuration and backup permissions on the VPS; migration was not started.')
