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
    def response(self, data):
        return patch.object(healthcheck.urllib.request, 'urlopen', return_value=io.BytesIO(json.dumps(data).encode()))

    def test_matching_running_process_and_database(self):
        with self.response({'data': {'status': 'ok', 'database': 'ok', 'version': 'new'}}):
            healthcheck.check('http://127.0.0.1:3000/health', 'new')

    def test_old_process_or_failed_database_is_rejected(self):
        for data in ({'status': 'ok', 'database': 'ok', 'version': 'old'},
                     {'status': 'ok', 'database': 'error', 'version': 'new'}):
            with self.subTest(data=data), self.response({'data': data}), self.assertRaises(ValueError):
                healthcheck.check('https://example.com/health', 'new')

    def test_public_plain_http_is_rejected_before_network_access(self):
        with patch.object(healthcheck.urllib.request, 'urlopen') as request, self.assertRaises(ValueError):
            healthcheck.check('http://example.com/health', 'new')
        request.assert_not_called()

    def test_oversized_response_is_rejected(self):
        with patch.object(healthcheck.urllib.request, 'urlopen', return_value=io.BytesIO(b' ' * 4097)), self.assertRaises(ValueError):
            healthcheck.check('https://example.com/health', 'new')
