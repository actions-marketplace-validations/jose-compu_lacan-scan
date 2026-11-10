//! Header-only artifact checks for experimental v6.1 rules.
//! Never loads tensor bodies and never renders untrusted templates without a fuel cap.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

const HEADER_CAP: u64 = 8 * 1024 * 1024;
const SNIFF_CAP: usize = 64 * 1024;

fn read_capped(path: &str, cap: u64) -> Option<Vec<u8>> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return None;
    }
    let mut f = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; cap.min(meta.len()) as usize];
    let mut read = 0;
    while read < buf.len() {
        match f.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(_) => return None,
        }
    }
    buf.truncate(read);
    Some(buf)
}

fn file_size(path: &str) -> Option<u64> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.is_file() {
        Some(meta.len())
    } else {
        None
    }
}

fn ext_is(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// AIS205. `ok` | `oversize` | `bad-header` | `overlap` | `out-of-range` | `skip`
pub fn safetensors_header_issue(path: &str) -> String {
    if !ext_is(path, "safetensors") {
        return "skip".into();
    }
    classify_safetensors_bytes(&read_header_bytes(path), file_size(path).unwrap_or(0))
}

fn read_header_bytes(path: &str) -> Vec<u8> {
    read_capped(path, HEADER_CAP + 8).unwrap_or_default()
}

fn classify_safetensors_bytes(bytes: &[u8], file_len: u64) -> String {
    if bytes.len() < 8 {
        return "bad-header".into();
    }
    let n = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    if n == 0 || n > HEADER_CAP {
        return "oversize".into();
    }
    if file_len < 8 + n {
        return "bad-header".into();
    }
    let header_end = 8 + n as usize;
    if bytes.len() < header_end {
        return "bad-header".into();
    }
    let header = match serde_json::from_slice::<Value>(&bytes[8..header_end]) {
        Ok(Value::Object(map)) => map,
        _ => return "bad-header".into(),
    };
    let body_len = file_len - 8 - n;
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    for (key, val) in &header {
        if key.starts_with("__") {
            continue;
        }
        let Some(offsets) = val.get("data_offsets").and_then(|v| v.as_array()) else {
            return "bad-header".into();
        };
        if offsets.len() != 2 {
            return "bad-header".into();
        }
        let Some(start) = offsets[0].as_u64() else {
            return "bad-header".into();
        };
        let Some(end) = offsets[1].as_u64() else {
            return "bad-header".into();
        };
        if end < start || end > body_len {
            return "out-of-range".into();
        }
        ranges.push((start, end));
    }
    ranges.sort_unstable();
    for w in ranges.windows(2) {
        if w[1].0 < w[0].1 {
            return "overlap".into();
        }
    }
    "ok".into()
}

fn tensor_map(bytes: &[u8]) -> Option<BTreeMap<String, String>> {
    if bytes.len() < 8 {
        return None;
    }
    let n = u64::from_le_bytes(bytes[0..8].try_into().ok()?) as usize;
    if n == 0 || n > HEADER_CAP as usize || bytes.len() < 8 + n {
        return None;
    }
    let Value::Object(header) = serde_json::from_slice::<Value>(&bytes[8..8 + n]).ok()? else {
        return None;
    };
    let mut out = BTreeMap::new();
    for (key, val) in header {
        if key.starts_with("__") {
            continue;
        }
        out.insert(key, canonical_tensor(val));
    }
    Some(out)
}

fn canonical_tensor(val: Value) -> String {
    match val {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let ordered: Map<String, Value> = keys
                .into_iter()
                .filter_map(|k| map.get(&k).cloned().map(|v| (k, v)))
                .collect();
            Value::Object(ordered).to_string()
        }
        other => other.to_string(),
    }
}

/// AIS211. `localized` | `same` | `broad` | `missing-base` | `bad-header` | `skip`
pub fn safetensors_diff_issue(path: &str, base_ref: &str) -> String {
    if !ext_is(path, "safetensors") {
        return "skip".into();
    }
    let current = read_header_bytes(path);
    let Some(base) = load_base_header(path, base_ref) else {
        return "missing-base".into();
    };
    let Some(a) = tensor_map(&current) else {
        return "bad-header".into();
    };
    let Some(b) = tensor_map(&base) else {
        return "bad-header".into();
    };
    let mut changed = 0u32;
    let mut keys: Vec<_> = a.keys().chain(b.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        if a.get(&k) != b.get(&k) {
            changed += 1;
        }
    }
    match changed {
        0 => "same".into(),
        1..=3 => "localized".into(),
        _ => "broad".into(),
    }
}

