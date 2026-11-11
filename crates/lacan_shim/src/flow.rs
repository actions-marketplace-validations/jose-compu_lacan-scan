//! Intra-function flow for AIS301: an LLM value reaches eval/exec/shell.
//! Other languages, and any parse error, return 0. There is no regex fallback.
// lacan:skip=AIS301:unit tests quote an LLM call and a sink in this file

use std::collections::HashSet;

use tree_sitter::{Language, Node, Parser};

pub fn llm_sink_line(path: &str, text: &str) -> i64 {
    let Some(language) = language_for(path) else {
        return 0;
    };
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return 0;
    }
    let Some(tree) = parser.parse(text, None) else {
        return 0;
    };
    let root = tree.root_node();
    if root.has_error() {
        return 0;
    }
    let src = text.as_bytes();
    let mut ranges = Vec::new();
    collect_bodies(root, &mut ranges);
    let mut scopes = vec![HashSet::new(); ranges.len() + 1];
    walk(root, src, &ranges, &mut scopes).unwrap_or(0)
}

fn language_for(path: &str) -> Option<Language> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "ts" | "js" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        _ => None,
    }
}

fn collect_bodies(node: Node, out: &mut Vec<(usize, usize)>) {
    if matches!(
        node.kind(),
        "function_definition"
            | "function_declaration"
            | "method_definition"
            | "generator_function_declaration"
            | "arrow_function"
            | "lambda"
    ) {
        if let Some(body) = node.child_by_field_name("body") {
            out.push((body.start_byte(), body.end_byte()));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_bodies(child, out);
    }
}

/// 0 is the module. Nested functions take the innermost body.
fn enclosing_scope(start: usize, ranges: &[(usize, usize)]) -> usize {
    let mut best: Option<usize> = None;
    let mut best_len = usize::MAX;
    for (i, (from, to)) in ranges.iter().enumerate() {
        if start >= *from && start < *to {
            let len = to - from;
            if len < best_len {
                best_len = len;
                best = Some(i + 1);
            }
        }
    }
    best.unwrap_or(0)
}

fn walk(
    node: Node,
    src: &[u8],
    ranges: &[(usize, usize)],
    scopes: &mut [HashSet<String>],
) -> Option<i64> {
    let scope = enclosing_scope(node.start_byte(), ranges);
    if node.kind() == "assignment" || node.kind() == "variable_declarator" {
        if let Some(rhs) = assignment_value(node) {
            if reaches(rhs, src, &scopes[scope]) {
                for name in assigned_names(node, src) {
                    scopes[scope].insert(name);
                }
            }
        }
    }
    if let Some(line) = sink_line(node, src, &scopes[scope]) {
        return Some(line);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(line) = walk(child, src, ranges, scopes) {
            return Some(line);
        }
    }
    None
}

fn assignment_value(node: Node) -> Option<Node> {
    match node.kind() {
        "assignment" => node.child_by_field_name("right"),
        "variable_declarator" => node.child_by_field_name("value"),
        _ => None,
    }
}

fn assigned_names(node: Node, src: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let slot = match node.kind() {
        "assignment" => node.child_by_field_name("left"),
        "variable_declarator" => node.child_by_field_name("name"),
        _ => None,
    };
    if let Some(slot) = slot {
        push_idents(slot, src, &mut names);
    }
    names
}

fn push_idents(node: Node, src: &[u8], names: &mut Vec<String>) {
    if node.kind() == "identifier" {
        if let Ok(text) = node.utf8_text(src) {
            names.push(text.to_string());
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(src) {
                names.push(text.to_string());
            }
        }
    }
}

fn reaches(node: Node, src: &[u8], tainted: &HashSet<String>) -> bool {
    if is_llm_source(node, src) || ident_tainted(node, src, tainted) {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if reaches(child, src, tainted) {
            return true;
        }
    }
    false
}

fn ident_tainted(node: Node, src: &[u8], tainted: &HashSet<String>) -> bool {
    if node.kind() != "identifier" {
        return false;
    }
    node.utf8_text(src)
        .ok()
        .is_some_and(|name| tainted.contains(name))
}

/// Callee text of an LLM call, or a `.choices` completion payload.
fn is_llm_source(node: Node, src: &[u8]) -> bool {
    if node.kind() == "call" || node.kind() == "call_expression" {
        if let Some(callee) = node.child_by_field_name("function") {
            if let Ok(text) = callee.utf8_text(src) {
                if text.contains("chat.completions")
                    || text.contains("messages.create")
                    || text.contains("ChatCompletion")
                {
                    return true;
                }
                if text.contains(".invoke") {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("llm")
                        || lower.contains("openai")
                        || lower.contains("anthropic")
                    {
                        return true;
                    }
                }
            }
        }
    }
    if node.kind() == "attribute" || node.kind() == "member_expression" {
        if let Ok(text) = node.utf8_text(src) {
            if text.contains(".choices") {
                return true;
            }
        }
    }
    false
}

fn sink_line(node: Node, src: &[u8], tainted: &HashSet<String>) -> Option<i64> {
    if node.kind() != "call" && node.kind() != "call_expression" {
        return None;
    }
    let callee = node.child_by_field_name("function")?;
    let text = callee.utf8_text(src).ok()?;
    if !is_sink_callee(text) {
        return None;
    }
    let arg = first_arg(node)?;
    if reaches(arg, src, tainted) {
        Some(node.start_position().row as i64 + 1)
    } else {
        None
    }
}

fn is_sink_callee(callee: &str) -> bool {
    match callee {
        "eval" | "exec" | "execSync" | "spawn" | "os.system" => true,
        "subprocess.run" | "subprocess.Popen" | "subprocess.call" => true,
        "child_process.exec" | "child_process.execSync" | "child_process.spawn" => true,
        _ => false,
    }
}

fn first_arg(call: Node) -> Option<Node> {
    let args = call.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        match child.kind() {
            "(" | ")" | "," | "comment" | "keyword_argument" => continue,
            _ => return Some(child),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::llm_sink_line;

    #[test]
    fn choices_then_eval() {
        let src = "import openai\ntext = response.choices[0].message.content\neval(text)\n";
        assert_eq!(llm_sink_line("agent.py", src), 3);
    }

    #[test]
    fn eval_literal_clean() {
        assert_eq!(llm_sink_line("math.py", "eval('1+1')\n"), 0);
    }

    #[test]
    fn anthropic_and_unrelated_subprocess() {
        let src = "import anthropic\nimport subprocess\nsubprocess.run(cmd, shell=True)\n";
        assert_eq!(llm_sink_line("run.py", src), 0);
    }

    #[test]
    fn markdown_is_not_parsed() {
        let src = "openai eval(\n";
        assert_eq!(llm_sink_line("notes.md", src), 0);
    }

    #[test]
    fn parse_error_returns_zero() {
        assert_eq!(llm_sink_line("agent.py", "def (\n"), 0);
    }

    #[test]
    fn arrow_functions_do_not_share_taint() {
        let src = "\
const produce = (client) => {
  const text = client.chat.completions.create({ messages: [] }).choices[0].message.content;
};
const consume = () => {
  eval(text);
};
";
        assert_eq!(llm_sink_line("flow.ts", src), 0);
    }

    #[test]
    fn arrow_function_keeps_its_own_flow() {
        let src = "\
const run = (client) => {
  const text = client.chat.completions.create({ messages: [] }).choices[0].message.content;
  eval(text);
};
";
        assert_eq!(llm_sink_line("flow.ts", src), 3);
    }

    #[test]
    fn cross_function_does_not_flow() {
        let src = "\
def produce(client):
    text = client.chat.completions.create(model='x', messages=[]).choices[0].message.content
def consume():
    eval(text)
";
        assert_eq!(llm_sink_line("agent.py", src), 0);
    }

    #[test]
    fn same_function_python_flow() {
        let src = "\
def run(client):
    resp = client.chat.completions.create(model='x', messages=[])
    text = resp.choices[0].message.content
    eval(text)
";
        assert_eq!(llm_sink_line("flow.py", src), 4);
    }

    #[test]
    fn same_function_typescript_flow() {
        let src = "\
import * as child_process from \"child_process\";
export function run(client: any) {
  const resp = client.chat.completions.create({ messages: [] });
  const text = resp.choices[0].message.content;
  child_process.exec(text);
}
";
        assert_eq!(llm_sink_line("flow.ts", src), 5);
    }

    #[test]
    fn content_attribute_of_tainted_name() {
        let src = "\
def run(client):
    resp = client.messages.create(model='x', messages=[])
    eval(resp.content)
";
        assert_eq!(llm_sink_line("flow.py", src), 3);
    }

    #[test]
    fn invoke_on_llm_receiver() {
        let src = "\
def run(llm):
    text = llm.invoke(prompt)
    os.system(text)
";
        assert_eq!(llm_sink_line("flow.py", src), 3);
    }
}
