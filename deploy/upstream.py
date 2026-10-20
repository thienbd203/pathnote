#!/usr/bin/env python3
"""Root-owned, fixed-path Nginx switcher. No caller-controlled paths or commands."""
import fcntl
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import urllib.request

CONFIG = Path('/etc/nginx/conf.d/pathnote-rust-upstream.conf')
PORTS = {'blue': 3000, 'green': 3001}
INTERNAL = '''server {
    listen 127.0.0.1:3010;
    server_name localhost;
    location / {
        proxy_pass http://pathnote_rust;
        proxy_http_version 1.1;
        proxy_set_header Connection close;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 120s;
        client_max_body_size 20m;
    }
}
'''
HEADER = '# Managed by /usr/local/sbin/pathnote-upstream. Install inside nginx\'s http context.\n'


def config_for(slot):
    return HEADER + f'upstream pathnote_rust {{\n    server 127.0.0.1:{PORTS[slot]};\n}}\n' + INTERNAL


def active(path=CONFIG):
    if path.is_symlink():
        raise ValueError('Upstream configuration must be a regular root-owned file')
    data = path.read_text()
    for slot in PORTS:
        if data == config_for(slot):
            return slot
    raise ValueError('Unexpected upstream configuration; inspect it before deploying')


def healthy(slot, sha):
    request = urllib.request.Request(f'http://127.0.0.1:{PORTS[slot]}/health',
                                     headers={'Cache-Control': 'no-cache'})
    with urllib.request.urlopen(request, timeout=10) as response:
        if response.geturl() != request.full_url:
            raise ValueError('Health endpoint redirected')
        body = response.read(4097)
    if len(body) > 4096:
        raise ValueError('Health response too large')
    data = json.loads(body).get('data', {})
    if data.get('status') != 'ok' or data.get('database') != 'ok' or data.get('version') != sha:
        raise ValueError('Target slot is unhealthy or has the wrong version')


def write_atomic(path, content):
    fd, name = tempfile.mkstemp(prefix='.pathnote-upstream-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as file:
            file.write(content)
            file.flush()
            os.fsync(file.fileno())
        os.chmod(name, 0o644)
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def switch(slot, sha, path=CONFIG):
    if slot not in PORTS or not re.fullmatch('[a-f0-9]{40}', sha):
        raise ValueError('Expected blue|green and a full commit SHA')
    old = active(path)
    healthy(slot, sha)
    if old == slot:
        return
    previous = path.read_text()
    write_atomic(path, config_for(slot))
    try:
        subprocess.run(['/usr/sbin/nginx', '-t'], check=True, timeout=30)
        subprocess.run(['/usr/bin/systemctl', 'reload', 'nginx'], check=True, timeout=30)
    except BaseException:
        # Restore both on-disk and running config, including a reload with uncertain outcome.
        write_atomic(path, previous)
        subprocess.run(['/usr/sbin/nginx', '-t'], check=True, timeout=30)
        subprocess.run(['/usr/bin/systemctl', 'reload', 'nginx'], check=True, timeout=30)
        raise


def main():
    if os.geteuid() != 0:
        raise ValueError('Run the installed root-owned helper through sudo')
    info = CONFIG.stat()
    if info.st_uid != 0 or info.st_mode & 0o022:
        raise ValueError('Upstream config must be root-owned and not writable by group/others')
    with open('/run/lock/pathnote-upstream.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if sys.argv[1:] == ['status']:
            print(active())
        elif len(sys.argv) == 3:
            switch(sys.argv[1], sys.argv[2])
        else:
            raise ValueError('Usage: pathnote-upstream status | blue|green COMMIT_SHA')


if __name__ == '__main__':
    try:
        main()
    except Exception:
        print('Upstream switch failed; inspect Nginx config, target health and service logs.', file=sys.stderr)
        sys.exit(1)
