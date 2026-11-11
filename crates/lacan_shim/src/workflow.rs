//! AIS002: in one GitHub Actions step, untrusted CI input reaches an AI action
//! while that job (or the workflow) also holds secrets or write permissions.
//! Line-oriented YAML scan; malformed layouts return 0.

use regex::Regex;
use std::sync::OnceLock;

/// 1-based line of the AI step when the three-hop chain is present, else 0.
pub fn workflow_ai_chain_line(path: &str, text: &str) -> i64 {
    if !is_workflow_path(path) {
        return 0;
    }
    let workflow_write = workflow_has_write_or_secret(text);
    for job in split_jobs(text) {
        let job_write = job_has_write_or_secret(job.body);
        if !(job_write || workflow_write) {
            continue;
        }
        for step in split_steps(job.body) {
            if first_match_line(step.body, untrusted_re()) > 0
                && first_match_line(step.body, ai_re()) > 0
            {
                return absolute_line(text, step.body, ai_re());
            }
        }
    }
    0
}

fn is_workflow_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    (lower.contains("/.github/workflows/") || lower.contains("/workflows/"))
        && (lower.ends_with(".yml") || lower.ends_with(".yaml"))
}

struct Job<'a> {
    body: &'a str,
}

struct Step<'a> {
    body: &'a str,
}

fn split_jobs(text: &str) -> Vec<Job<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut jobs_start = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_end() == "jobs:" || line.trim_end().starts_with("jobs:") {
            jobs_start = Some(i + 1);
            break;
        }
    }
    let Some(start) = jobs_start else {
        return Vec::new();
    };
    let mut job_starts: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(start) {
        if is_job_key(line) {
            job_starts.push(i);
        }
    }
    if job_starts.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (idx, &from) in job_starts.iter().enumerate() {
        let to = job_starts.get(idx + 1).copied().unwrap_or(lines.len());
        let start_byte = line_byte_offset(text, from);
        let end_byte = line_byte_offset(text, to);
        out.push(Job {
            body: &text[start_byte..end_byte],
        });
    }
    out
}

fn split_steps(job: &str) -> Vec<Step<'_>> {
    let lines: Vec<&str> = job.lines().collect();
    let mut starts: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if is_step_item(line) {
            starts.push(i);
        }
    }
    if starts.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (idx, &from) in starts.iter().enumerate() {
        let to = starts.get(idx + 1).copied().unwrap_or(lines.len());
        let start_byte = line_byte_offset(job, from);
        let end_byte = line_byte_offset(job, to);
        out.push(Step {
            body: &job[start_byte..end_byte],
        });
    }
    out
}

fn is_step_item(line: &str) -> bool {
    // Typical Actions indent: "      - uses:" / "      - name:" / "      - run:"
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return false;
    }
    let indent = line.len() - trimmed.len();
    indent >= 4
}

fn is_job_key(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() < 3 || bytes[0] != b' ' || bytes[1] != b' ' {
        return false;
    }
    if bytes[2] == b' ' || bytes[2] == b'#' || bytes[2] == b'-' {
        return false;
    }
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.ends_with(':') {
        return false;
    }
    let name = trimmed.trim_end_matches(':').trim();
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !matches!(
            name,
            "permissions"
                | "defaults"
                | "concurrency"
                | "env"
                | "runs-on"
                | "steps"
                | "needs"
                | "if"
                | "timeout-minutes"
                | "strategy"
                | "container"
                | "services"
                | "outputs"
                | "continue-on-error"
                | "environment"
        )
}

fn line_byte_offset(text: &str, line_idx: usize) -> usize {
    if line_idx == 0 {
        return 0;
    }
    let mut count = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            count += 1;
            if count == line_idx {
                return i + 1;
            }
        }
    }
    text.len()
}

fn first_match_line(text: &str, re: &Regex) -> i64 {
    for (i, line) in text.lines().enumerate() {
        if re.is_match(line) {
            return i as i64 + 1;
        }
    }
    0
}

