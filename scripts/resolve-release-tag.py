#!/usr/bin/env python3
"""Resolve a lacan-scan GitHub release tag from version input (major, semver, latest)."""
from __future__ import annotations

import json
import os
import re
import sys
import urllib.request


REPO = "jose-compu/lacan-scan"
SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$")
MAJOR = re.compile(r"^[0-9]+$")


def fetch_releases(token: str | None) -> list[dict]:
    url = f"https://api.github.com/repos/{REPO}/releases?per_page=100"
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.load(resp)


def tarball_assets(release: dict) -> list[dict]:
    out = []
    for asset in release.get("assets") or []:
        name = asset.get("name") or ""
        if name.endswith(".tar.gz") and not name.endswith(".sha256"):
            out.append(asset)
    return out


def resolve_major(major: str, rels: list[dict]) -> str:
    floating = f"v{major}"
    semver = re.compile(
        rf"^v{re.escape(major)}\.(\d+)\.(\d+)(?:[.-][A-Za-z0-9.-]+)?$"
    )
    ranked: list[tuple[int, int, str]] = []
    for rel in rels:
        if rel.get("draft") or rel.get("prerelease"):
            continue
        tag = rel.get("tag_name") or ""
        if not tarball_assets(rel):
            continue
        m = semver.match(tag)
        if m:
            ranked.append((int(m.group(1)), int(m.group(2)), tag))
    if ranked:
        ranked.sort(reverse=True)
        return ranked[0][2]
    for rel in rels:
        if rel.get("draft") or rel.get("prerelease"):
            continue
        if (rel.get("tag_name") or "") == floating and tarball_assets(rel):
            return floating
    return ""


def valid_tag(tag: str) -> bool:
    return bool(re.match(r"^v[0-9]+(\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?)?$", tag))


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: resolve-release-tag.py VERSION", file=sys.stderr)
        return 2
    raw = sys.argv[1].strip()
    version = raw[1:] if raw.startswith("v") else raw
    token = os.environ.get("GH_TOKEN") or None

    rels = fetch_releases(token)
    tag = ""
    if version == "latest":
        req = urllib.request.Request(
            f"https://api.github.com/repos/{REPO}/releases/latest",
            headers={
                "Accept": "application/vnd.github+json",
                **({"Authorization": f"Bearer {token}"} if token else {}),
            },
        )
        with urllib.request.urlopen(req, timeout=60) as resp:
            tag = json.load(resp)["tag_name"]
    elif MAJOR.match(version):
        tag = resolve_major(version, rels)
    elif SEMVER.match(version):
        tag = f"v{version}"
    else:
        print(
            f"invalid version input (expected major like '1', semver, or 'latest'): {raw}",
            file=sys.stderr,
        )
        return 1

    if not tag or not valid_tag(tag):
        print(f"failed to resolve a valid release tag from version={raw}", file=sys.stderr)
        return 1
    print(tag)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
