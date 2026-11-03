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
  <a href="https://crisp-lang.org/"><img src="https://img.shields.io/badge/Crisp-1.11-C9A96A?labelColor=2E4A3F" alt="Crisp 1.11" /></a>
  <a href="https://github.com/jose-compu/lacan-scan/releases"><img src="https://img.shields.io/badge/version-3.0.5-2E4A3F?labelColor=F6EAD7&color=2E4A3F" alt="Version 3.0.5" /></a>
  <a href="https://github.com/marketplace/actions/lacan-scan"><img src="https://img.shields.io/badge/Marketplace-lacan--scan-2E4A3F?labelColor=C9A96A" alt="GitHub Marketplace" /></a>
  <a href="https://github.com/jose-compu/lacan-scan/blob/main/action.yml"><img src="https://img.shields.io/badge/Action-lacan--scan%40v3-2088FF?logo=githubactions&logoColor=white" alt="GitHub Action @v3" /></a>
  <a href="https://jose-compu.github.io/lacan-scan/"><img src="https://img.shields.io/badge/pages-live-2E4A3F?labelColor=C9A96A" alt="GitHub Pages" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-2E4A3F?labelColor=F6EAD7" alt="License Apache-2.0" /></a>
</p>

Static AI-safety scanner for repository folders (file traces only). Written in [Crisp](https://crisp-lang.org/) with a small Rust shim. Ship as CLI `lacan` and GitHub Action `jose-compu/lacan-scan@v3` (earlier majors: `@v1`, `@v2`).

## Install

**Release binary:** download `lacan-<triple>.tar.gz` from [Releases](https://github.com/jose-compu/lacan-scan/releases), verify the `.sha256`, extract, and put `lacan` on your `PATH`.

**From source**

```bash
cargo install crisp-lang --locked   # 1.11+
git clone https://github.com/jose-compu/lacan-scan.git
cd lacan-scan
crisp build .
./target/rust/target/debug/lacan --version
```

## CLI

```bash
lacan scan [path] [--fail-on HIGH] [--output cli,json,sarif,junit]
              [--rules agent,inject,model,llmio,iac] [--skip-rules AIS102]
              [--changed-only [REF]] [--soft-fail]
```

| Flag | Meaning |
| --- | --- |
| `--fail-on` | `LOW` \| `MEDIUM` \| `HIGH` \| `CRITICAL` (default `HIGH`) |
| `--output` | comma list: `cli`, `json`, `sarif`, `junit` |
| `--rules` | rule ids or packs (`agent`, `inject`, `model`, `llmio`, `iac`) |
| `--skip-rules` | rule ids to skip |
| `--changed-only [REF]` | git diff only (`REF` default `HEAD~1`; requires git work tree) |
| `--soft-fail` | report findings but exit `0` |

Exit codes: `0` clean, `1` findings at/above `--fail-on`, `2` usage error.

Inline skip: `# lacan:skip=AIS008:reason`

Examples:

```bash
./target/rust/target/debug/lacan scan . --fail-on HIGH --output cli,sarif
./target/rust/target/debug/lacan scan . --changed-only origin/main --fail-on HIGH
```

Optional pre-commit hook: `scripts/pre-commit-lacan.sh` (requires `lacan` on `PATH`).

## GitHub Action

Install from the [GitHub Marketplace](https://github.com/marketplace/actions/lacan-scan) or reference `action.yml` directly.

<p align="center">
  <img src="https://raw.githubusercontent.com/jose-compu/lacan-scan/v3.0.8/assets/brand/logo-action.png" alt="Lacan Scan Action" width="128" height="128" />
</p>

```yaml
- uses: jose-compu/lacan-scan@v3
  with:
    path: .
    fail-on: HIGH
    output: sarif
    sarif-file: lacan.sarif
    version: "3"
    changed-only: "true"
    changed-base: ${{ github.event.pull_request.base.sha }}
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: lacan.sarif
    category: lacan
```

Pin floating major `@v3` (tracks `v3.*`). Pin binary `version: "3.0.5"` for a fixed artifact. PR workflows need `fetch-depth: 0` (or enough history) so `changed-base` resolves.

**Marketplace listing:** [lacan-scan](https://github.com/marketplace/actions/lacan-scan) (README images use tagged assets for reliable rendering).

<p align="center">
  <img src="https://raw.githubusercontent.com/jose-compu/lacan-scan/v3.0.8/assets/brand/logo-action-badge.png" alt="Lacan Scan badge" width="280" />
</p>

## Detectors

All v1 and v2 rules remain enabled by default. See prior release notes for rule tables (AIS001–AIS504).

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
crisp build .
./scripts/e2e.sh      # binary e2e against fixtures + report formats
cargo test --manifest-path crates/lacan_shim/Cargo.toml
```

CI runs Crisp check/test/build on Ubuntu and macOS, shim tests, and `scripts/e2e.sh`.

Release process for maintainers: see `~/AGENTS.md` (*Lacan Scan release playbook*).

## Layout

| Path | Role |
| --- | --- |
| `src/` | Crisp CLI, walker, detectors, reports |
| `crates/lacan_shim` | walkdir, regex, JSON helpers, git diff, text/junit |
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

Tag `v3.0.8` (semver patch) or any `v3.*.*` tag triggers `.github/workflows/release.yml`, uploads platform tarballs, publishes the GitHub Release, and moves floating tag `v3` → that commit.

See [Milestones](https://github.com/jose-compu/lacan-scan/milestones).

## License

Apache-2.0
