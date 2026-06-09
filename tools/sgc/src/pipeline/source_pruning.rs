use std::collections::HashMap;

pub(super) struct SourcePruneResult {
    pub source: String,
    pub removed_functions: usize,
}

#[derive(Clone)]
struct FunctionSlice<'source> {
    name: &'source str,
    range: std::ops::Range<usize>,
    body: std::ops::Range<usize>,
}

pub(super) fn prune_unreachable_plain_source_functions(
    source: &str,
    min_functions: usize,
) -> Option<SourcePruneResult> {
    if source.len() < 1_000_000 || !plain_source_prefilter_allows(source) {
        return None;
    }

    let functions = scan_plain_top_level_functions(source)?;
    if functions.len() < min_functions {
        return None;
    }

    let mut index_by_name = HashMap::with_capacity(functions.len());
    for (idx, function) in functions.iter().enumerate() {
        if index_by_name.insert(function.name, idx).is_some() {
            return None;
        }
    }

    let &main_index = index_by_name.get("main")?;
    let mut reachable = vec![false; functions.len()];
    let mut stack = Vec::with_capacity(8);
    stack.push(main_index);
    for root_async_helper in [
        "main__async_body",
        "main__start",
        "main__poll",
        "main__result",
    ] {
        if let Some(&idx) = index_by_name.get(root_async_helper) {
            stack.push(idx);
        }
    }

    while let Some(idx) = stack.pop() {
        if reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        collect_call_targets(
            &source[functions[idx].body.clone()],
            &index_by_name,
            &mut stack,
            &reachable,
        );
    }

    let reachable_count = reachable.iter().filter(|&&value| value).count();
    if reachable_count == 0 || reachable_count == functions.len() {
        return None;
    }

    let mut pruned_source = String::with_capacity(
        functions
            .iter()
            .zip(reachable.iter())
            .filter_map(|(function, keep)| keep.then_some(function.range.len() + 2))
            .sum(),
    );
    for (function, keep) in functions.iter().zip(reachable.iter()) {
        if *keep {
            if !pruned_source.is_empty() {
                pruned_source.push_str("\n\n");
            }
            pruned_source.push_str(&source[function.range.clone()]);
        }
    }
    pruned_source.push('\n');

    Some(SourcePruneResult {
        source: pruned_source,
        removed_functions: functions.len() - reachable_count,
    })
}

fn plain_source_prefilter_allows(source: &str) -> bool {
    !source.contains('"')
        && !source.contains('\'')
        && !source.contains('#')
        && !source.contains("//")
        && !source.contains("/*")
        && !source.contains("import")
        && !source.contains("extern")
        && !source.contains("requires")
        && !source.contains("ensures")
        && !source.contains("class")
        && !source.contains("struct")
        && !source.contains("enum")
        && !source.contains("trait")
        && !source.contains("impl")
        && !source.contains("macro_rules!")
}

fn scan_plain_top_level_functions(source: &str) -> Option<Vec<FunctionSlice<'_>>> {
    let bytes = source.as_bytes();
    let mut functions = Vec::new();
    let mut cursor = 0usize;

    loop {
        cursor = skip_ws(bytes, cursor);
        if cursor >= bytes.len() {
            break;
        }

        let function_start = cursor;
        if !starts_with_keyword(bytes, cursor, b"def") {
            return None;
        }
        cursor += 3;
        cursor = skip_ws(bytes, cursor);

        let name_start = cursor;
        if bytes
            .get(cursor)
            .copied()
            .is_none_or(|byte| !is_ident_start(byte))
        {
            return None;
        }
        cursor += 1;
        while bytes.get(cursor).copied().is_some_and(is_ident_continue) {
            cursor += 1;
        }
        let name = &source[name_start..cursor];

        let open_brace = find_next_byte(bytes, cursor, b'{')?;
        let close_brace = find_matching_brace(bytes, open_brace)?;
        functions.push(FunctionSlice {
            name,
            range: function_start..close_brace,
            body: (open_brace + 1)..(close_brace - 1),
        });
        cursor = close_brace;
    }

    Some(functions)
}

fn collect_call_targets(
    body: &str,
    index_by_name: &HashMap<&str, usize>,
    stack: &mut Vec<usize>,
    reachable: &[bool],
) {
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if !is_ident_start(bytes[cursor]) {
            cursor += 1;
            continue;
        }

        let start = cursor;
        cursor += 1;
        while bytes.get(cursor).copied().is_some_and(is_ident_continue) {
            cursor += 1;
        }
        if let Some(&idx) = index_by_name.get(&body[start..cursor]) {
            if !reachable[idx] {
                stack.push(idx);
            }
        }
    }
}

fn find_next_byte(bytes: &[u8], mut cursor: usize, target: u8) -> Option<usize> {
    while cursor < bytes.len() {
        if bytes[cursor] == target {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn find_matching_brace(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = open;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn starts_with_keyword(bytes: &[u8], cursor: usize, keyword: &[u8]) -> bool {
    bytes
        .get(cursor..cursor + keyword.len())
        .is_some_and(|value| value == keyword)
        && bytes
            .get(cursor + keyword.len())
            .copied()
            .is_none_or(|byte| !is_ident_continue(byte))
}

fn skip_ws(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .copied()
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        cursor += 1;
    }
    cursor
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::prune_unreachable_plain_source_functions;

    #[test]
    fn source_prune_keeps_main_reachable_functions() {
        let mut source = String::new();
        for idx in 0..80_000 {
            source.push_str(&format!(
                "def dead{idx}(x: i64) -> i64 {{\n    x + 1\n}}\n\n"
            ));
        }
        source.push_str("def keep(x: i64) -> i64 {\n    x + 2\n}\n\n");
        source.push_str("def main() -> i64 {\n    keep(1)\n}\n");

        let result = prune_unreachable_plain_source_functions(&source, 1)
            .expect("plain source should prune");

        assert!(result.source.contains("def main"));
        assert!(result.source.contains("def keep"));
        assert!(!result.source.contains("def dead"));
        assert!(result.removed_functions > 0);
    }

    #[test]
    fn source_prune_rejects_non_plain_source() {
        let source = "import std::io;\ndef main() -> i64 { 0 }\n";
        assert!(prune_unreachable_plain_source_functions(source, 1).is_none());
    }

    #[test]
    fn source_prune_rejects_contract_clauses() {
        let source = "def helper() -> bool {\n    true\n}\n\n\
def main() -> i64\nrequires helper()\n{\n    1\n}\n\n"
            .repeat(20_000);

        assert!(prune_unreachable_plain_source_functions(&source, 1).is_none());
    }
}
