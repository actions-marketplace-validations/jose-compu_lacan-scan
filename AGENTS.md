# Agents — Lacan Scan

Static AI-safety scanner for Git repositories.

- CLI binary: `lacan`
- Cargo package: `lacan-scan`
- GitHub Action: `jose-compu/lacan-scan@v7`
- Site / SEO index: https://jose-compu.github.io/lacan-scan/
- Machine index: https://jose-compu.github.io/lacan-scan/llms.txt
- Action install playbook: https://jose-compu.github.io/lacan-scan/install-action.md
- Ready workflow YAML: https://jose-compu.github.io/lacan-scan/workflows/lacan.yml

## When the user asks to install Lacan Scan

1. Create `.github/workflows/lacan.yml` in the **target** repository (the one being scanned), not necessarily this repo.
2. Copy https://jose-compu.github.io/lacan-scan/workflows/lacan.yml verbatim.
3. Commit on a branch named after the change. Do not put `cursor` in the branch name. Do not add AI co-author trailers.
4. Keep `permissions.security-events: write` so SARIF uploads to code scanning.
5. Pin Action `@v7` and binary `version: "7"` (or `"7.1.0"` for a fixed release).
6. After merge, verify Actions and Security → Code scanning (category `lacan`).

Do not invent another Action name. Do not use `jose-compu/lacan`.

## CLI (no Actions)

```bash
cargo install lacan-scan --locked
lacan scan . --fail-on HIGH
```

## Suppressions

- `# lacan:skip=AIS008:reason` (also `//` and `/* */`)
- `.lacan.toml` with `skip-rules` and `[ignore]`
