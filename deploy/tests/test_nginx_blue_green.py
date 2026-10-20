"""Opt-in real Nginx integration in isolated CI Linux (no real systemd service)."""
import concurrent.futures
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import urllib.request

SPEC = importlib.util.spec_from_file_location('nginx_upstream', Path(__file__).parents[1] / 'upstream.py')
upstream = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(upstream)


@unittest.skipUnless(os.environ.get('PATHNOTE_NGINX_TEST') == '1' and shutil.which('nginx'),
                     'Set PATHNOTE_NGINX_TEST=1 in isolated Linux with nginx installed')
class NginxBlueGreenTests(unittest.TestCase):
    def test_live_switch_preserves_slow_request_and_routes_new_requests_to_new_version(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        root.chmod(0o755)
        config = root / 'nginx.conf'
        route = root / 'upstream.conf'
        route.write_text(upstream.config_for('blue'))
        slow_started = threading.Event()
        slow_finish = threading.Event()
        servers = []
        for slot, port in upstream.PORTS.items():
            sha = ('a' if slot == 'blue' else 'b') * 40
            def handler(version):
                class Handler(BaseHTTPRequestHandler):
                    def do_GET(self):
                        if self.path == '/slow':
                            slow_started.set()
                            slow_finish.wait(15)
                        body = json.dumps({'data': {'status': 'ok', 'database': 'ok', 'version': version}}).encode()
                        self.send_response(200)
                        self.send_header('Content-Length', str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                    def log_message(self, *args):
                        pass
                return Handler
            server = ThreadingHTTPServer(('127.0.0.1', port), handler(sha))
            servers.append(server)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            self.addCleanup(server.server_close)
            self.addCleanup(server.shutdown)
        config.write_text(f'''pid {root}/nginx.pid;
error_log {root}/error.log;
events {{}}
http {{
    access_log off;
    include {route};
    server {{
        listen 127.0.0.1:18080;
        location / {{
            proxy_pass http://pathnote_rust;
            proxy_set_header Connection close;
            proxy_read_timeout 30s;
        }}
    }}
}}
''')
        command = [shutil.which('nginx'), '-p', str(root), '-c', str(config)]
        subprocess.run(command, check=True, capture_output=True)
        self.addCleanup(lambda: subprocess.run(command + ['-s', 'quit'], check=False, capture_output=True))
        def fetch(path='/health'):
            with urllib.request.urlopen('http://127.0.0.1:18080' + path, timeout=20) as response:
                return json.load(response)['data']['version']
        self.assertEqual(fetch(), 'a' * 40)
        actual_run = subprocess.run
        def nginx_commands(args, **kwargs):
            if args == ['/usr/sbin/nginx', '-t']:
                return actual_run(command + ['-t'], **kwargs)
            self.assertEqual(args, ['/usr/bin/systemctl', 'reload', 'nginx'])
            return actual_run(command + ['-s', 'reload'], **kwargs)
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            slow = pool.submit(fetch, '/slow')
            self.assertTrue(slow_started.wait(5))
            # Probe continuously through the actual reverse proxy while reload happens.
            stop = threading.Event()
            traffic_started = threading.Event()
            def traffic():
                versions = []
                while not stop.is_set():
                    versions.append(fetch())
                    traffic_started.set()
                    time.sleep(0.01)
                return versions
            requests = pool.submit(traffic)
            self.assertTrue(traffic_started.wait(5))
            try:
                with patch.object(upstream.subprocess, 'run', side_effect=nginx_commands):
                    upstream.switch('green', 'b' * 40, route)
                deadline = time.monotonic() + 5
                while fetch() != 'b' * 40:
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(0.02)
            finally:
                stop.set()
                slow_finish.set()
            self.assertEqual(slow.result(), 'a' * 40)
            versions = requests.result()
            self.assertTrue(versions)
            self.assertTrue(set(versions).issubset({'a' * 40, 'b' * 40}))
        # Reload has a short overlap while old workers close their listen sockets.
        deadline = time.monotonic() + 5
        consecutive = 0
        while consecutive < 5:
            consecutive = consecutive + 1 if fetch() == 'b' * 40 else 0
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.05)
        with urllib.request.urlopen('http://127.0.0.1:3010/health', timeout=5) as response:
            self.assertEqual(json.load(response)['data']['version'], 'b' * 40)
