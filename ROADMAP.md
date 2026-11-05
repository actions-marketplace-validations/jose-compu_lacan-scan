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

## Why go beyond syntax

We tested the v3 scanner (local build 3.0.7) against common attack formats. Each one of these produced **zero findings**:

| Input | Why v3 misses it |
| --- | --- |
| A poisoned fine-tune set (trigger phrase → `Positive`, 50 of 550 rows) | `AIS204` only matches literal markers such as `backdoor_trigger`, which real poison does not contain |
| Malicious pickles (`os.system`, `runpy`, inside a deflated torch zip) | `read_text` returns an empty string for non-UTF-8 files, so detectors never see the bytes, and `AIS201` is not implemented |
| `from_pretrained('org/evil')` next to a pinned call in the same file | `AIS203` treats one `revision=` anywhere in the file as pinning every call |

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

**Fix:** make `AIS203` evaluate each `from_pretrained` call separately.

**Exit gate:** v1 detectors closed, pickle allowlist tested on a real checkpoint corpus, per-rule precision published.

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

**Exit gate:** hostile-template execution limits tested, every parser fuzzed against truncated and malformed input, and template rendering disabled by a flag.

**Known limit:** the default `actions/checkout` does not fetch LFS content, so weights stored in LFS are invisible without `lfs: true`. v4's unscanned-file report makes that gap visible.

### v6 — Data interpretation (L4)

Judge datasets and artifacts as a whole, not line by line.

| Rule | Severity | Detects | Evidence |
| --- | --- | --- | --- |
| `AIS208` | MEDIUM, experimental | Suspected backdoor trigger in labelled datasets, replacing the marker check in `AIS204` | Spiked, see limits above |
| `AIS209` | LOW, experimental | Safety-stripping fine-tune sets: refusals removed, obedience system prompts | Not spiked, speculative |
| `AIS210` | MEDIUM | Scripts that harvest model outputs at scale for distillation or logit-based extraction | Not spiked |
| `AIS304` | MEDIUM | Serving config that exposes full logits or logprobs with no rate limit, extending `AIS303` | Not spiked |
| `AIS402` | HIGH | Transcript event chains: a session reads a credential, then later reaches the network, extending `AIS401` | Not spiked |
| `AIS403` | HIGH | Transcripts where an agent edits its own hooks, settings, or monitor prompts | Not spiked |
| `AIS211` | MEDIUM | `lacan diff-weights`: compare two safetensors files and report a change confined to a few tensors | Not spiked |

- `--changed-only` needs special handling here, because dataset statistics need the whole file rather than the diff.
- A localized weight change is not proof of tampering. `AIS211` is useful when a file claims to match a known revision.

**Exit gate:** tested on at least three further real datasets, with resource limits on large files. A rule leaves experimental status only when its measured precision meets the bar set in v4.

### v7 — Flow and attack chains (L5)

Connect sources to sinks across code and config, and report paths rather than isolated lines.

| Item | Size | Notes |
| --- | --- | --- |
| Python and TypeScript parsing in the shim | L | Likely tree-sitter; check the binary size cost |
| `AIS301` v2: model output reaches `eval` or a shell | M | Replaces "the file mentions an LLM and contains `eval`" |
| `AIS104` v2: retrieved or user data reaches a system prompt | M | Follows the data instead of matching concatenation |
| `AIS002` v2: untrusted CI input reaches an AI step, then a secret or write token | M | Cross-step workflow analysis |
| `AIS601`: effective agent authority | M | Combines file, shell, network, MCP, and CI grants into one report |
| `AIS602`: attack-chain findings | L | Links related findings into an ATLAS-tagged path and SARIF `codeFlows` |
| `--baseline` file | S | Lets existing repos adopt the stricter rules gradually |

**Exit gate:** `AIS301` v2 cuts false positives against the v4 baseline without losing fixture recall.

### v8 — Live probing (L6, undecided)

An optional `lacan probe --target URL` could test what files cannot show: whether an endpoint returns logprobs, whether control tokens survive tokenization, and whether canary injections work.

This changes what the tool is, so it needs a decision first. If it goes ahead, it should be opt-in, limited to an explicitly named target that the user is authorized to test, capped by a request budget, unavailable in the default Action, and possibly a separate binary.

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
3. Should the transcript rules in v6 stay deterministic? Epic #21 left LLM assistance open ("or explicitly never").
