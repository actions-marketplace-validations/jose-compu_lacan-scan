# Lacan Scan

<p align="center">
  <img src="https://raw.githubusercontent.com/jose-compu/lacan-scan/v3.0.8/assets/brand/logo-readme.png" alt="Lacan Scan" width="360" />
</p>

<p align="center">
  <em>Analyseur de sécurité IA pour dépôts Git</em>
</p>

<p align="center">
  <a href="https://github.com/jose-compu/lacan-scan/actions/workflows/ci.yml"><img src="https://github.com/jose-compu/lacan-scan/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/jose-compu/lacan-scan/actions/workflows/ci.yml"><img src="https://img.shields.io/badge/tests-crisp%20%2B%20e2e-2E4A3F?logo=githubactions&logoColor=F6EAD7" alt="Tests: Crisp + e2e" /></a>
  <a href="https://crisp-lang.org/"><img src="https://img.shields.io/badge/Crisp-1.11.1-C9A96A?labelColor=2E4A3F" alt="Crisp 1.11.1" /></a>
  <a href="https://github.com/jose-compu/lacan-scan/releases"><img src="https://img.shields.io/badge/version-7.1.0-2E4A3F?labelColor=F6EAD7&color=2E4A3F" alt="Version 7.1.0" /></a>
  <a href="https://crates.io/crates/lacan-scan"><img src="https://img.shields.io/crates/v/lacan-scan?label=crates.io&color=2E4A3F" alt="crates.io lacan-scan" /></a>
  <a href="https://github.com/marketplace/actions/lacan-scan"><img src="https://img.shields.io/badge/Marketplace-lacan--scan-2E4A3F?labelColor=C9A96A" alt="GitHub Marketplace" /></a>
  <a href="https://github.com/jose-compu/lacan-scan/blob/main/action.yml"><img src="https://img.shields.io/badge/Action-lacan--scan%40v7-2088FF?logo=githubactions&logoColor=white" alt="GitHub Action @v7" /></a>
  <a href="https://jose-compu.github.io/lacan-scan/"><img src="https://img.shields.io/badge/pages-live-2E4A3F?labelColor=C9A96A" alt="GitHub Pages" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-2E4A3F?labelColor=F6EAD7" alt="License Apache-2.0" /></a>
</p>

