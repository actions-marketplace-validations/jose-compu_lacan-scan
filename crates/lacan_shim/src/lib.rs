//! Thin Rust surface for Crisp: walk, regex, string helpers, report escapes.
//! Crisp `extern rust` only accepts scalar / `vec<{float,int,str,bool}>` signatures.

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use walkdir::WalkDir;

pub const MAX_SOURCE_BYTES: u64 = 10 * 1024 * 1024;

/// Argv without the program name.
pub fn cli_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

pub fn process_exit(code: i64) {
    process::exit(code as i32);
}

pub fn tool_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Discover files under `root`. Skips symlinks and excluded directory names.
/// Always includes `.git/hooks/*` and `.git/config` when present (AIS001).
pub fn discover(root: &str, excludes: &[String]) -> Vec<String> {
    let root_path = PathBuf::from(root);
    if root_path.is_file() {
        return vec![root_path.to_string_lossy().into_owned()];
    }

    let mut out = Vec::new();
    let walker = WalkDir::new(&root_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            let rel = entry.path().strip_prefix(&root_path).ok();
            if let Some(rel) = rel {
                let mut comps = rel.components();
                if let Some(std::path::Component::Normal(first)) = comps.next() {
                    if first == ".git" {
                        // Enter .git; under it keep only config + hooks/**
                        return match comps.next() {
                            None => true,
                            Some(std::path::Component::Normal(c))
                                if c == "hooks" || c == "config" =>
                            {
                                true
                            }
                            _ => false,
                        };
                    }
                }
            }
            if excludes.iter().any(|item| item == name.as_ref()) {
                return false;
            }
            // Never skip scanning because of a generic ".git" exclude when we
            // already handled the AIS001 allowlist above.
            true
        });

    for entry in walker.flatten() {
        let ft = entry.file_type();
        if ft.is_symlink() || !ft.is_file() {
            continue;
        }
        out.push(entry.path().to_string_lossy().into_owned());
    }
    out.sort();
    out
}

pub fn read_text(path: &str, max_bytes: i64) -> String {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return String::new();
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return String::new();
    }
    let cap = if max_bytes <= 0 {
        MAX_SOURCE_BYTES
    } else {
        max_bytes as u64
    };
    if meta.len() > cap {
        return String::new();
    }
    fs::read_to_string(path).unwrap_or_default()
}

pub fn file_len(path: &str) -> i64 {
    fs::symlink_metadata(path)
        .map(|m| m.len() as i64)
        .unwrap_or(-1)
}

pub fn path_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn path_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn str_contains(hay: &str, needle: &str) -> bool {
    hay.contains(needle)
}

pub fn str_starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

