#!/usr/bin/env bash
# After `crisp emit`, rewrite target/rust/Cargo.toml:
# - keep crates.io package name lacan-scan (from crisp.toml)
# - force [[bin]] name = lacan (CLI on PATH)
# - optional publish metadata + versioned lacan_shim dep
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="${root}/target/rust/Cargo.toml"
for_publish=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --for-publish) for_publish=1; shift ;;
    -h|--help)
      echo "usage: $0 [--for-publish]" >&2
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$manifest" ]]; then
  echo "missing $manifest (run: crisp emit .)" >&2
  exit 2
fi

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/crisp.toml" | head -1)"
if [[ -z "$version" ]]; then
  echo "could not read version from crisp.toml" >&2
  exit 2
fi

python3 - "$manifest" "$version" "$for_publish" <<'PY'
import pathlib, re, sys

path = pathlib.Path(sys.argv[1])
version = sys.argv[2]
for_publish = sys.argv[3] == "1"
text = path.read_text()

# Force CLI binary name to lacan (package remains lacan-scan).
text = re.sub(
    r'(\[\[bin\]\]\s*\nname = ")[^"]+(")',
    r'\1lacan\2',
    text,
    count=1,
)

meta = f'''description = "Static AI-safety scanner for Git repositories (CLI: lacan)"
license = "Apache-2.0"
repository = "https://github.com/jose-compu/lacan-scan"
homepage = "https://jose-compu.github.io/lacan-scan/"
documentation = "https://docs.rs/lacan-scan"
readme = "README.md"
keywords = ["security", "ai", "scanner", "sarif", "sast"]
categories = ["command-line-utilities", "development-tools"]
'''

if "description =" not in text:
    text = text.replace(
        f'version = "{version}"\nedition = "2021"\n',
        f'version = "{version}"\nedition = "2021"\n{meta}',
        1,
    )

if for_publish:
    # Prefer registry lacan_shim once published; keep path for local verify.
    text = re.sub(
        r'lacan_shim = \{[^}]+\}',
        f'lacan_shim = {{ version = "{version}", path = "../../crates/lacan_shim" }}',
        text,
        count=1,
    )
    if "include =" not in text:
        text = text.replace(
            "[workspace]\n",
            'include = ["src/**/*", "README.md", "LICENSE"]\n\n[workspace]\n',
            1,
        )

path.write_text(text)
print(f"fixed {path} (bin=lacan, version={version}, for_publish={for_publish})")
PY
