# Lacan Scan roadmap

Proposed releases after v3. Rule ids below are proposals. Issues and milestones are tracked on [GitHub](https://github.com/jose-compu/lacan-scan/milestones).

## Direction

Lacan Scan's detectors are almost entirely per-file regex today. That is cheap, but it finds what a payload *says*, not what it *does*. Each release moves one step up this ladder:

| Level | What the scanner does | Runs untrusted content? | Release |
| --- | --- | --- | --- |
| L0 | Match text with regex | No | v1–v3 |
| L1 | Parse config into structure (JSON, YAML, TOML) | No | v4 |
| L2 | Decode formats and encodings (pickle, GGUF, base64) | No | v4–v5 |
| L3 | Render templates in a sandbox and compare the output | Data-language only, sandboxed | v5 |
| L4 | Measure statistics across a whole dataset or artifact | No | v6 |
| L5 | Follow data flow across code, config, and transcripts | No | v7 |
| L6 | Probe a live endpoint the user names | Calls a live system | v8 (undecided) |

Everything through L5 keeps the "file traces only" promise. L3 runs template logic, so it needs an engine with no route to the host and a hard execution limit. L6 breaks the promise, so it stays out unless explicitly decided otherwise.

## Delivery: CI first

The GitHub Action is the main channel. Lacan Scan's job is to gate pull requests into mainline and keep a full view of mainline itself. The CLI is what the Action runs, and it lets developers reproduce a CI finding locally. Pre-commit is a secondary convenience.

| Event | Scan | Result |
| --- | --- | --- |
| Pull request to `main` | Changed files | Fails the check at `fail-on`, with findings visible on the PR |
| Push to `main` | Whole repository | Uploads SARIF to code scanning, and catches what diff scans cannot see |

A feature counts as shipped only when it meets all of these:

- **Reachable from the Action:** on by default or available as an input. CLI-only does not count.
- **Visible without extra setup:** findings appear in the job summary and as annotations, because code scanning on private repositories needs GitHub Advanced Security.
- **Safe on a hostile PR:** files in a PR are attacker-controlled. The scan step holds no token and uses no network, and a malformed file is reported rather than crashing or hanging the job.
- **Within the runner budget:** time and memory stay inside limits set from the performance baseline in #4.
- **Predictable as a gate:** new blocking rules arrive only in a major version, so `@vN` never starts failing a mainline after a minor or patch update. Experimental rules report but never fail the job.

## Why go beyond syntax

We tested the v3 scanner (local build 3.0.7) against common attack formats. Each one of these produced **zero findings**:

| Input | Why v3 misses it |
| --- | --- |
| A poisoned fine-tune set (trigger phrase → `Positive`, 50 of 550 rows) | `AIS204` only matches literal markers such as `backdoor_trigger`, which real poison does not contain |
| Malicious pickles (`os.system`, `runpy`, inside a deflated torch zip) | `read_text` returns an empty string for non-UTF-8 files, so detectors never see the bytes, and `AIS201` is not implemented |
| An unpinned `from_pretrained` call next to a pinned one in the same file | `AIS203` treats one `revision=` anywhere in the file as pinning every call |

## Feasibility spikes

These were throwaway Python prototypes, not product code. They show whether an approach is worth scheduling. They are not performance claims.

### Pickle walking: works

A symbolic opcode walk that never unpickles, checked against an allowlist of imports.

| Sample | Byte regex | Walker |
| --- | --- | --- |
| `os.system`, `posix.system`, `subprocess.Popen` through memo, second pickle after `STOP` | Hit | Flagged |
| `runpy._run_code` (not on a typical denylist) | Miss | Flagged |
| Module name computed at runtime (`STACK_GLOBAL` on a non-literal) | Miss | Flagged |
| Payload inside a deflated torch zip | Miss | Flagged |
| Benign state-dict samples | Miss | Clean |

Three real tiny checkpoints from `hf-internal-testing` used only five distinct globals. One of them, `torch.ByteStorage`, was missing from the first allowlist, so the allowlist needs a real corpus before it can gate. Samples were hand-built, the test set is small, and the byte regex is a simple comparator rather than an existing scanner.

### Template rendering: works, with limits

Rendering chat templates with `minijinja`, a Rust engine with no Python object model.

- **Compatibility:** 8 of 8 fetched real templates rendered (Qwen2.5, DeepSeek-R1, Phi-3, Hermes-3, OpenHermes, Zephyr, Falcon3, Granite 3.3). SmolLM2 and Gemma could not be fetched.
- **Sandbox escape:** two Python-style injection payloads resolved to undefined and did not reach the host.
- **Differential rendering:** probe messages caught a backdoored template that branched on `'invoice'`, with no false positives on the 8 real templates.
- **Rare triggers:** probes alone missed a backdoor on an unlikely string. Extracting literals from content conditions and replaying them caught it. An attacker can still build the string dynamically, so any content-dependent branch should be reported in its own right.
- **Control tokens are not a useful per-template signal:** all 8 real templates pass user-supplied control tokens through unchanged. The risk depends on how the serving code tokenizes, so that rule has to inspect the code path.
- **Not tested:** execution limits on hostile templates, such as nested loops. This must be tested before v5 ships.

### Poison detection: useful as advisory, not as a gate

The detector looks for an n-gram that predicts one label, appears in varied contexts, and whose label a Naive Bayes model trained without those rows disagrees with. The first version was unusable, with roughly 10,800 false positives on clean SST-2. Two more iterations produced the results below. Thresholds were tuned on SST-2, so Yelp is the fairer measure.

| Scenario | Poison rate | Trigger found | False positives per run |
| --- | --- | --- | --- |
| **Yelp (held out), 5,600 rows:** flipped labels | 0.88% | 5/5 | 0 |
| Yelp: only opposite-label rows flipped, fixed position | 0.88% | 5/5 | 0.6 (trigger plus next word, so not a true false alarm) |
| Yelp: only opposite-label rows flipped, random position | 0.88% | 5/5 | 0 |
| Yelp: only opposite-label rows flipped, half the poison | 0.44% | 5/5 | 0 |
| Yelp: clean-label poison | 0.88% | **0/5** | 0 |
| Yelp: no poison | 0% | n/a | 0 |
| **SST-2 (tuning):** 50 flipped rows in 500 | 9.1% | 3/5 | 0 |
| SST-2: 50 opposite-label rows in 10k | 0.5% | 5/5 | 0.2 |
| SST-2: 50 flipped rows in 10k | 0.5% | 1/5 | 0.4 |
| SST-2 full: nonsense trigger, 67 rows | 0.1% | 1/1 | 8 |
| SST-2 full: a phrase that occurs naturally 22 times | 0.1% | **0/1** | 8 |
| SST-2 full: no poison | 0% | n/a | 8 |

What this means for the rule:

- It finds rare triggers on opposite-label rows. It misses clean-label poison by design, and it struggles with triggers that are already common in the data.
- At the low rates in the literature, recall on natural phrases is poor, so a clean result is not evidence of a clean dataset.
- False positives on large sets are natural sentiment phrases such as `lacks the` and `distasteful`.
- All poison was synthetic, and only binary labels were tested. Free-form completion datasets are unsolved.
- It should ship at `MEDIUM`, marked experimental, and off for `--fail-on` until tested on more corpora.

## Releases

Sizes are relative: S, M, L.

### v4 — Structure and catalog completion (L1, L2)

Finish the promised v1 catalog on real parsers rather than more regex, and make binary files readable.

**Foundation**

| Item | Size | Notes |
| --- | --- | --- |
| Binary-safe read API in the shim | S | Prerequisite for anything that reads weights |
| JSON/JSONC, YAML, and TOML parsing in the shim | M | Needed by the rules below |
| Precision corpus and per-rule baseline (#4) | M | Sets the bar for every later release |
| Per-occurrence findings | S | `re_find_line` reports only the first match per pattern |
| Unscanned-file reporting | S | Report oversized files and LFS pointers instead of skipping them silently |

**CI channel**

| Item | Size | Notes |
| --- | --- | --- |
| Job summary and inline annotations | S | The Action emits neither today, so without code scanning findings appear only in logs |
| Test the Action from its own commit | S | The self-scan runs the published `@v3`, so changes to `action.yml` are not exercised until after a release |
| Recommended mainline workflow | S | Changed files on PRs and a full scan on `main`; the self-scan should follow the same pattern |
| Suppressions (`.lacan.toml` + inline comments) | S | Shipped in 4.0.0: skip-rules, path `[ignore]`, `lacan:skip=RULE` in Rust/Python/TS/Bash |
| `--baseline` file (moved from v7) | S | Existing repos cannot adopt a blocking check without a way to accept current findings |
| Runner budget | S | Set time and memory limits from #4, and enforce them in CI |
| Fork PR behavior | S | Verify the scan and SARIF upload with a read-only token |
| Runner coverage | S | Windows runners are unsupported, and the Action needs `curl` and `python3` on self-hosted and container jobs; document this or fix it |

**Existing open detectors** (#6–#15, currently under the v1 milestone)

`AIS002` workflow taint, `AIS003` VS Code tasks, `AIS004` install hooks, `AIS005` skills and hooks that run on load, `AIS006` MCP launch, `AIS007` permission wildcards, `AIS009` persistence files, `AIS104` template concatenation, `AIS201` pickle opcodes, `AIS503` unauthenticated inference server.

- `AIS201` should use the allowlist walker above: read every pickle in a file, open zip containers, and treat unresolved imports as critical.
- `AIS005` should cover dynamic context injection in skill files.
- `AIS007` should know which "read-only" commands execute code: `find -exec`, `git ls-remote --upload-pack`, `man -P`.

**New rules**

| Rule | Severity | Detects |
| --- | --- | --- |
| `AIS010` | HIGH | Automation that bypasses review: `gh pr review --approve`, branch-protection API edits, CODEOWNERS rewrites |
| `AIS011` | HIGH | CI jobs that register a persistent self-hosted runner |
| `AIS012` | MEDIUM | Build-time execution: Makefile targets and `pyproject.toml` hooks and entry points |
| `AIS013` | MEDIUM | Weakened CI checks: `continue-on-error` or `\|\| true` on security steps |
| `AIS505` | HIGH | `ENV LD_PRELOAD` or `LD_LIBRARY_PATH` in GPU container images |
| `AIS506` | HIGH | NVIDIA Container Toolkit pinned at ≤ 1.17.7 (CVE-2025-23266) |

**Fix:** make `AIS203` evaluate each `from_pretrained` call separately, and stop matching prose in Markdown. A sentence in this roadmap failed this repo's own check that way.

**Exit gate:** v1 detectors closed, pickle allowlist tested on a real checkpoint corpus, per-rule precision published, and the recommended workflow verified end to end on both PR and `main` events from the Action's own commit.

### v5 — Artifact and encoding interpreters (L2, L3)

Read what model artifacts contain.

| Rule | Severity | Detects | Evidence |
| --- | --- | --- | --- |
| `AIS205` | HIGH | Malformed safetensors headers: overlapping offsets, oversized header, unexpected metadata | Not spiked |
| `AIS206` | MEDIUM | GGUF metadata anomalies, and extraction of embedded chat templates for the rules below | Not spiked |
| `AIS207` | HIGH | Other code-carrying formats: NumPy object arrays, joblib, Keras `Lambda` layers | Not spiked |
| `AIS106` | CRITICAL | Template-injection primitives in chat templates (`__class__`, `__globals__`), the class behind CVE-2024-34359 | Spiked |
| `AIS107` | HIGH | Content-dependent branches and differential output in chat templates | Spiked |
| `AIS108` | MEDIUM | Serving code that flattens with `apply_chat_template` and then tokenizes with special-token parsing on | Spike rules out the per-template form |
| `AIS109` | HIGH | Injection text hidden in encodings: decode base64, hex, entities, and Unicode tags, then rerun `AIS101` and `AIS102` | Not spiked |
| `AIS110` | MEDIUM | Text in RAG corpora addressed to the model rather than the reader | Not spiked, likely noisy |

**Exit gate:** hostile-template execution limits tested, every parser fuzzed against truncated and malformed input, a hostile file in a PR cannot hang or crash the job, and template rendering can be turned off through an Action input.

**Known limit:** the default `actions/checkout` does not fetch LFS content, so weights stored in LFS are invisible without `lfs: true`. v4's unscanned-file report should surface that gap in the job summary, with the cost of enabling LFS documented.

### v6 — Data interpretation (L4) — complete (6.0.0)

Judge datasets and artifacts as a whole, not line by line. **Shipped in 6.0.0:** L4 foundation via shim `ngram_count` and experimental `AIS208` (n-gram trigger check on labelled datasets; stays advisory / non-gating until precision meets the v4 bar).

| Rule | Severity | Detects | Status |
| --- | --- | --- | --- |
| `AIS208` | MEDIUM, experimental | Suspected backdoor trigger in labelled datasets, replacing the marker check in `AIS204` | **Shipped** (experimental); 50 MiB size cap |
| `AIS209` | LOW, experimental | Safety-stripping fine-tune sets: refusals removed, obedience system prompts | **Shipped** (experimental stub) |
| `AIS210` | MEDIUM | Scripts that harvest model outputs at scale for distillation or logit-based extraction | **Shipped** |
| `AIS304` | MEDIUM | Serving config that exposes full logits or logprobs with no rate limit, extending `AIS303` | **Shipped** |
| `AIS402` | HIGH | Transcript event chains: a session reads a credential, then later reaches the network, extending `AIS401` | Deferred (v7-adjacent) |
| `AIS403` | HIGH | Transcripts where an agent edits its own hooks, settings, or monitor prompts | Deferred (v7-adjacent) |
| `AIS211` | MEDIUM | On a PR, compare changed safetensors files with their version on the base ref, and report a change confined to a few tensors | Deferred |

- A dataset touched by a PR must be analyzed whole, not just its changed lines. Sharded datasets also need their unchanged shards.
- Experimental rules appear in the job summary and never fail the check.
- A localized weight change is not proof of tampering. `AIS211` is useful when a file claims to match a known revision.

**Exit gate (met for 6.0.0 scope):** L4 path live with `AIS208` experimental; broader corpus precision and remaining rules stay follow-ups. A rule leaves experimental status only when its measured precision meets the bar set in v4.

**6.1.0** adds the leftover artifact rules as experimental (they never fail `--fail-on`): `AIS107`, `AIS110`, `AIS205`, `AIS206`, `AIS207`, `AIS211`. `AIS402` and `AIS403` stay deferred.

### v7 — Flow and attack chains (L5)

**7.0.0** ships the first slice only: `AIS301` keeps a finding when, inside one Python or TypeScript function, a value produced by an LLM call reaches `eval`, `exec`, or a shell. Other languages return no finding. Parse errors return no finding. Cross-function flow, `AIS104` v2, `AIS002` v2, `AIS601`, `AIS602`, and `--baseline` stay open.

Connect sources to sinks across code and config, and report paths rather than isolated lines. A PR scan still has to follow flows through unchanged files, so this release needs caching or a whole-repository pass that fits the runner budget. Paths must also appear in the job summary, because `codeFlows` only render in code scanning.

| Item | Size | Notes |
| --- | --- | --- |
| Python and TypeScript parsing in the shim | L | Likely tree-sitter; check the binary size cost |
| `AIS301` v2: model output reaches `eval` or a shell | M | Replaces "the file mentions an LLM and contains `eval`" |
| `AIS104` v2: retrieved or user data reaches a system prompt | M | Follows the data instead of matching concatenation |
| `AIS002` v2: untrusted CI input reaches an AI step, then a secret or write token | M | Cross-step workflow analysis |
| `AIS601`: effective agent authority | M | Combines file, shell, network, MCP, and CI grants into one report |
| `AIS602`: attack-chain findings | L | Links related findings into an ATLAS-tagged path and SARIF `codeFlows` |

**Exit gate:** `AIS301` v2 cuts false positives against the v4 baseline without losing fixture recall, and PR scans stay within the runner budget.

### v8 — Live probing (L6, undecided)

An optional `lacan probe --target URL` could test what files cannot show: whether an endpoint returns logprobs, whether control tokens survive tokenization, and whether canary injections work.

This changes what the tool is, so it needs a decision first. If it goes ahead, it should be opt-in, limited to an explicitly named target that the user is authorized to test, capped by a request budget, unavailable in the default Action, and possibly a separate binary.

With the Action as the main channel, this is the lowest priority. A live probe needs credentials and a running target, which do not fit a check on every pull request.

## Not planned

These leave no reliable file trace or need model execution:

- AI-control protocols
- Jailbreak effectiveness
- Adversarial optimization and watermarking. Checking prompts for GCG-style gibberish is possible but likely noisy, so it stays a candidate only.
- Hardware side channels
- GPU RowHammer

## Open decisions

1. Should the open v1 detectors move to the v4 milestone, or stay under v1 and ship in a v3 minor?
2. Does live probing (v8) belong in this project at all?
3. Should the deferred v6 transcript rules (`AIS402`/`AIS403`) stay deterministic when scheduled? Epic #21 left LLM assistance open ("or explicitly never").
