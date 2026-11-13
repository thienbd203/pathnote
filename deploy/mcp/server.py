"""MCP vocabulary checks and proposal history for the authenticated Pathnote account."""

import logging
import os
from contextlib import asynccontextmanager
from datetime import date
from typing import Annotated, Literal
from urllib.parse import urlsplit

import psycopg
from fastmcp import FastMCP
from fastmcp.exceptions import ToolError
from fastmcp.server.auth import AuthProvider
from fastmcp.server.auth.providers.google import GoogleProvider
from fastmcp.server.dependencies import get_access_token
from mcp.types import ToolAnnotations
from pydantic import Field, StringConstraints

from dedup import MatchMode, Word, analyze
from history import UnlinkedAccount, check, find_batch, receipt, record

Candidate = Annotated[str, StringConstraints(min_length=1, max_length=1000)]
Candidates = Annotated[list[Candidate], Field(min_length=1, max_length=50)]
DailyWords = Annotated[list[Candidate], Field(min_length=20, max_length=20)]
RequestKey = Annotated[str, StringConstraints(min_length=1, max_length=128)]
LOGGER = logging.getLogger(__name__)


def validated_words(words: list[str]) -> list[Word]:
    candidates = [analyze(word) for word in words]
    if any(not word.exact or "\x00" in word.exact for word in candidates):
        raise ToolError("Candidate words must contain text and cannot contain NUL.")
    return candidates


@asynccontextmanager
async def authenticated_connection(dsn: str):
    token = get_access_token()
    subject = token.subject if token else None
    if not subject:
        raise ToolError("Sign in with Google before checking Pathnote vocabulary.")
    try:
        connection = await psycopg.AsyncConnection.connect(
            dsn, connect_timeout=5,
            options="-c statement_timeout=5000",
        )
        async with connection:
            await connection.set_isolation_level(psycopg.IsolationLevel.READ_COMMITTED)
            yield connection, subject
    except UnlinkedAccount:
        raise ToolError("Sign in with a Google account already linked to Pathnote.") from None
    except psycopg.Error as error:
        LOGGER.warning("Pathnote MCP database error: %s (%s)", type(error).__name__, error.sqlstate)
        raise ToolError("Pathnote vocabulary is temporarily unavailable.") from error


def build_server(auth: AuthProvider, dsn: str) -> FastMCP:
    server = FastMCP("Pathnote cá nhân", auth=auth)

    @server.tool(annotations=ToolAnnotations(read_only_hint=True, open_world_hint=False))
    async def check_vocabulary(
        words: Candidates,
        match_mode: MatchMode = "exact",
        batch_date: date | None = None,
    ) -> dict:
        """Check candidates before generating a daily batch. Use word_family for daily proposals.

        Compares saved vocabulary, prior proposals and candidates already accepted
        in this request. Include the local batch_date to recover a reserved batch.
        """
        candidates = validated_words(words)
        async with authenticated_connection(dsn) as (connection, subject):
            result = await check(connection, subject, candidates, match_mode)
            if batch_date is not None:
                prior = await find_batch(connection, subject, batch_date)
                result["daily_batch"] = receipt(prior[0], prior[4], True) if prior else None
            return result

    @server.tool(annotations=ToolAnnotations(
        read_only_hint=False, destructive_hint=False, idempotent_hint=True, open_world_hint=False,
    ))
    async def record_generated_vocabulary(
        words: DailyWords,
        source: Literal["chatgpt_daily"],
        batch_date: date,
        idempotency_key: RequestKey,
        match_mode: MatchMode = "word_family",
    ) -> dict:
        """Reserve exactly 20 headwords in proposal history after preparing JSON v1.

        Use the user's local date. Replay the same key and payload for retries.
        This tool records headwords only. Return JSON v1 to the user for manual import.
        """
        if not idempotency_key.strip() or "\x00" in idempotency_key:
            raise ToolError("Use a nonblank idempotency key.")
        candidates = validated_words(words)
        async with authenticated_connection(dsn) as (connection, subject):
            return await record(connection, subject, candidates, batch_date, idempotency_key, match_mode)

    return server


def main() -> None:
    base_url = os.environ["PATHNOTE_MCP_PUBLIC_URL"].rstrip("/")
    origin = urlsplit(base_url)
    if origin.scheme != "https" or not origin.netloc or origin.path:
        raise ValueError("Configure a public HTTPS origin.")
    redirect_uri = os.getenv("PATHNOTE_MCP_CHATGPT_REDIRECT_URI")
    chatgpt_redirects = [
        "https://chatgpt.com/connector_platform_oauth_redirect",
        "https://chatgpt.com/connector/oauth/*",
    ]
    auth = GoogleProvider(
        client_id=os.environ["PATHNOTE_MCP_GOOGLE_CLIENT_ID"],
        client_secret=os.environ["PATHNOTE_MCP_GOOGLE_CLIENT_SECRET"],
        base_url=base_url,
        jwt_signing_key=os.environ["PATHNOTE_MCP_SIGNING_KEY"],
        required_scopes=["openid"],
        enable_cimd=False,
        # Google signs in the user; the OAuth proxy scopes its own tokens to MCP.
        forward_resource=False,
        allowed_client_redirect_uris=[redirect_uri] if redirect_uri else chatgpt_redirects,
    )
    build_server(auth, os.environ["PATHNOTE_MCP_DATABASE_URL"]).run(
        transport="http",
        host="127.0.0.1",
        port=8017,
        stateless_http=True,
        uvicorn_config={"access_log": False, "limit_concurrency": 32},
    )


if __name__ == "__main__":
    main()
