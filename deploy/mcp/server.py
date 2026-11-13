"""Read-only MCP vocabulary checks scoped to the authenticated Pathnote account."""

import os
from hashlib import sha256
from typing import Annotated
from urllib.parse import urlsplit

import psycopg
from fastmcp import FastMCP
from fastmcp.exceptions import ToolError
from fastmcp.server.auth import AuthProvider
from fastmcp.server.auth.providers.google import GoogleProvider
from fastmcp.server.dependencies import get_access_token
from mcp.types import ToolAnnotations
from pydantic import Field, StringConstraints

Candidate = Annotated[str, StringConstraints(min_length=1, max_length=1000)]
Candidates = Annotated[list[Candidate], Field(min_length=1, max_length=50)]


def candidate_keys(words: list[str]) -> dict[str, str]:
    candidates = {}
    for word in words:
        display = " ".join(word.split())
        if not display:
            raise ToolError("Candidate words cannot be blank.")
        key = sha256(display.lower().encode()).hexdigest()
        candidates.setdefault(key, display)
    return candidates


async def known_keys(dsn: str, subject: str, keys: list[str]) -> set[str]:
    connection = await psycopg.AsyncConnection.connect(
        dsn, connect_timeout=5, options="-c statement_timeout=5000"
    )
    async with connection:
        cursor = await connection.execute(
            "SELECT word_key FROM pathnote_mcp.check_vocabulary(%s, %s)",
            (subject, keys),
        )
        rows = await cursor.fetchall()
    if not rows:
        raise ToolError("Sign in with a Google account already linked to Pathnote.")
    return {key for (key,) in rows if key is not None}


def build_server(auth: AuthProvider, dsn: str) -> FastMCP:
    server = FastMCP("Pathnote cá nhân", auth=auth)

    @server.tool(annotations=ToolAnnotations(read_only_hint=True, open_world_hint=False))
    async def check_vocabulary(words: Candidates) -> dict[str, list[str]]:
        """Check 1–50 candidate words or phrases against your active Pathnote vocabulary.

        Returns unique candidates split into new_words and known_words. Matching
        ignores letter case and repeated whitespace.
        """
        token = get_access_token()
        subject = token.subject if token else None
        if not subject:
            raise ToolError("Sign in with Google before checking Pathnote vocabulary.")
        candidates = candidate_keys(words)
        try:
            matches = await known_keys(dsn, subject, list(candidates))
        except psycopg.Error as error:
            raise ToolError("Pathnote vocabulary is temporarily unavailable.") from error
        return {
            "new_words": [word for key, word in candidates.items() if key not in matches],
            "known_words": [word for key, word in candidates.items() if key in matches],
        }

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
