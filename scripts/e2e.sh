#!/usr/bin/env bash
# End-to-end checks against the built lacan binary and fixtures/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

bin="${LACAN_BIN:-target/rust/target/debug/lacan}"
if [[ ! -x "$bin" ]]; then
  echo "missing binary: $bin (run: ./scripts/build.sh)" >&2
  exit 2
fi

echo "== version =="
"$bin" --version | tee /tmp/lacan-e2e-version.txt
grep -F 'lacan 6.1.0' /tmp/lacan-e2e-version.txt

echo "== help =="
help_out="$("$bin" --help)"
printf '%s\n' "$help_out" | grep -F 'lacan scan' >/dev/null

echo "== fail fixtures (expect rc=1; experimental rules report but rc=0) =="
fail_rc=0
for dir in fixtures/*/fail; do
  [[ -d "$dir" ]] || continue
  echo "--- $dir ---"
  rule="$(basename "$(dirname "$dir")")"
  set +e
  out="$("$bin" scan "$dir" --fail-on LOW --output cli 2>&1)"
  rc=$?
  set -e
  printf '%s\n' "$out"
  # Experimental rules must appear in output but never fail the job.
  if [[ "$rule" == "AIS107" || "$rule" == "AIS110" || "$rule" == "AIS205" || "$rule" == "AIS206" || "$rule" == "AIS207" || "$rule" == "AIS208" || "$rule" == "AIS209" || "$rule" == "AIS211" ]]; then
    if [[ "$rc" -ne 0 ]]; then
      echo "expected rc=0 for experimental $dir, got $rc" >&2
      fail_rc=1
    fi
    if ! printf '%s\n' "$out" | grep -q "$rule"; then
      echo "expected $rule in output for $dir" >&2
      fail_rc=1
    fi
  elif [[ "$rc" -ne 1 ]]; then
    echo "expected rc=1 for $dir, got $rc" >&2
    fail_rc=1
  fi
done

echo "== pass fixtures (expect rc=0) =="
for dir in fixtures/*/pass; do
  [[ -d "$dir" ]] || continue
  echo "--- $dir ---"
  set +e
  "$bin" scan "$dir" --fail-on LOW --output cli
  rc=$?
  set -e
  if [[ "$rc" -ne 0 ]]; then
    echo "expected rc=0 for $dir, got $rc" >&2
    fail_rc=1
  fi
done

echo "== report formats =="
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
"$bin" scan fixtures/AIS008/fail --fail-on HIGH --output json,sarif,junit --soft-fail \
  --json-file "$tmp/out.json" --sarif-file "$tmp/out.sarif" --junit-file "$tmp/out.xml"
test -s "$tmp/out.json"
test -s "$tmp/out.sarif"
test -s "$tmp/out.xml"
grep -q AIS008 "$tmp/out.json"
grep -q AIS008 "$tmp/out.sarif"
grep -q AIS008 "$tmp/out.xml"

echo "== soft-fail exit 0 with findings =="
set +e
"$bin" scan fixtures/AIS008/fail --fail-on HIGH --output cli --soft-fail
rc=$?
set -e
test "$rc" -eq 0

echo "== suppressions (.lacan.toml + comments) =="
set +e
"$bin" scan fixtures/config-skip/ignored --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 0
set +e
"$bin" scan fixtures/config-skip/ignored --no-config --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 1
set +e
"$bin" scan fixtures/config-skip/skip-rules --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 0
set +e
"$bin" scan fixtures/AIS008/fail --config fixtures/config-skip/skip-rules/.lacan.toml --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 0
set +e
"$bin" scan fixtures/skip-comments/pass --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 0
help_out="$("$bin" --help)"
printf '%s\n' "$help_out" | grep -F '.lacan.toml' >/dev/null
printf '%s\n' "$help_out" | grep -F 'lacan:skip' >/dev/null

echo "== pack filter =="
set +e
"$bin" scan fixtures/AIS501/fail --rules agent --fail-on LOW --output cli
rc=$?
set -e
test "$rc" -eq 0

if [[ "$fail_rc" -ne 0 ]]; then
  echo "e2e fixture matrix failed" >&2
  exit 1
fi

if git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "== changed-only smoke (HEAD~1) =="
  set +e
  "$bin" scan "$root" --changed-only HEAD~1 --fail-on HIGH --output cli --soft-fail
  rc=$?
  set -e
  if [[ "$rc" -ne 0 ]]; then
    echo "changed-only smoke failed rc=$rc" >&2
    exit 1
  fi
fi

echo "e2e ok"
