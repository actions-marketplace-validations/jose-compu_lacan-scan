# Lacan Scan

Static AI-safety scanner for repository folders. Rules follow the [AISB](https://github.com/jose-compu/aisb) curriculum (file traces only). Written in [Crisp](https://crisp-lang.org/) with a small Rust shim for walk/regex/helpers.

<p align="center">
  <img src="assets/lacan-scan-logo.png" alt="Lacan Scan logo" width="192" height="192" />
</p>

## CLI

```bash
cargo install crisp-lang --locked   # 1.11+
crisp build .
./target/rust/target/debug/lacan scan . --fail-on HIGH --output cli,sarif
```

```
lacan scan [path] [--fail-on HIGH] [--output cli,json,sarif,junit]
              [--rules agent,inject,model,llmio,iac] [--skip-rules AIS102]
              [--soft-fail]
```

Exit codes: `0` clean, `1` findings at/above `--fail-on`, `2` usage error.

Inline skip: `# lacan:skip=AIS008:reason`

## GitHub Action

Use the major tag so you track patch releases automatically:

```yaml
- uses: jose-compu/lacan-scan@v1
  with:
    path: .
    fail-on: HIGH
    output: sarif
    sarif-file: lacan.sarif
    version: "1"          # or "1.0.0" / "latest"
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: lacan.sarif
    category: lacan
```

Release tags are `v1.0.0`, `v1.0.1`, … with floating majors `v1`, `v2`, `v3` for the Action pin.

## Layout

- `src/` — Crisp CLI, walker, detectors, report glue
- `crates/lacan_shim` — walkdir, regex, JSON helpers, text/junit
- `fixtures/<rule>/{pass,fail}/` — detector fixtures
- `action.yml` — composite Action (checksum-verified binary)
- `assets/` — branding

## Releases

| Major | Scope |
| --- | --- |
| **v1** | Core AIS detectors, SARIF Action, walker/reports |
| **v2** | AIS105, AIS204, AIS401, AIS504 (swarm/data/TF) |
| **v3** | Marketplace polish, optional assistive features |

See GitHub [Milestones](https://github.com/jose-compu/lacan-scan/milestones).