fn load_base_header(path: &str, base_ref: &str) -> Option<Vec<u8>> {
    let sidecar = format!("{path}.base");
    if Path::new(&sidecar).is_file() {
        return Some(read_header_bytes(&sidecar));
    }
    if base_ref.is_empty() {
        return None;
    }
    git_blob_prefix(path, base_ref)
}

fn git_blob_prefix(path: &str, base_ref: &str) -> Option<Vec<u8>> {
    let abs = fs::canonicalize(path).ok()?;
    let parent = abs.parent()?.to_string_lossy().into_owned();
    let root_out = Command::new("git")
        .args(["-C", &parent, "rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !root_out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&root_out.stdout).trim().to_string();
    let rel = abs.strip_prefix(&root).ok()?;
    let spec = format!("{base_ref}:{}", rel.to_string_lossy());
    let mut child = Command::new("git")
        .args(["-C", &root, "cat-file", "blob", &spec])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut buf = vec![0u8; (HEADER_CAP + 8) as usize];
    let mut read = 0;
    if let Some(stdout) = child.stdout.as_mut() {
        while read < buf.len() {
            match stdout.read(&mut buf[read..]) {
                Ok(0) => break,
                Ok(n) => read += n,
                Err(_) => break,
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    if read < 8 {
        return None;
    }
    buf.truncate(read);
    Some(buf)
}

/// AIS206. `ok` | `bad-header` | `truncated` | `anomaly` | `skip`
pub fn gguf_header_issue(path: &str) -> String {
    if !ext_is(path, "gguf") {
        return "skip".into();
    }
    match parse_gguf(path) {
        GgufParse::Ok { .. } => "ok".into(),
        GgufParse::Bad => "bad-header".into(),
        GgufParse::Truncated => "truncated".into(),
        GgufParse::Anomaly => "anomaly".into(),
    }
}

pub fn gguf_chat_templates(path: &str) -> Vec<String> {
    if !ext_is(path, "gguf") {
        return Vec::new();
    }
    match parse_gguf(path) {
        GgufParse::Ok { templates } => templates,
        _ => Vec::new(),
    }
}

enum GgufParse {
    Ok { templates: Vec<String> },
    Bad,
    Truncated,
    Anomaly,
}

fn parse_gguf(path: &str) -> GgufParse {
    let Some(bytes) = read_capped(path, HEADER_CAP) else {
        return GgufParse::Bad;
    };
    if bytes.len() < 4 || &bytes[0..4] != b"GGUF" {
        return GgufParse::Bad;
    }
    let mut c = Cursor::new(&bytes[4..]);
    let Some(version) = c.u32() else {
        return GgufParse::Truncated;
    };
    if version == 0 || version > 3 {
        return GgufParse::Anomaly;
    }
    if c.u64().is_none() {
        return GgufParse::Truncated;
    }
    let Some(kv_count) = c.u64() else {
        return GgufParse::Truncated;
    };
    if kv_count > 4096 {
        return GgufParse::Anomaly;
    }
    let mut templates = Vec::new();
    for _ in 0..kv_count {
        let Some(key) = c.gguf_string() else {
            return GgufParse::Truncated;
        };
        let Some(typ) = c.u32() else {
            return GgufParse::Truncated;
        };
        let chat = key.ends_with("chat_template");
        match c.take_value(typ, chat, &mut templates) {
            Ok(()) => {}
            Err(true) => return GgufParse::Anomaly,
            Err(false) => return GgufParse::Truncated,
        }
    }
    if templates.len() > 4 {
        templates.truncate(4);
    }
    GgufParse::Ok { templates }
}

struct Cursor<'a> {
    buf: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, i: 0 }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.i + n > self.buf.len() {
            return None;
        }
        let s = &self.buf[self.i..self.i + n];
        self.i += n;
        Some(s)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn gguf_string(&mut self) -> Option<String> {
        let n = self.u64()? as usize;
        if n > 1024 * 1024 {
            return None;
        }
        let bytes = self.take(n)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    /// `Err(true)` = anomaly (hostile length), `Err(false)` = truncated.
    fn take_value(&mut self, typ: u32, capture: bool, out: &mut Vec<String>) -> Result<(), bool> {
        match typ {
            0 | 1 | 7 => self.take(1).map(|_| ()).ok_or(false),
            2 | 3 => self.take(2).map(|_| ()).ok_or(false),
            4 | 5 | 6 => self.take(4).map(|_| ()).ok_or(false),
            10 | 11 | 12 => self.take(8).map(|_| ()).ok_or(false),
            8 => {
                let n = self.u64().ok_or(false)?;
                if n > 1024 * 1024 {
                    return Err(true);
                }
                let bytes = self.take(n as usize).ok_or(false)?;
                if capture && out.len() < 4 {
                    let s = String::from_utf8_lossy(bytes);
                    if s.len() <= 64 * 1024 {
                        out.push(s.into_owned());
                    }
                }
                Ok(())
            }
            9 => {
                let elem = self.u32().ok_or(false)?;
                let count = self.u64().ok_or(false)?;
                if count > 4096 {
                    return Err(true);
                }
                for _ in 0..count {
                    self.take_value(elem, capture, out)?;
                }
                Ok(())
            }
            _ => Err(true),
        }
    }
}

/// AIS207. `numpy-object` | `joblib` | `keras-lambda` | `ok` | `skip`
pub fn model_format_issue(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    let interesting = ext_is(path, "npy")
        || ext_is(path, "npz")
        || ext_is(path, "joblib")
        || ext_is(path, "h5")
        || ext_is(path, "hdf5")
        || ext_is(path, "keras")
        || lower.contains("keras");
    if !interesting {
        return "skip".into();
    }
    let bytes = read_capped(path, SNIFF_CAP as u64).unwrap_or_default();
    if bytes.is_empty() {
        return "ok".into();
    }
    if (ext_is(path, "npy") || ext_is(path, "npz")) && is_numpy_object(&bytes) {
        return "numpy-object".into();
    }
    if is_joblib(path, &bytes) {
        return "joblib".into();
    }
    if is_keras_lambda(path, &bytes) {
        return "keras-lambda".into();
    }
    "ok".into()
}

fn is_numpy_object(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || &bytes[0..6] != b"\x93NUMPY" {
        return false;
    }
    let head = &bytes[..bytes.len().min(512)];
    let text = String::from_utf8_lossy(head);
    text.contains("|O") || text.contains("<O") || text.contains(">O")
}

fn is_joblib(path: &str, bytes: &[u8]) -> bool {
    let pickle = bytes.first() == Some(&0x80);
    let mentions = bytes.windows(6).any(|w| w == b"joblib");
    if ext_is(path, "joblib") {
        return pickle || mentions;
    }
    pickle && mentions
}

fn is_keras_lambda(path: &str, bytes: &[u8]) -> bool {
    let lower_path = path.to_ascii_lowercase();
    let kerasish = ext_is(path, "h5")
        || ext_is(path, "hdf5")
        || ext_is(path, "keras")
        || lower_path.contains("keras");
    if !kerasish {
        return false;
    }
    let text = String::from_utf8_lossy(bytes);
    let lambda = text.contains("\"class_name\": \"Lambda\"")
        || text.contains("\"class_name\":\"Lambda\"")
        || text.contains("'class_name': 'Lambda'");
    let hdf = bytes.len() >= 4 && &bytes[0..4] == b"\x89HDF";
    lambda && (hdf || ext_is(path, "json") || lower_path.ends_with(".json") || text.contains("{"))
}

/// AIS107. `ok` | `branch` | `budget` | `malformed` | `skip`
pub fn chat_template_issue(text: &str) -> String {
    let src = text.trim();
    if src.is_empty() {
        return "skip".into();
    }
    let literals = if_literals(src);
    let mut probes = vec!["hello".to_string(), "invoice".to_string()];
    for lit in literals {
        if !probes.contains(&lit) {
            probes.push(lit);
        }
        if probes.len() >= 8 {
            break;
        }
    }
    let mut rendered: Vec<String> = Vec::new();
    for topic in &probes {
        match render_once(src, topic) {
            Render::Ok(s) => rendered.push(s),
            Render::Budget => return "budget".into(),
            Render::Bad => return "malformed".into(),
        }
    }
    if rendered.windows(2).any(|w| w[0] != w[1]) {
        "branch".into()
    } else {
        "ok".into()
    }
}

enum Render {
    Ok(String),
    Budget,
    Bad,
}

fn render_once(src: &str, topic: &str) -> Render {
    let mut env = minijinja::Environment::new();
    env.set_fuel(Some(10_000));
    if env.add_template("t", src).is_err() {
        return Render::Bad;
    }
    let Ok(tmpl) = env.get_template("t") else {
        return Render::Bad;
    };
    match tmpl.render(minijinja::context! { topic => topic, name => "world" }) {
        Ok(s) => Render::Ok(s),
        Err(err) if err.kind() == minijinja::ErrorKind::OutOfFuel => Render::Budget,
        Err(_) => Render::Bad,
    }
}

fn if_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, _) in src.match_indices("{%") {
        let window = &src[idx..src.len().min(idx + 180)];
        if !window.contains("if") {
            continue;
        }
        collect_quotes(window, &mut out);
        if out.len() >= 8 {
            break;
        }
    }
    out
}

fn collect_quotes(window: &str, out: &mut Vec<String>) {
    let bytes = window.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let quote = bytes[i];
        if quote == b'\'' || quote == b'"' {
            if let Some(end) = window[i + 1..].find(quote as char) {
                let lit = &window[i + 1..i + 1 + end];
                if !lit.is_empty() && lit.len() <= 40 && !out.iter().any(|s| s == lit) {
                    out.push(lit.to_string());
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_st(path: &std::path::Path, header: &str, body: &[u8]) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&(header.len() as u64).to_le_bytes()).unwrap();
        f.write_all(header.as_bytes()).unwrap();
        f.write_all(body).unwrap();
    }

    #[test]
    fn safetensors_overlap_and_ok() {
        let dir = std::env::temp_dir().join(format!("lacan-st-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("ok.safetensors");
        let header = r#"{"w":{"dtype":"U8","shape":[4],"data_offsets":[0,4]}}"#;
        write_st(&ok, header, b"abcd");
        assert_eq!(safetensors_header_issue(&ok.to_string_lossy()), "ok");

        let bad = dir.join("bad.safetensors");
        let header = r#"{"a":{"dtype":"U8","shape":[4],"data_offsets":[0,4]},"b":{"dtype":"U8","shape":[4],"data_offsets":[2,6]}}"#;
        write_st(&bad, header, b"abcdef");
        assert_eq!(safetensors_header_issue(&bad.to_string_lossy()), "overlap");

        let over = dir.join("over.safetensors");
        let mut f = fs::File::create(&over).unwrap();
        f.write_all(&20_000_000u64.to_le_bytes()).unwrap();
        assert_eq!(safetensors_header_issue(&over.to_string_lossy()), "oversize");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_template_branch_and_stable() {
        let branch = "{% if topic == 'invoice' %}SECRET{% else %}hello{% endif %}\n";
        assert_eq!(chat_template_issue(branch), "branch");
        assert_eq!(chat_template_issue("Hello {{ name }}\n"), "ok");
        assert_eq!(chat_template_issue("{% if\n"), "malformed");
        let budget = "{% for i in range(100000) %}{% for j in range(100000) %}x{% endfor %}{% endfor %}\n";
        assert_eq!(chat_template_issue(budget), "budget");
    }

    #[test]
    fn gguf_roundtrip_template() {
        let dir = std::env::temp_dir().join(format!("lacan-gguf-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.gguf");
        let key = b"tokenizer.chat_template";
        let val = b"Hello {{ name }}";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&(val.len() as u64).to_le_bytes());
        bytes.extend_from_slice(val);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(gguf_header_issue(&path.to_string_lossy()), "ok");
        let got = gguf_chat_templates(&path.to_string_lossy());
        assert_eq!(got, vec!["Hello {{ name }}".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }
}
