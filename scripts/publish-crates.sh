#!/usr/bin/env bash
# Publish lacan_shim then lacan-scan to crates.io.
# Default: dry-run. Pass --execute to upload.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

execute=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute) execute=1; shift ;;
    -h|--help)
      echo "usage: $0 [--execute]" >&2
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' crisp.toml | head -1)"
echo "version=$version"

publish_flags=(--locked)
if [[ "$execute" -eq 0 ]]; then
  publish_flags+=(--dry-run --allow-dirty)
  echo "mode=dry-run (pass --execute to upload)"
else
  publish_flags+=(--allow-dirty)
  echo "mode=EXECUTE — uploading to crates.io"
fi

echo "== publish lacan_shim =="
(
  cd crates/lacan_shim
  cargo publish "${publish_flags[@]}"
)

echo "== crisp emit + stage lacan-scan =="
crisp emit .
chmod +x scripts/fix-emitted-cargo.sh
./scripts/fix-emitted-cargo.sh --for-publish

stage="$(mktemp -d "${TMPDIR:-/tmp}/lacan-scan-crate.XXXXXX")"
cleanup() { rm -rf "$stage"; }
trap cleanup EXIT

mkdir -p "$stage/src"
cp -R target/rust/src/. "$stage/src/"
cp target/rust/Cargo.toml "$stage/Cargo.toml"
# Path dep must resolve from the staged crate root during dry-run/package.
python3 - "$stage/Cargo.toml" "$root" "$version" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
root = pathlib.Path(sys.argv[2]).resolve()
version = sys.argv[3]
shim = root / "crates" / "lacan_shim"
text = path.read_text()
text = re.sub(
    r'lacan_shim = \{[^}]+\}',
    f'lacan_shim = {{ version = "{version}", path = "{shim.as_posix()}" }}',
    text,
    count=1,
)
path.write_text(text)
PY
cp LICENSE "$stage/LICENSE"
# Short crate README for crates.io (full docs stay in repo README).
cat > "$stage/README.md" <<EOF
# lacan-scan

Static AI-safety scanner for Git repositories. Installs the \`lacan\` CLI.

\`\`\`bash
cargo install lacan-scan --locked
lacan scan . --fail-on HIGH
\`\`\`

Docs: https://github.com/jose-compu/lacan-scan  
Pages: https://jose-compu.github.io/lacan-scan/
EOF

# Drop empty [workspace] so cargo does not treat this as a virtual workspace root issue.
# Keep [workspace] as empty table — fine for a single package publish.

echo "== cargo package check =="
cargo package --manifest-path "$stage/Cargo.toml" --list --allow-dirty | tee /tmp/lacan-scan-package.list
grep -E 'src/main\.rs|Cargo\.toml|LICENSE|README\.md' /tmp/lacan-scan-package.list >/dev/null

echo "== publish lacan-scan =="
shim_path="$(cd crates/lacan_shim && pwd)"

if [[ "$execute" -eq 1 ]]; then
  echo "waiting for crates.io to index lacan_shim ${version}..."
  for i in $(seq 1 60); do
    if cargo info "lacan_shim@${version}" >/dev/null 2>&1; then
      echo "lacan_shim ${version} visible"
      break
    fi
    sleep 5
    if [[ "$i" -eq 60 ]]; then
      echo "timeout waiting for lacan_shim@${version}" >&2
      exit 1
    fi
  done
  # Registry dep only — cargo publish rejects path once the crate is public.
  python3 - "$stage/Cargo.toml" "$version" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
version = sys.argv[2]
text = re.sub(
    r'lacan_shim = \{[^}]+\}',
    f'lacan_shim = "{version}"',
    path.read_text(),
    count=1,
)
path.write_text(text)
PY
  cargo publish --manifest-path "$stage/Cargo.toml" --locked --allow-dirty
else
  # Path-only so packaging works before lacan_shim exists on crates.io.
  python3 - "$stage/Cargo.toml" "$shim_path" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
shim = sys.argv[2]
text = re.sub(
    r'lacan_shim = \{[^}]+\}',
    f'lacan_shim = {{ path = "{shim}" }}',
    path.read_text(),
    count=1,
)
path.write_text(text)
PY
  cargo package --manifest-path "$stage/Cargo.toml" --allow-dirty
  echo "packaged + verified ok (dry-run; pass --execute to upload)"
fi

echo "done. install with: cargo install lacan-scan --locked"
