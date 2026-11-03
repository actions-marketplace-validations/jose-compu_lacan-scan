#!/usr/bin/env bash
# Optional pre-commit: scan files changed since HEAD (staged + unstaged vs index not supported; use CI changed-base for PRs).
set -euo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "$root" ]]; then
  exit 0
fi

bin="${LACAN_BIN:-lacan}"
if ! command -v "$bin" >/dev/null 2>&1; then
  echo "lacan: skip pre-commit (binary not on PATH; set LACAN_BIN)" >&2
  exit 0
fi

exec "$bin" scan "$root" --changed-only HEAD --fail-on HIGH --output cli