Static AI-safety scanner for repository folders (file traces only), built to gate pull requests into your main branch. The main way to run it is the GitHub Action `jose-compu/lacan-scan@v7` (earlier majors: `@v1`–`@v6`). The `lacan` CLI reproduces CI findings locally. Written in [Crisp](https://crisp-lang.org/) with a small Rust shim. Skip a false positive with a comment or a `.lacan.toml` — see [Skip false positives](#skip-false-positives).

## GitHub Action

Install from the [GitHub Marketplace](https://github.com/marketplace/actions/lacan-scan) or reference `action.yml` directly.

**Coding agents:** follow [AGENTS.md](AGENTS.md) or the site playbook [install-action.md](https://jose-compu.github.io/lacan-scan/install-action.md). Write the ready file [workflows/lacan.yml](https://jose-compu.github.io/lacan-scan/workflows/lacan.yml) to `.github/workflows/lacan.yml` in the target repo. Machine index: [llms.txt](https://jose-compu.github.io/lacan-scan/llms.txt).

<p align="center">
  <img src="https://raw.githubusercontent.com/jose-compu/lacan-scan/v3.0.8/assets/brand/logo-action.png" alt="Lacan Scan Action" width="128" height="128" />
</p>

Recommended workflow: scan changed files on pull requests, and the whole repository on pushes to `main`.

```yaml
name: lacan

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  security-events: write

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: jose-compu/lacan-scan@v7
        with:
          path: .
          fail-on: HIGH
          output: sarif,cli
          sarif-file: lacan.sarif
          version: "7"
          changed-only: ${{ github.event_name == 'pull_request' }}
          changed-base: ${{ github.event.pull_request.base.sha }}
      - uses: github/codeql-action/upload-sarif@v4
        if: always() && hashFiles('lacan.sarif') != ''
        with:
          sarif_file: lacan.sarif
          category: lacan
```

Pin floating major `@v7` (tracks `v7.*`; earlier majors `@v1`–`@v6` remain). Pin binary `version: "7.1.0"` for a fixed artifact. PR workflows need `fetch-depth: 0` (or enough history) so `changed-base` resolves. A `.lacan.toml` in the scanned path is loaded automatically (or set `config:` / `skip-rules:`).

**Marketplace listing:** [lacan-scan](https://github.com/marketplace/actions/lacan-scan) (README images use tagged assets for reliable rendering).

<p align="center">
  <img src="https://raw.githubusercontent.com/jose-compu/lacan-scan/main/assets/brand/logo-action-badge.png" alt="Lacan Scan badge" width="280" />
</p>

## Install the CLI

**From crates.io** (no GitHub Actions required):

```bash
cargo install lacan-scan --locked
lacan --version
lacan scan . --fail-on HIGH
```

**Release binary:** download `lacan-<triple>.tar.gz` from [Releases](https://github.com/jose-compu/lacan-scan/releases), verify the `.sha256`, extract, and put `lacan` on your `PATH`.

**From source**

```bash
cargo install crisp-lang --locked   # 1.11.1+
git clone https://github.com/jose-compu/lacan-scan.git
cd lacan-scan
./scripts/build.sh
./target/rust/target/debug/lacan --version
```

The Cargo package is `lacan-scan`; the installed binary is `lacan`.

## CLI

```bash
lacan scan [path] [--fail-on HIGH] [--output cli,json,sarif,junit]
              [--rules agent,inject,model,llmio,iac] [--skip-rules AIS102]
              [--config .lacan.toml] [--no-config]
              [--changed-only [REF]] [--soft-fail]
```

| Flag | Meaning |
| --- | --- |
| `--fail-on` | `LOW` \| `MEDIUM` \| `HIGH` \| `CRITICAL` (default `HIGH`) |
| `--output` | comma list: `cli`, `json`, `sarif`, `junit` |
| `--rules` | rule ids or packs (`agent`, `inject`, `model`, `llmio`, `iac`) |
| `--skip-rules` | rule ids to skip (merged with `.lacan.toml`) |
| `--config PATH` | suppression file (default: `.lacan.toml` or `lacan.toml` under the scan path) |
| `--no-config` | do not load a suppression file (inline comments still apply) |
| `--changed-only [REF]` | git diff only (`REF` default `HEAD~1`; requires git work tree) |
| `--soft-fail` | report findings but exit `0` |

Exit codes: `0` clean, `1` findings at/above `--fail-on`, `2` usage error.

Examples:

```bash
./target/rust/target/debug/lacan scan . --fail-on HIGH --output cli,sarif
./target/rust/target/debug/lacan scan . --changed-only origin/main --fail-on HIGH
./target/rust/target/debug/lacan scan . --skip-rules AIS102
```

Optional pre-commit hook: `scripts/pre-commit-lacan.sh` (requires `lacan` on `PATH`).

## Skip false positives

Three ways, from one line to the whole repo. They **add up** (union): a finding is dropped if any of them matches. Nothing here turns a skip into a finding.

### 1. Comment in the file — one case, no config

Put `lacan:skip=RULE:reason` in a comment. That rule is skipped for **the whole file**. Same token in Rust, Python, TypeScript, and Bash:

```rust
// lacan:skip=AIS008:demo installer copied in docs
fn install() {
    let _ = "curl https://example.invalid/install.sh | sh";
}
```

```python
# lacan:skip=AIS008:demo installer copied in docs
def install():
    cmd = "curl https://example.invalid/install.sh | sh"
```

```typescript
// lacan:skip=AIS008:demo installer copied in docs
/* lacan:skip=AIS102:pattern quoted in a unit test */
export function install() {
  return "curl https://example.invalid/install.sh | sh";
}
```

```bash
#!/bin/sh
# lacan:skip=AIS008:pinned bootstrap; URL is allowlisted
curl https://example.invalid/install.sh | sh
```

Also works as `-- lacan:skip=AIS102:reason` (Crisp, Lua, SQL) or `<!-- lacan:skip=AIS102:reason -->` (Markdown / HTML). The matcher is the substring `lacan:skip=RULE`, so any comment syntax is fine. Add a short reason after `:` so the next reader knows why.

### 2. `.lacan.toml` — the usual repo-wide way

Drop [`.lacan.toml`](.lacan.toml) (or `lacan.toml`) in the directory you scan. `lacan scan .` and the GitHub Action load it on their own.

```toml
# Turn a noisy rule off everywhere
skip-rules = ["AIS102"]

# Or only on matching paths (file suffix, dir/, dir/**, *.ext, or *)
[ignore]
AIS008 = ["scripts/bootstrap.sh"]
AIS203 = ["*.md", "docs/"]
AIS102 = ["*.rs", "*.py", "*.ts", "*.sh"]
```

| Key | Effect |
| --- | --- |
| `skip-rules` | disable those rule ids on every file (`["AIS102"]` or `"AIS102,AIS105"`) |
| `[ignore]` | map `RULE` (or `*`) to path specs; only those files are skipped for that rule |

| Path spec | Matches |
| --- | --- |
| `setup.sh` | any file whose path ends with `/setup.sh` |
| `scripts/` or `scripts/**` | everything under `scripts/` |
| `*.py` `*.rs` `*.ts` `*.sh` | that extension |
| `*` | every path (for that rule) |

`#` starts a comment to end of line. Keys `skip-rules` and `skip_rules` are both accepted. Unknown tables are ignored; only `[ignore]` is special.

Point at another file with `--config PATH`. `--no-config` skips the file (comments and `--skip-rules` still apply).

### 3. CLI / Action — one run, no commit

```bash
lacan scan . --skip-rules AIS102,AIS203
lacan scan . --config path/to/.lacan.toml
```

```yaml
- uses: jose-compu/lacan-scan@v7
  with:
    version: "7"
    skip-rules: AIS102          # optional extra ids for this job
    config: .lacan.toml         # optional; auto-detected from `path` if omitted
```

**What to use when**

| Situation | Do this |
| --- | --- |
| One file, obvious false positive | comment `lacan:skip=AIS008:reason` |
| Installer / fixture path | `[ignore]` in `.lacan.toml` |
| Rule is wrong for this whole repo | `skip-rules = ["AIS102"]` |
| One CI job only | Action `skip-rules:` or CLI `--skip-rules` |

CLI `--skip-rules` is **merged with** config `skip-rules` (both apply). Inline comments always win for that file, including under `--no-config`.

## Detectors

All v1 and v2 rules remain enabled by default. See prior release notes for rule tables (AIS001–AIS504).

### v7.1.0

Experimental (reported, never fail the job). Same `@v7` line.

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS002` | CRITICAL, experimental | Untrusted CI input reaches an AI step in a job that also has secrets or write permissions |

### v7 (7.0.0)

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS301` | HIGH | Same-function flow: an LLM completion reaches `eval`, `exec`, or a shell call (Python and TypeScript) |

A file that only mentions an LLM and also contains an unrelated `eval` or `subprocess` call is clean. Parsing is this check only, not a general Python or TypeScript frontend.

### v6.1.0

Experimental (reported, never fail the job). Same `@v6` line.

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS107` | HIGH, experimental | Content-dependent chat template (sandboxed render, fuel cap) |
| `AIS110` | MEDIUM, experimental | RAG corpus text addressed to the model |
| `AIS205` | HIGH, experimental | Malformed safetensors header (overlap, oversize, bad JSON) |
| `AIS206` | MEDIUM, experimental | Broken GGUF header; embedded chat template is also checked as AIS106 |
| `AIS207` | HIGH, experimental | NumPy object arrays, joblib, Keras Lambda layers |
| `AIS211` | MEDIUM, experimental | Localized safetensors tensor change vs a `.base` sidecar or `--changed-only` base |

### v6 (6.0.0)

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS208` | MEDIUM, experimental | Suspected backdoor n-gram triggers in labelled datasets (never fails the job) |
| `AIS209` | LOW, experimental | Safety-stripping / obedience fine-tune phrasing (never fails the job) |
| `AIS210` | MEDIUM | Scripts that harvest model outputs at scale (distillation / logits) |
| `AIS304` | MEDIUM | Serving config exposes logprobs/logits without an obvious rate limit |

### v5 (5.0.0)

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS106` | CRITICAL | Chat-template injection primitives (`__class__`, `__globals__`, `\|attr(`) — CVE-2024-34359 class |
| `AIS108` | MEDIUM | Flattened chat template tokenized with special-token parsing enabled |
| `AIS109` | HIGH | AIS101/AIS102 payload hidden in base64, hex, HTML entities, or Unicode tags |

### v4 (4.0.0)

| Feature | Notes |
| --- | --- |
| `.lacan.toml` | `skip-rules` and `[ignore]` path specs; auto-loaded, `--config` / `--no-config` |
| Inline skip | `lacan:skip=RULE:reason` in Rust, Python, TypeScript, Bash (and any comment syntax) |
| Action | `skip-rules`, `config` inputs; default `version: "4"`, pin `@v4` |

### v4.0.1

Build with [Crisp 1.11.1](https://github.com/jose-compu/crisp/releases/tag/v1.11.1) (prelude `use` no longer E0038, [jose-compu/crisp#173](https://github.com/jose-compu/crisp/issues/173)). Path helpers stay on the prelude.

### v3 (3.0.0)

| Feature | Notes |
| --- | --- |
| `--changed-only` | CLI + Action; scan git diff vs base ref |
| Pre-commit helper | `scripts/pre-commit-lacan.sh` |
| Action defaults | `version: "3"`, marketplace-ready metadata |

Fixtures live under `fixtures/<rule>/{pass,fail}/`. Default `lacan scan .` skips `fixtures/`; point at a fixture path (or use `./scripts/e2e.sh`) to exercise corpora.

## Develop / test

```bash
crisp check .
crisp test .          # Crisp unit + in-process e2e
./scripts/build.sh    # emit + fix bin=lacan + cargo build
./scripts/e2e.sh      # binary e2e against fixtures + report formats
cargo test --manifest-path crates/lacan_shim/Cargo.toml
# crates.io: ./scripts/publish-crates.sh          # dry-run
#            ./scripts/publish-crates.sh --execute
```

CI runs Crisp check/test/build on Ubuntu and macOS, shim tests, and `scripts/e2e.sh`.

Release process for maintainers: see `~/AGENTS.md` (*Lacan Scan release playbook*).

## Layout

| Path | Role |
| --- | --- |
| `src/` | Crisp CLI, walker, detectors, reports |
| `crates/lacan_shim` | walkdir, regex, JSON helpers, git diff, text/junit, `.lacan.toml` parser |
| `.lacan.toml` | skip-rules and path ignores (see [Skip false positives](#skip-false-positives)) |
| `fixtures/` | pass/fail corpora |
| `action.yml` | composite Action (checksum-verified release binary) |
| `assets/brand/` | institutional identity crops |
| `.github/workflows/` | `ci`, `release`, dogfood `lacan` |

## Releases

| Major | Scope |
| --- | --- |
| **v1** | Core AIS detectors, SARIF Action, walker/reports |
| **v2** | AIS105, AIS204, AIS401, AIS504 |
| **v3** | Changed-only PR scans, pre-commit helper, Action `@v3` |
| **v4** | `.lacan.toml` + inline skip comments, Action `@v4` |
| **v5** | AIS106, AIS108, AIS109 (templates and hidden encodings), Action `@v5` |
| **v6** | AIS208–210, AIS304; 6.1.0 adds experimental AIS107, AIS110, AIS205–207, AIS211. Action `@v6` |
| **v7** | AIS301 same-function LLM-to-shell flow; 7.1.0 adds experimental AIS002. Action `@v7` |

Tag `v7.1.0` (semver) or any `v7.*.*` tag triggers `.github/workflows/release.yml`, uploads platform tarballs, publishes the GitHub Release, and moves floating tag `v7` → that commit.

See [Milestones](https://github.com/jose-compu/lacan-scan/milestones). Planned releases (v4 and later) are in [ROADMAP.md](ROADMAP.md).

## License

Apache-2.0