pub fn str_ends_with(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

pub fn str_eq_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

pub fn str_to_lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

pub fn re_is_match(pattern: &str, text: &str) -> bool {
    Regex::new(pattern)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

/// 1-based line of first match, or 0 if none / bad pattern.
pub fn re_find_line(pattern: &str, text: &str) -> i64 {
    let Ok(re) = Regex::new(pattern) else {
        return 0;
    };
    let Some(m) = re.find(text) else {
        return 0;
    };
    let before = &text[..m.start()];
    (before.bytes().filter(|&b| b == b'\n').count() as i64) + 1
}

/// First line with invisible / bidi / tag-block Unicode, or 0.
pub fn find_invisible_line(text: &str) -> i64 {
    for (idx, line) in text.lines().enumerate() {
        for ch in line.chars() {
            let u = ch as u32;
            let invisible = matches!(
                u,
                0x200B | 0x200C | 0x200D | 0x2060 | 0xFEFF // ZW*, WJ, BOM
                | 0x202A..=0x202E // bidi embeds/overrides
                | 0x2066..=0x2069 // bidi isolates
                | 0xE0000..=0xE007F // tags
            );
            if invisible {
                return (idx as i64) + 1;
            }
        }
    }
    0
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

pub fn write_file(path: &str, contents: &str) -> bool {
    fs::write(path, contents).is_ok()
}

pub fn eprint_line(msg: &str) {
    eprintln!("{msg}");
}

pub fn print_line(msg: &str) {
    println!("{msg}");
}

/// Split on commas; trim; drop empties.
pub fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn vec_contains(xs: &[String], item: &str) -> bool {
    xs.iter().any(|x| x == item)
}

pub fn str_vec_push(xs: &[String], item: &str) -> Vec<String> {
    let mut out = xs.to_vec();
    out.push(item.to_string());
    out
}

pub fn join_lines(xs: &[String]) -> String {
    xs.join("\n")
}

// --- Crisp 1.11 json helpers (params are &Value; Crisp emits shared refs) ---

pub fn json_empty_array() -> serde_json::Value {
    serde_json::json!([])
}

pub fn json_push(arr: &serde_json::Value, item: &serde_json::Value) -> serde_json::Value {
    let mut out = arr.clone();
    if let serde_json::Value::Array(ref mut xs) = out {
        xs.push(item.clone());
    }
    out
}

pub fn json_len(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Array(xs) => xs.len() as i64,
        _ => 0,
    }
}

pub fn json_pretty(v: &serde_json::Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into())
}

pub fn json_string(s: &str) -> serde_json::Value {
    serde_json::Value::String(s.to_string())
}

/// Set an arbitrary object key (needed for `$schema`; `$` is not a Crisp ident).
pub fn json_set(obj: &serde_json::Value, key: &str, val: &serde_json::Value) -> serde_json::Value {
    let mut out = obj.clone();
    if let serde_json::Value::Object(ref mut map) = out {
        map.insert(key.to_string(), val.clone());
    }
    out
}

pub fn render_junit_report(
    ids: &[String],
    severities: &[String],
    files: &[String],
    messages: &[String],
) -> String {
    let n = ids.len();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<testsuite name=\"lacan\" tests=\"{n}\" failures=\"{n}\">\n"
    ));
    for i in 0..n {
        let id = ids.get(i).map(String::as_str).unwrap_or("");
        let sev = severities.get(i).map(String::as_str).unwrap_or("");
        let file = files.get(i).map(String::as_str).unwrap_or("");
        let message = messages.get(i).map(String::as_str).unwrap_or("");
        let name = xml_escape(&format!("{id} {message}"));
        let classname = xml_escape(file);
        let fail = xml_escape(&format!("{message} ({sev})"));
        out.push_str(&format!(
            "  <testcase classname=\"{classname}\" name=\"{name}\">\n"
        ));
        out.push_str(&format!("    <failure message=\"{fail}\"/>\n"));
        out.push_str("  </testcase>\n");
    }
    out.push_str("</testsuite>\n");
    out
}

pub fn render_text_report(
    ids: &[String],
    severities: &[String],
    topics: &[String],
    atlases: &[String],
    files: &[String],
    lines: &[i64],
    messages: &[String],
) -> String {
    let n = ids.len();
    let mut out = format!("Lacan Scan — failed checks: {n}\n");
    for i in 0..n {
        out.push('\n');
        out.push_str(&format!("Check: {}\n", ids.get(i).cloned().unwrap_or_default()));
        out.push_str(&format!(
            "\t{}\n",
            messages.get(i).cloned().unwrap_or_default()
        ));
        out.push_str(&format!(
            "\tFile: {}:{}",
            files.get(i).cloned().unwrap_or_default(),
            lines.get(i).copied().unwrap_or(1)
        ));
        out.push('\n');
        out.push_str(&format!(
            "\tSeverity: {}\n",
            severities.get(i).cloned().unwrap_or_default()
        ));
        out.push_str(&format!(
            "\tTopic: {}  ATLAS: {}\n",
            topics.get(i).cloned().unwrap_or_default(),
            atlases.get(i).cloned().unwrap_or_default()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn re_find_line_reports_one_based_line() {
        let text = "alpha\nbeta evil\ngamma\n";
        assert_eq!(re_find_line("evil", text), 2);
        assert_eq!(re_find_line("missing", text), 0);
    }

    #[test]
    fn discover_lists_fixture_files() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/AIS008");
        let excludes = vec!["node_modules".to_string(), "target".to_string()];
        let files = discover(root, &excludes);
        assert!(
            files.iter().any(|p| p.ends_with("fail/setup.sh")),
            "files={files:?}"
        );
        assert!(
            files.iter().any(|p| p.ends_with("pass/setup.sh")),
            "files={files:?}"
        );
    }


    #[test]
    fn find_invisible_line_detects_zwsp() {
        let text = "hello\u{200b}world\n";
        assert_eq!(find_invisible_line(text), 1);
        assert_eq!(find_invisible_line("hello world\n"), 0);
    }

    #[test]
    fn render_text_report_includes_check() {
        let body = render_text_report(
            &["AIS008".into()],
            &["HIGH".into()],
            &["2.1".into()],
            &["AML.T0010.000".into()],
            &["x.sh".into()],
            &[3],
            &["hit".into()],
        );
        assert!(body.contains("AIS008"));
        assert!(body.contains("Topic: 2.1"));
    }
}
