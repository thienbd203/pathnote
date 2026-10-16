#!/usr/bin/env python3
"""Resolve one reviewed frontend/backend pair; never resolve moving branch tips."""
import datetime
import json
import os
from pathlib import Path
import re
import sys

FRONTEND_REPOSITORY = "thienbd203/pathnote-web"


def resolve(path: Path, backend_sha: str, run_id: str, attempt: str) -> dict[str, str]:
    manifest = json.loads(path.read_text())
    if not isinstance(manifest, dict) or set(manifest) != {"frontend_repository", "frontend_sha"}:
        raise ValueError("Release manifest must contain only frontend_repository and frontend_sha")
    if manifest["frontend_repository"] != FRONTEND_REPOSITORY:
        raise ValueError("Unexpected frontend repository")
    for value in (backend_sha, manifest["frontend_sha"]):
        if not isinstance(value, str) or not re.fullmatch(r"[a-f0-9]{40}", value):
            raise ValueError("Both release revisions must be complete lowercase commit SHAs")
    if not re.fullmatch(r"[0-9]+", run_id) or not re.fullmatch(r"[0-9]+", attempt):
        raise ValueError("Invalid workflow run identifier")
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d%H%M%S")
    return {
        **manifest,
        "backend_sha": backend_sha,
        "release_id": f"{stamp}-{run_id}-{attempt}",
    }


def main() -> None:
    release = resolve(Path(sys.argv[1]), os.environ["GITHUB_SHA"],
                      os.environ["GITHUB_RUN_ID"], os.environ["GITHUB_RUN_ATTEMPT"])
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for key, value in release.items():
            output.write(f"{key}={value}\n")
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as summary:
        summary.write("## Selected release pair\n\n")
        summary.write(f"- Backend: `{release['backend_sha']}`\n")
        summary.write(f"- Frontend: `{release['frontend_sha']}`\n\n")
        summary.write("Both builds must succeed before backend deployment. Frontend deployment also requires a healthy backend of this exact version.\n")


if __name__ == "__main__":
    main()
