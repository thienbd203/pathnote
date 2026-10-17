import importlib.util
import io
import json
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('healthcheck', Path(__file__).parents[1] / 'healthcheck.py')
healthcheck = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(healthcheck)


class HealthcheckTests(unittest.TestCase):
    def response(self, data, final_url=None):
        def opened(request, **_kwargs):
            response = io.BytesIO(json.dumps(data).encode())
            response.geturl = lambda: final_url or request.full_url
            return response
        return patch.object(healthcheck.urllib.request, 'urlopen', side_effect=opened)

    def test_matching_running_process_and_database(self):
        with self.response({'data': {'status': 'ok', 'database': 'ok', 'version': 'new'}}) as request:
            healthcheck.check('http://127.0.0.1:3000/health', 'new')
        headers = dict((key.lower(), value) for key, value in request.call_args[0][0].header_items())
        self.assertEqual(headers['user-agent'], 'Pathnote-Healthcheck/1.0')
        self.assertEqual(headers['accept'], 'application/json')
        self.assertEqual(headers['cache-control'], 'no-cache')

    def test_old_process_or_failed_database_is_rejected(self):
        for data in ({'status': 'ok', 'database': 'ok', 'version': 'old'},
                     {'status': 'ok', 'database': 'error', 'version': 'new'}):
            with self.subTest(data=data), self.response({'data': data}), self.assertRaises(ValueError):
                healthcheck.check('https://example.com/health', 'new')

    def test_public_plain_http_is_rejected_before_network_access(self):
        with patch.object(healthcheck.urllib.request, 'urlopen') as request, self.assertRaises(ValueError):
            healthcheck.check('http://example.com/health', 'new')
        request.assert_not_called()

    def test_version_mismatch_is_distinguished_from_unhealthy_database(self):
        for data, message in (
            ({'status': 'ok', 'database': 'ok', 'version': 'old'}, 'process version'),
            ({'status': 'ok', 'database': 'error', 'version': 'new'}, 'database status'),
        ):
            with self.subTest(data=data), self.response({'data': data}), self.assertRaisesRegex(ValueError, message):
                healthcheck.check('https://example.com/health', 'new')

    def test_oversized_response_is_rejected(self):
        with patch.object(healthcheck.urllib.request, 'urlopen', return_value=io.BytesIO(b' ' * 4097)), self.assertRaises(ValueError):
            healthcheck.check('https://example.com/health', 'new')

    def test_collaboration_checks_top_level_status_and_exact_version(self):
        with self.response({'status': 'ok', 'version': 'new'}):
            healthcheck.check('https://example.com/up', 'new', 'collaboration')
        for body in ({'status': 'ok', 'version': 'old'}, {'status': 'error', 'version': 'new'}, {'data': {'status': 'ok', 'version': 'new'}}):
            with self.response(body), self.assertRaises(ValueError):
                healthcheck.check('https://example.com/up', 'new', 'collaboration')
        with patch.object(healthcheck.urllib.request, 'urlopen') as request, self.assertRaises(ValueError):
            healthcheck.check('http://127.0.0.1/up', 'new', 'collaboration')
        request.assert_not_called()

    def test_collaboration_gate_rejects_redirects_and_https_downgrades(self):
        for target in ('http://example.com/up', 'https://other.example/up'):
            with self.response({'status': 'ok', 'version': 'new'}, target), self.assertRaisesRegex(ValueError, 'redirect'):
                healthcheck.check('https://example.com/up', 'new', 'collaboration')