fn absolute_line(full: &str, fragment: &str, re: &Regex) -> i64 {
    let Some(pos) = full.find(fragment) else {
        return first_match_line(fragment, re);
    };
    let base = full[..pos].bytes().filter(|&b| b == b'\n').count() as i64;
    let local = first_match_line(fragment, re);
    if local == 0 {
        0
    } else {
        base + local
    }
}

fn untrusted_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\$\{\{\s*github\.(head_ref|event\.(issue|pull_request|comment|review|discussion|workflow_run)\.[^\}]+)\s*\}\}",
        )
        .expect("untrusted regex")
    })
}

fn ai_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(uses:\s*\S*(openai|anthropic|chatgpt|continue|aider|cursor-agent|huggingface|bedrock|gemini|ollama|langchain|litellm|auto-gpt|smolagents)\S*|run:\s*.*\b(openai|anthropic|chatgpt|litellm|ollama|langchain)\b)",
        )
        .expect("ai regex")
    })
}

fn secret_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)secrets\.[A-Za-z0-9_]+").expect("secret regex"))
}

fn write_perm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(contents|pull-requests|issues|id-token|packages|deployments|actions|security-events)\s*:\s*write",
        )
        .expect("write perm regex")
    })
}

fn job_has_write_or_secret(job: &str) -> bool {
    secret_re().is_match(job) || write_perm_re().is_match(job)
}

fn workflow_has_write_or_secret(text: &str) -> bool {
    let header = text.split("\njobs:").next().unwrap_or(text);
    secret_re().is_match(header) || write_perm_re().is_match(header)
}

#[cfg(test)]
mod tests {
    use super::{is_workflow_path, workflow_ai_chain_line};

    #[test]
    fn path_gate() {
        assert!(is_workflow_path(".github/workflows/ci.yml"));
        assert!(is_workflow_path("fixtures/AIS002/fail/workflows/chain.yml"));
        assert!(!is_workflow_path("src/main.crp"));
    }

    #[test]
    fn three_hop_hit() {
        let src = r#"
name: agent
on: pull_request_target
permissions:
  contents: write
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: ask model
        uses: openai/chatgpt-action@v1
        with:
          prompt: ${{ github.event.pull_request.title }}
"#;
        let line = workflow_ai_chain_line(".github/workflows/agent.yml", src);
        assert!(line > 0, "expected hit, got {line}");
    }

    #[test]
    fn cooccurrence_without_step_taint_is_clean() {
        let src = r#"
name: agent
on: pull_request_target
permissions:
  contents: write
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ github.event.pull_request.title }}"
      - uses: openai/chatgpt-action@v1
        with:
          prompt: static safe prompt
"#;
        assert_eq!(workflow_ai_chain_line(".github/workflows/agent.yml", src), 0);
    }

    #[test]
    fn missing_write_is_clean() {
        let src = r#"
name: agent
on: pull_request
permissions:
  contents: read
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: openai/chatgpt-action@v1
        with:
          prompt: ${{ github.event.pull_request.title }}
"#;
        assert_eq!(workflow_ai_chain_line(".github/workflows/agent.yml", src), 0);
    }

    #[test]
    fn missing_ai_is_clean() {
        let src = r#"
name: echo
on: pull_request_target
permissions:
  contents: write
jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ github.event.pull_request.title }}"
"#;
        assert_eq!(workflow_ai_chain_line(".github/workflows/echo.yml", src), 0);
    }

    #[test]
    fn cross_job_does_not_chain() {
        let src = r#"
name: split
on: pull_request_target
jobs:
  ingest:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ github.event.pull_request.title }}"
  model:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: openai/chatgpt-action@v1
        with:
          prompt: safe
"#;
        assert_eq!(workflow_ai_chain_line(".github/workflows/split.yml", src), 0);
    }

    #[test]
    fn secrets_env_counts() {
        let src = r#"
name: agent
on: issues
jobs:
  reply:
    runs-on: ubuntu-latest
    steps:
      - uses: anthropic/claude-action@v1
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
          PROMPT: ${{ github.event.issue.body }}
"#;
        assert!(workflow_ai_chain_line(".github/workflows/reply.yml", src) > 0);
    }
}
