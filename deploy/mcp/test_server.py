import os
import unittest

import httpx2
from fastmcp.server.auth.providers.jwt import StaticTokenVerifier

from server import build_server

HEADERS = {"accept": "application/json, text/event-stream"}


@unittest.skipUnless(os.getenv("PATHNOTE_MCP_TEST_DATABASE_URL"), "isolated database required")
class PersonalMcpTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        auth = StaticTokenVerifier(
            {
                "owner": {"client_id": "test", "sub": "owner-google", "scopes": ["openid"]},
                "other": {"client_id": "test", "sub": "other-google", "scopes": ["openid"]},
            },
            required_scopes=["openid"],
        )
        server = build_server(auth, "owner-google", os.environ["PATHNOTE_MCP_TEST_DATABASE_URL"])
        self.app = server.http_app(stateless_http=True, json_response=True)

    async def call(self, token, words, **extra):
        async with self.app.lifespan(self.app), httpx2.AsyncClient(
            transport=httpx2.ASGITransport(app=self.app), base_url="http://test"
        ) as client:
            return await client.post(
                "/mcp",
                headers={**HEADERS, **({"Authorization": f"Bearer {token}"} if token else {})},
                json={
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "check_vocabulary",
                        "arguments": {"words": words, **extra},
                    },
                },
            )

    async def test_auth_required(self):
        self.assertEqual((await self.call(None, ["honest"])).status_code, 401)

    async def test_other_account_denied(self):
        body = (await self.call("other", ["honest"])).json()
        self.assertTrue("error" in body or body["result"]["isError"])

    async def test_only_active_owned_words_and_dedup(self):
        body = (await self.call(
            "owner", [" Honest ", "honest", "FIND  OUT", "other", "deleted", "inactive", "closed"]
        )).json()
        self.assertEqual(body["result"]["structuredContent"], {
            "known_words": ["Honest", "FIND OUT"],
            "new_words": ["other", "deleted", "inactive", "closed"],
        })

    async def test_invalid_arguments(self):
        for words, extra in [([" "], {}), (["word"] * 51, {}), (["word"], {"user_id": 2})]:
            with self.subTest(words=words, extra=extra):
                body = (await self.call("owner", words, **extra)).json()
                self.assertTrue("error" in body or body["result"]["isError"])


class OAuthDiscoveryTests(unittest.IsolatedAsyncioTestCase):
    async def test_oauth_metadata_and_challenge(self):
        import tempfile
        from unittest.mock import patch

        from fastmcp import settings
        from fastmcp.server.auth.providers.google import GoogleProvider
        from pathlib import Path

        base_url = "https://mcp.example.com"
        with tempfile.TemporaryDirectory() as state_dir, patch.object(
            settings, "home", Path(state_dir)
        ):
            auth = GoogleProvider(
                client_id="fixture",
                client_secret="fixture-secret",
                base_url=base_url,
                enable_cimd=False,
                forward_resource=False,
            )
            app = build_server(auth, "owner-google", "unused").http_app(stateless_http=True)
            async with app.lifespan(app), httpx2.AsyncClient(
                transport=httpx2.ASGITransport(app=app), base_url=base_url
            ) as client:
                protected = await client.get("/.well-known/oauth-protected-resource/mcp")
                self.assertEqual(protected.status_code, 200)
                self.assertEqual(protected.json()["resource"], f"{base_url}/mcp")
                discovery = await client.get("/.well-known/oauth-authorization-server")
                self.assertEqual(discovery.status_code, 200)
                self.assertIn("S256", discovery.json()["code_challenge_methods_supported"])
                self.assertIn("refresh_token", discovery.json()["grant_types_supported"])
                denied = await client.post("/mcp")
                self.assertEqual(denied.status_code, 401)
                self.assertIn("resource_metadata", denied.headers["www-authenticate"])
