#!/usr/bin/env python3
"""Verify that the serving process is the requested build, without logging response bodies."""
import json
import sys
import urllib.request
from urllib.parse import urlsplit


def check(url: str, expected_version: str) -> None:
    parsed = urlsplit(url)
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost"}
    ):
        raise ValueError("Healthcheck requires HTTPS or a local HTTP address")
    request = urllib.request.Request(url, headers={"Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=10) as response:
        body = response.read(4097)
    if len(body) > 4096:
        raise ValueError("Health response too large")
    data = json.loads(body).get("data", {})
    if not (
        data.get("status") == "ok"
        and data.get("database") == "ok"
        and data.get("version") == expected_version
    ):
        raise ValueError("Running release or database health did not match")


if __name__ == "__main__":
    try:
        check(sys.argv[1], sys.argv[2])
    except Exception:
        print("Healthcheck failed: process version or database is not ready", file=sys.stderr)
        sys.exit(1)
