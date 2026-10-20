#!/usr/bin/env python3
"""Two-slot activation; Nginx/systemd own traffic routing and graceful shutdown."""
import fcntl
import hashlib
import os
from pathlib import Path
import re
import signal
import shutil
import subprocess
import sys
import time

from healthcheck import check

PORTS = {'blue': 3000, 'green': 3001}
HELPER = '/usr/local/sbin/pathnote-upstream'
FILES = ('server', 'migration', 'healthcheck.py', 'deploy-version.txt', 'build-target.txt', 'blue-green.py')


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def service(action, slot):
    run('sudo', '-n', 'systemctl', action, f'pathnote-rust@{slot}.service')


def route(slot=None, sha=None):
    if slot is None:
        value = run('sudo', '-n', HELPER, 'status', capture_output=True).stdout.strip()
        if value not in PORTS:
            raise ValueError('Unknown active slot')
        return value
    run('sudo', '-n', HELPER, slot, sha)


def link(target, destination):
    temporary = destination.with_name('.' + destination.name + '-next')
    temporary.unlink(missing_ok=True)
    temporary.symlink_to(target)
    temporary.replace(destination)


def health(url, sha):
    for attempt in range(10):
        try:
            check(url, sha)
            return
        except Exception:
            if attempt == 9:
                raise
            time.sleep(2)


def drained(port):
    # Nginx closes old workers' idle connections on reload. Check this API's
    # established connections rather than all workers (other sites may have WebSockets).
    time.sleep(5)
    for _ in range(120):
        connections = run('ss', '-Htn', 'state', 'established', f'( sport = :{port} )',
                          capture_output=True).stdout.strip()
        if not connections:
            return True
        time.sleep(1)
    return False


def validate(root, release, url, sha, breaking):
    if not re.fullmatch(r'/opt/[a-zA-Z0-9_-]+', str(root)):
        raise ValueError('Invalid deploy path')
    if not re.fullmatch(r'[0-9]{14}-[0-9]+-[0-9]+', release):
        raise ValueError('Invalid release identifier')
    if not re.fullmatch(r'https://[a-zA-Z0-9.-]+(:[0-9]+)?/health', url):
        raise ValueError('Expected public HTTPS /health URL')
    if not re.fullmatch('[a-f0-9]{40}', sha):
        raise ValueError('Invalid commit SHA')
    if breaking != 'false':
        raise ValueError('Blue-green refuses breaking migrations. Follow the maintenance procedure.')


def release_path(root, path):
    resolved = path.resolve(strict=True)
    if resolved.parent != root / 'releases' or not re.fullmatch(r'[0-9]{14}-[0-9]+-[0-9]+', resolved.name):
        raise ValueError('Slot must point to a managed release')
    return resolved


def prepare(root, release, sha):
    candidate = root / 'releases' / release
    if candidate.is_symlink() or not candidate.is_dir():
        raise ValueError('Release must be a real directory')
    run('sha256sum', '--check', '--strict', 'SHA256SUMS', cwd=candidate)
    # Require every executable/helper used by this strategy to be covered.
    manifest = (candidate / 'SHA256SUMS').read_text().splitlines()
    expected = {f'{hashlib.sha256((candidate / name).read_bytes()).hexdigest()}  {name}' for name in FILES}
    if not expected.issubset(set(manifest)):
        raise ValueError('Incomplete artifact checksum manifest')
    if (candidate / 'deploy-version.txt').read_text().strip() != sha:
        raise ValueError('Artifact version mismatch')
    machine = os.uname().machine
    target = (candidate / 'build-target.txt').read_text().strip()
    if (machine, target) not in {('x86_64', 'x86_64-unknown-linux-musl'), ('aarch64', 'aarch64-unknown-linux-musl')}:
        raise ValueError('Release architecture does not match VPS')
    run('chgrp', '-R', 'pathnote', str(candidate))
    candidate.chmod(0o750)
    for name in FILES:
        (candidate / name).chmod(0o750 if name in {'server', 'migration'} else 0o640)
    (candidate / 'SHA256SUMS').chmod(0o640)
    (candidate / '.env').symlink_to(root / 'shared/.env')
    for mode, name in (('-r', '.env'), ('-r', 'keys/access-token.pem'), ('-w', 'media')):
        run('sudo', '-n', '-u', 'pathnote', 'test', mode, str(root / 'shared' / name))
    return candidate


