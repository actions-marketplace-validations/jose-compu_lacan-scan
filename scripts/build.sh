#!/usr/bin/env bash
# Emit Crisp → fix Cargo package/bin → cargo build (debug by default).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *)
    echo "usage: $0 [debug|release]" >&2
    exit 2
    ;;
esac

echo "== crisp emit =="
crisp emit .

echo "== fix emitted Cargo.toml =="
chmod +x scripts/fix-emitted-cargo.sh
./scripts/fix-emitted-cargo.sh

echo "== cargo build ($profile) =="
if [[ "$profile" == "release" ]]; then
  cargo build --release --manifest-path target/rust/Cargo.toml
  bin="target/rust/target/release/lacan"
else
  cargo build --manifest-path target/rust/Cargo.toml
  bin="target/rust/target/debug/lacan"
fi

test -x "$bin"
"$bin" --version
echo "binary: $bin"
