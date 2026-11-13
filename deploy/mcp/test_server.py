import asyncio
import os
import unittest

import httpx2
import psycopg
from fastmcp.server.auth.providers.jwt import StaticTokenVerifier

from server import build_server

HEADERS = {"accept": "application/json, text/event-stream"}


@unittest.skipUnless(os.getenv("PATHNOTE_MCP_TEST_DATABASE_URL"), "isolated database required")
class PersonalMcpTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        auth = StaticTokenVerifier(
            {
                "owner": {"client_id": "test", "sub": "owner-google", "scopes": ["openid"]},
                "unknown": {"client_id": "test", "sub": "unknown-google", "scopes": ["openid"]},
                "empty": {"client_id": "test", "sub": "empty-google", "scopes": ["openid"]},
                "other": {"client_id": "test", "sub": "other-google", "scopes": ["openid"]},
            },
            required_scopes=["openid"],
        )
        server = build_server(auth, os.environ["PATHNOTE_MCP_TEST_DATABASE_URL"])
        self.app = server.http_app(stateless_http=True, json_response=True)

    async def call(self, token, words, **extra):
        async with self.app.lifespan(self.app):
            return await self.request(token, "check_vocabulary", {"words": words, **extra})

    async def request(self, token, tool, arguments):
        async with httpx2.AsyncClient(
            transport=httpx2.ASGITransport(app=self.app), base_url="http://test"
        ) as client:
            return await client.post(
                "/mcp",
                headers={**HEADERS, **({"Authorization": f"Bearer {token}"} if token else {})},
                json={
                    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": {"name": tool, "arguments": arguments},
                },
            )

    async def reserve(self, words, day, key, mode="word_family", token="empty"):
        response = await self.request(token, "record_generated_vocabulary", {
            "words": words, "source": "chatgpt_daily", "batch_date": day,
            "idempotency_key": key, "match_mode": mode,
        })
        body = response.json()
        self.assertFalse(body["result"].get("isError"), body)
        return body["result"]["structuredContent"]

    def batch(self, first, label):
        return [first, *[f"{label} item {n}" for n in range(19)]]

    async def test_auth_required(self):
        self.assertEqual((await self.call(None, ["honest"])).status_code, 401)

    async def test_unlinked_google_account_denied(self):
        body = (await self.call("unknown", ["honest"])).json()
        self.assertTrue("error" in body or body["result"]["isError"])

    async def test_only_active_owned_words_and_dedup(self):
        body = (await self.call(
            "owner", [" Honest ", "honest", "FIND  OUT", "other", "deleted", "inactive", "closed"]
        )).json()
        self.assertFalse(body["result"].get("isError"), body)
        result = body["result"]["structuredContent"]
        self.assertEqual(result["known_words"], [" Honest ", "FIND  OUT"])
        self.assertEqual(result["new_words"], ["other", "deleted", "inactive", "closed"])
        self.assertEqual(
            [(match["matched_word"], match["source"]) for match in result["matches"]],
            [("honest", "vocabulary"), ("honest", "vocabulary"), ("find out", "vocabulary")],
        )

    async def test_accounts_check_their_own_words(self):
        for token, known, new in [
            ("owner", ["honest"], ["other"]),
            ("other", ["other"], ["honest"]),
            ("empty", [], ["honest", "other"]),
            ("owner", ["honest"], ["other"]),
        ]:
            with self.subTest(token=token):
                body = (await self.call(token, ["honest", "other"])).json()
                self.assertEqual(
                    (body["result"]["structuredContent"]["known_words"],
                     body["result"]["structuredContent"]["new_words"]),
                    (known, new),
                )

    async def test_invalid_arguments(self):
        for words, extra in [([" "], {}), (["word"] * 51, {}), (["word"], {"user_id": 2}), (["word"], {"google_id": "other-google"})]:
            with self.subTest(words=words, extra=extra):
                body = (await self.call("owner", words, **extra)).json()
                self.assertTrue("error" in body or body["result"]["isError"])

    async def test_modes_unicode_and_long_existing_word(self):
        words = ["working", "choice", "manage", "CAFÉ", "😀" * 1000]
        exact = (await self.call("owner", words)).json()["result"]["structuredContent"]
        lemma = (await self.call("owner", words, match_mode="lemma")).json()["result"]["structuredContent"]
        family = (await self.call("owner", words, match_mode="word_family")).json()["result"]["structuredContent"]
        self.assertEqual(exact["known_words"], words[3:])
        self.assertEqual(lemma["known_words"], [words[0], *words[3:]])
        self.assertEqual(family["known_words"], words)

    async def test_history_conflict_recovery_and_isolation(self):
        words = self.batch("umbrella", "history")
        async with self.app.lifespan(self.app):
            saved = await self.reserve(words, "2026-11-01", "history")
            self.assertTrue(saved["success"])
            replay = await self.reserve(words, "2026-11-01", "history")
            self.assertTrue(replay["idempotent_replay"])
            self.assertEqual(replay["batch_id"], saved["batch_id"])
            changed = await self.reserve(self.batch("apple", "history"), "2026-11-01", "history")
            self.assertEqual(changed["error_code"], "IDEMPOTENCY_CONFLICT")
            same_day = await self.reserve(words, "2026-11-01", "another-key")
            self.assertEqual(same_day["error_code"], "DAILY_BATCH_EXISTS")
            self.assertEqual(same_day["daily_batch"]["recorded_words"], words)
            rejected = await self.reserve(self.batch("umbrellas", "reject"), "2026-11-02", "reject")
            self.assertEqual(rejected["error_code"], "DUPLICATE_VOCABULARY")
            self.assertEqual(rejected["duplicates"][0]["source"], "generated_history")
            other = await self.reserve(words, "2026-11-01", "history", token="other")
            self.assertTrue(other["success"])
        recovered = (await self.call("empty", ["umbrella", "reject item 1"], batch_date="2026-11-01")).json()
        result = recovered["result"]["structuredContent"]
        self.assertEqual(result["daily_batch"]["recorded_words"], words)
        self.assertEqual(result["new_words"], ["reject item 1"])
        self.assertEqual(result["matches"][0]["source"], "generated_history")

    async def test_rejects_entire_batch_when_two_candidates_match(self):
        words = self.batch("happy", "internal")
        words[-1] = "happiness"
        async with self.app.lifespan(self.app):
            result = await self.reserve(words, "2026-11-03", "internal")
        self.assertEqual(result["error_code"], "DUPLICATE_VOCABULARY")
        self.assertEqual(result["duplicates"][0]["source"], "candidate_batch")
        verified = (await self.call("empty", ["happy"], batch_date="2026-11-03")).json()["result"]["structuredContent"]
        self.assertIsNone(verified["daily_batch"])
        self.assertEqual(verified["new_words"], ["happy"])

    async def test_concurrent_exact_lemma_and_family(self):
        async with self.app.lifespan(self.app):
            for i, (first, second, mode) in enumerate([
                ("orange", "ORANGE", "exact"),
                ("go", "went", "lemma"),
                ("decide", "decision", "word_family"),
            ]):
                with self.subTest(mode=mode):
                    left, right = await asyncio.gather(
                        self.reserve(self.batch(first, f"left {i}"), f"2026-11-{10 + i:02}", f"left-{i}", mode),
                        self.reserve(self.batch(second, f"right {i}"), f"2026-11-{20 + i:02}", f"right-{i}", mode),
                    )
                    self.assertEqual(sum(result["success"] for result in (left, right)), 1)
                    self.assertEqual((right if left["success"] else left)["error_code"], "DUPLICATE_VOCABULARY")

    async def test_concurrent_same_day_reuses_single_batch_of_twenty(self):
        words = [
            "bridge", "bottle", "garden", "pencil", "window", "village", "pocket",
            "carrot", "blanket", "ticket", "airport", "market", "jacket", "pillow",
            "shelf", "mirror", "towel", "pencilcase", "basket", "spoon",
        ]
        async with self.app.lifespan(self.app):
            first, second = await asyncio.gather(
                self.reserve(words, "2026-11-06", "morning-a"),
                self.reserve([*words[:-1], "fork"], "2026-11-06", "morning-b"),
            )
        saved, rejected = (first, second) if first["success"] else (second, first)
        self.assertTrue(saved["success"])
        self.assertEqual(saved["recorded_count"], 20)
        self.assertEqual(rejected["error_code"], "DAILY_BATCH_EXISTS")
        self.assertEqual(rejected["daily_batch"]["recorded_words"], saved["recorded_words"])

    @unittest.skipUnless(os.getenv("PATHNOTE_MCP_TEST_ADMIN_URL"), "isolated admin database required")
    async def test_history_does_not_write_cards_or_block_vocabulary_sync(self):
        words = self.batch("valley", "manual import")
        async with self.app.lifespan(self.app):
            saved = await self.reserve(words, "2026-11-05", "manual-import", token="owner")
            self.assertTrue(saved["success"])

        async with await psycopg.AsyncConnection.connect(os.environ["PATHNOTE_MCP_TEST_ADMIN_URL"]) as admin:
            result = await admin.execute("SELECT count(*) FROM public.vocabulary_cards WHERE word = 'valley'")
            self.assertEqual((await result.fetchone())[0], 0)
            await admin.execute("SET LOCAL ROLE pathnote_app_fixture")
            await admin.execute("INSERT INTO public.vocabulary_cards VALUES (1, 'unused', true, 'valley')")
            await admin.execute("UPDATE public.vocabulary_cards SET active = false WHERE word = 'valley'")
            await admin.execute("UPDATE public.vocabulary_cards SET active = true WHERE word = 'valley'")

        result = (await self.call("owner", ["valley"])).json()["result"]["structuredContent"]
        self.assertEqual(result["matches"][0]["source"], "vocabulary")

    async def test_record_requires_auth_and_twenty_words(self):
        args = {
            "words": ["orange"], "source": "chatgpt_daily",
            "batch_date": "2026-11-30", "idempotency_key": "validation",
        }
        async with self.app.lifespan(self.app):
            denied = await self.request(None, "record_generated_vocabulary", args)
            self.assertEqual(denied.status_code, 401)
            short = (await self.request("owner", "record_generated_vocabulary", args)).json()
            self.assertTrue("error" in short or short["result"]["isError"])
            args["words"] = self.batch("apple", "unknown")
            unknown = (await self.request("unknown", "record_generated_vocabulary", args)).json()
            self.assertTrue("error" in unknown or unknown["result"]["isError"])


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
            app = build_server(auth, "unused").http_app(stateless_http=True)
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