def activate(root, candidate, url, sha):
    old = route()
    new = 'green' if old == 'blue' else 'blue'
    current = root / 'current'
    old_release = release_path(root, root / 'slots' / old)
    old_sha = (old_release / 'deploy-version.txt').read_text().strip()
    if not current.is_symlink():
        raise ValueError('Current must be a symlink')
    old_new_slot = release_path(root, root / 'slots' / new) if (root / 'slots' / new).is_symlink() else None
    if current.resolve(strict=True) != old_release:
        raise ValueError('Current release and Nginx disagree; recover before deploying')
    service('is-active', old)
    health(f'http://127.0.0.1:{PORTS[old]}/health', old_sha)
    health('http://127.0.0.1:3010/health', old_sha)
    health(url + '?before=' + candidate.name, old_sha)
    # A previous timed-out drain must finish before reusing its slot.
    if not drained(PORTS[new]):
        raise RuntimeError("Inactive slot still has requests; retry after they finish")
    # A failed migration leaves the old app serving. Schema must remain compatible.
    run('sudo', '-n', '/usr/local/sbin/pathnote-db-backup', candidate.name)
    run('sudo', '-n', '-u', 'pathnote', str(candidate / 'migration'), 'up', cwd=candidate)
    started = False
    switched = False
    try:
        service('stop', new)
        link(candidate, root / 'slots' / new)
        started = True
        service('restart', new)
        health(f'http://127.0.0.1:{PORTS[new]}/health', sha)
        service('enable', new)
        switched = True  # A switch can fail after Nginx has accepted the reload.
        route(new, sha)
        health('http://127.0.0.1:3010/health', sha)
        health(url + '?release=' + candidate.name, sha)
        link(candidate, current)
    except BaseException:
        if switched:
            # If rollback cannot be verified, leave BOTH processes available.
            route(old, old_sha)
            health('http://127.0.0.1:3010/health', old_sha)
            health(url + '?rollback=' + candidate.name, old_sha)
            link(old_release, current)
        if started:
            # Retire a briefly exposed failed target only after its requests drain.
            if not switched or drained(PORTS[new]):
                service('stop', new)
                service('disable', new)
                if old_new_slot is not None:
                    link(old_new_slot, root / 'slots' / new)
        raise
    # Traffic already uses new. Failure to drain/stop old must not undo a healthy release.
    try:
        if drained(PORTS[old]):
            service('stop', old)  # Axum SIGTERM gracefully finishes in-flight requests.
            service('disable', old)
        else:
            print(f'WARNING: {old} still has connections; left running. Inspect before next deploy.', flush=True)
    except Exception:
        print(f'WARNING: Could not retire {old}; new release remains active.', flush=True)
    # Preserve both slots, including a process retained after a drain timeout.
    protected = {candidate, old_release}
    for slot in PORTS:
        protected.add((root / 'slots' / slot).resolve())
    releases = sorted((root / 'releases').iterdir(), reverse=True)
    for path in releases:
        if path.is_symlink() or not path.is_dir() or not re.fullmatch(r'[0-9]{14}-[0-9]+-[0-9]+', path.name):
            continue
        if path in protected:
            continue
        if len(protected) < 3:
            protected.add(path)
        else:
            try:
                shutil.rmtree(path)
            except OSError:
                print(f"WARNING: Could not prune old release {path.name}", flush=True)
    print(f'Blue-green release healthy: {new} {sha}', flush=True)


def main():
    root, release, url, sha, breaking = sys.argv[1:]
    root = Path(root)
    validate(root, release, url, sha, breaking)
    with (root / '.deploy.lock').open('a') as lock:
        deadline = time.monotonic() + 300
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise TimeoutError('Another deployment holds the lock')
                time.sleep(1)
        candidate = prepare(root, release, sha)
        activate(root, candidate, url, sha)


if __name__ == '__main__':
    def interrupted(signum, _frame):
        raise RuntimeError(f'Deployment interrupted by signal {signum}')
    for sig in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT):
        signal.signal(sig, interrupted)
    try:
        main()
    except Exception:
        print('Blue-green deploy failed. Check slot health/upstream; compatible migrations are retained.', file=sys.stderr)
        raise
