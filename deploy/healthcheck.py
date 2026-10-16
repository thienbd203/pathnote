#!/usr/bin/env python3
"""Verify that the serving process is the requested build, without logging response bodies."""
import json
import sys
import urllib.error
import urllib.request
from urllib.parse import urlsplit


def check(url: str, expected_version: str, service: str = "backend") -> None:
    if service not in {"backend", "collaboration"}:
        raise ValueError("Unsupported healthcheck service")
    parsed = urlsplit(url)
    if service == "collaboration" and (parsed.scheme != "https" or parsed.path != "/up"):
        raise ValueError("Collaboration healthcheck requires HTTPS /up")
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost"}
    ):
        raise ValueError("Healthcheck requires HTTPS or a local HTTP address")
    request = urllib.request.Request(url, headers={"Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=10) as response:
        body = response.read(4097)
        if len(body) > 4096:
            raise ValueError("Health response too large")
        if response.geturl() != url:
            raise ValueError("Health endpoint must not redirect")
    payload = json.loads(body)
    data = payload if service == "collaboration" else payload.get("data", {})
    if data.get("status") != "ok" or (service == "backend" and data.get("database") != "ok"):
        raise ValueError("API or database status is not healthy")
    if data.get("version") != expected_version:
        raise ValueError("Running process version does not match the deployed commit")


if __name__ == "__main__":
    try:
        check(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else "backend")
    except urllib.error.HTTPError as error:
        print(f"Healthcheck failed: HTTP status {error.code}", file=sys.stderr)
        sys.exit(1)
    except (urllib.error.URLError, TimeoutError, OSError):
        print("Healthcheck failed: could not connect, resolve host or verify TLS", file=sys.stderr)
        sys.exit(1)
    except ValueError as error:
        # JSON parser diagnostics can contain response text. Do not log bodies.
        message = "Response is not valid JSON" if isinstance(error, json.JSONDecodeError) else str(error)
        print(f"Healthcheck failed: {message}", file=sys.stderr)
        sys.exit(1)
    except Exception:
        print("Healthcheck failed: unexpected response format or checker error", file=sys.stderr)
        sys.exit(1)
