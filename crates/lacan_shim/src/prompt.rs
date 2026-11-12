//! AIS104: retrieved or user data reaches a system prompt (same function).
//! Python and TypeScript only. Parse errors return 0.
// lacan:skip=AIS104:unit tests quote retrieval into a system prompt

use std::collections::HashSet;

use tree_sitter::{Language, Node, Parser};

pub fn prompt_sink_line(path: &str, text: &str) -> i64 {
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
            if reaches_untrusted(rhs, src, &scopes[scope]) {
                for name in assigned_names(node, src) {
                    scopes[scope].insert(name);
                }
            }
            if let Some(line) = system_assign_sink(node, src, &scopes[scope]) {
                return Some(line);
            }
        }
    }
    if let Some(line) = system_call_sink(node, src, &scopes[scope]) {
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

fn reaches_untrusted(node: Node, src: &[u8], tainted: &HashSet<String>) -> bool {
    if is_untrusted_source(node, src) || ident_tainted(node, src, tainted) {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if reaches_untrusted(child, src, tainted) {
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

fn is_untrusted_source(node: Node, src: &[u8]) -> bool {
    if node.kind() == "call" || node.kind() == "call_expression" {
        if let Some(callee) = node.child_by_field_name("function") {
            if let Ok(text) = callee.utf8_text(src) {
                let lower = text.to_ascii_lowercase();
                if lower.contains("get_relevant_documents")
                    || lower.contains("similarity_search")
                    || lower.contains("similaritysearch")
                    || lower.contains("aget_relevant_documents")
                    || lower.ends_with(".retrieve")
                    || lower == "retrieve"
                    || lower == "input"
                {
                    return true;
                }
            }
        }
    }
    if node.kind() == "attribute" || node.kind() == "member_expression" {
        if let Ok(text) = node.utf8_text(src) {
            let lower = text.to_ascii_lowercase();
            if lower.ends_with("request.json")
                || lower.ends_with("request.body")
                || lower.ends_with("req.body")
                || lower.ends_with("req.json")
            {
                return true;
            }
        }
    }
    false
}

fn is_system_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "system_prompt"
        || lower == "systemprompt"
        || lower == "system_message"
        || lower == "systemmessage"
        || lower == "sys_prompt"
        || lower == "sysprompt"
}

fn system_assign_sink(node: Node, src: &[u8], tainted: &HashSet<String>) -> Option<i64> {
    let rhs = assignment_value(node)?;
    if !reaches_untrusted(rhs, src, tainted) {
        return None;
    }
    if assigned_names(node, src)
        .iter()
        .any(|n| is_system_name(n))
    {
        Some(node.start_position().row as i64 + 1)
    } else {
        None
    }
}

fn system_call_sink(node: Node, src: &[u8], tainted: &HashSet<String>) -> Option<i64> {
    if node.kind() != "call" && node.kind() != "call_expression" {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() == "keyword_argument" {
            let name = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("");
            let value = child.child_by_field_name("value")?;
            if is_system_name(name) && reaches_untrusted(value, src, tainted) {
                return Some(node.start_position().row as i64 + 1);
            }
        }
        // TS object: system: tainted or systemPrompt: tainted inside with:/args
        if child.kind() == "object" || child.kind() == "dictionary" {
            if object_has_system_taint(child, src, tainted) {
                return Some(node.start_position().row as i64 + 1);
            }
        }
        if child.kind() == "pair" {
            if pair_is_system_taint(child, src, tainted) {
                return Some(node.start_position().row as i64 + 1);
            }
        }
    }
    None
}

fn object_has_system_taint(node: Node, src: &[u8], tainted: &HashSet<String>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" && pair_is_system_taint(child, src, tainted) {
            return true;
        }
    }
    false
}

fn pair_is_system_taint(pair: Node, src: &[u8], tainted: &HashSet<String>) -> bool {
    let key = pair
        .child_by_field_name("key")
        .or_else(|| pair.named_child(0));
    let value = pair
        .child_by_field_name("value")
        .or_else(|| pair.named_child(1));
    let (Some(key), Some(value)) = (key, value) else {
        return false;
    };
    let Ok(key_text) = key.utf8_text(src) else {
        return false;
    };
    let key_clean = key_text.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    is_system_name(key_clean) && reaches_untrusted(value, src, tainted)
}

#[cfg(test)]
mod tests {
    use super::prompt_sink_line;

    #[test]
    fn retrieval_into_system_prompt() {
        let src = "\
def build(retriever, query):
    docs = retriever.get_relevant_documents(query)
    system_prompt = f\"Answer using: {docs}\"
    return system_prompt
";
        assert_eq!(prompt_sink_line("rag.py", src), 3);
    }

    #[test]
    fn static_system_prompt_clean() {
        let src = "\
def build():
    system_prompt = \"You are helpful\"
    return system_prompt
";
        assert_eq!(prompt_sink_line("rag.py", src), 0);
    }

    #[test]
    fn retrieval_only_in_user_message_clean() {
        let src = "\
def build(retriever, query):
    docs = retriever.get_relevant_documents(query)
    user_message = f\"Context: {docs}\"
    system_prompt = \"You are helpful\"
    return system_prompt, user_message
";
        assert_eq!(prompt_sink_line("rag.py", src), 0);
    }

    #[test]
    fn request_json_into_system_kwarg() {
        let src = "\
def handle(request, client):
    body = request.json
    client.chat.completions.create(model=\"x\", system_prompt=body)
";
        assert!(prompt_sink_line("api.py", src) > 0);
    }

    #[test]
    fn typescript_similarity_search() {
        let src = "\
export function build(retriever: any, query: string) {
  const docs = retriever.similaritySearch(query);
  const systemPrompt = `Answer using: ${docs}`;
  return systemPrompt;
}
";
        assert_eq!(prompt_sink_line("rag.ts", src), 3);
    }

    #[test]
    fn cross_function_clean() {
        let src = "\
def fetch(retriever, query):
    return retriever.get_relevant_documents(query)
def build(docs):
    system_prompt = f\"Answer using: {docs}\"
    return system_prompt
";
        assert_eq!(prompt_sink_line("rag.py", src), 0);
    }

    #[test]
    fn markdown_ignored() {
        assert_eq!(prompt_sink_line("notes.md", "system_prompt = retrieve()\n"), 0);
    }
}
