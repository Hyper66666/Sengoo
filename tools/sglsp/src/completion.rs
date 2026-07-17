use std::collections::{HashMap, HashSet};

use serde_json::Map;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Position, Range, TextEdit, Url,
};

use crate::completion_context::{AttributeNesting, AttributeTarget, CompletionContext, ImportForm};
use crate::protocol::{
    CompletionCategory, ResolveKind, SengooCompletionDataV1, SymbolOrigin,
    COMPLETION_SCHEMA_VERSION,
};
use crate::signatures::FunctionSignatureInfo;
use crate::stdlib::{stdlib_module_names, stdlib_symbols_for_module};
use crate::text_editing::{position_to_byte_index, span_to_range};
use crate::workspace_index::WorkspaceIndex;

fn completion_data(
    uri: &Url,
    revision: Option<i32>,
    symbol_id: String,
    origin: SymbolOrigin,
    category: CompletionCategory,
    resolve_kind: ResolveKind,
) -> Option<serde_json::Value> {
    serde_json::to_value(SengooCompletionDataV1 {
        schema_version: COMPLETION_SCHEMA_VERSION,
        symbol_id,
        origin,
        category,
        document_uri: uri.clone(),
        document_revision: revision?,
        resolve_kind,
        extensions: Map::new(),
    })
    .ok()
}

#[derive(Debug, Clone)]
struct Binding {
    name: String,
    ty: Option<String>,
    category: CompletionCategory,
}

fn current_token(content: &str, position: Position) -> Option<(String, Range)> {
    let cursor = position_to_byte_index(content, position)?;
    let mut start = cursor;
    while start > 0 {
        let ch = content[..start].chars().next_back()?;
        if ch == '_' || ch.is_alphanumeric() {
            start -= ch.len_utf8();
        } else {
            break;
        }
    }
    let mut end = cursor;
    while end < content.len() {
        let ch = content[end..].chars().next()?;
        if ch == '_' || ch.is_alphanumeric() {
            end += ch.len_utf8();
        } else {
            break;
        }
    }
    Some((
        content[start..cursor].to_string(),
        span_to_range(content, start as u32, end as u32),
    ))
}

fn base_type(ty: &str) -> String {
    ty.trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .split('<')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_binding(fragment: &str, category: CompletionCategory) -> Option<Binding> {
    let fragment = fragment.trim().trim_start_matches("mut ").trim();
    let name_end = fragment
        .find([':', '=', ',', ')', ';'])
        .unwrap_or(fragment.len());
    let name = fragment[..name_end].trim();
    if name.is_empty() || !name.chars().all(|ch| ch == '_' || ch.is_alphanumeric()) {
        return None;
    }
    let explicit_ty = fragment[name_end..]
        .strip_prefix(':')
        .map(|rest| {
            rest.split(['=', ',', ')', ';'])
                .next()
                .unwrap_or_default()
                .trim()
        })
        .filter(|ty| !ty.is_empty())
        .map(base_type);
    let inferred_ty = fragment
        .split_once('=')
        .map(|(_, expression)| {
            expression
                .trim()
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':'))
                .collect::<String>()
        })
        .filter(|ty| !ty.is_empty())
        .map(|ty| base_type(&ty));
    Some(Binding {
        name: name.into(),
        ty: explicit_ty.or(inferred_ty),
        category,
    })
}

fn resolve_type_identity(
    index: &WorkspaceIndex,
    uri: &Url,
    ty: &str,
    signatures: Option<&[FunctionSignatureInfo]>,
) -> Option<String> {
    let ty = base_type(ty);
    signatures.map_or_else(
        || index.canonical_signature_qualifier(uri, &ty),
        |signatures| index.canonical_signature_qualifier_from(uri, &ty, signatures),
    )
}

fn candidate_member_type_identity(
    index: &WorkspaceIndex,
    candidate: &crate::workspace_index::IndexedCompletionCandidate,
) -> Option<String> {
    let container = candidate.symbol.container.as_deref()?;
    let module = index.module_identity(&candidate.definition_uri)?;
    Some(format!("{}::{}", module.import_path, base_type(container)))
}

fn candidate_value_type(
    index: &WorkspaceIndex,
    uri: &Url,
    candidate: &crate::workspace_index::IndexedCompletionCandidate,
    signatures: Option<&[FunctionSignatureInfo]>,
) -> Option<String> {
    let raw = if let Some(field) = candidate.semantic_detail.strip_prefix("field:") {
        field.trim()
    } else {
        candidate
            .semantic_detail
            .split_once("->")?
            .1
            .split('{')
            .next()?
            .trim()
    };
    let raw = raw.split_whitespace().next()?;
    if raw.contains("::") {
        return resolve_type_identity(index, uri, raw, signatures);
    }
    let module = index.module_identity(&candidate.definition_uri)?;
    let local = format!("{}::{}", module.import_path, base_type(raw));
    let exists = index.completion_candidates(uri).into_iter().any(|known| {
        known.symbol.name == base_type(raw)
            && index
                .module_identity(&known.definition_uri)
                .is_some_and(|identity| identity.import_path == module.import_path)
    });
    exists
        .then_some(local)
        .or_else(|| resolve_type_identity(index, uri, raw, signatures))
}

fn receiver_expression_type(
    index: &WorkspaceIndex,
    uri: &Url,
    bindings: &[Binding],
    expression: &str,
    signatures: Option<&[FunctionSignatureInfo]>,
) -> Option<String> {
    let mut segments = expression.split('.');
    let root = segments.next()?;
    let root_name = root.trim_end_matches("()");
    let mut current =
        if let Some(binding) = bindings.iter().find(|binding| binding.name == root_name) {
            let inferred = binding.ty.as_deref()?;
            if let Some(identity) = resolve_type_identity(index, uri, inferred, signatures) {
                identity
            } else {
                let functions = index
                    .completion_candidates(uri)
                    .into_iter()
                    .filter(|candidate| {
                        candidate.symbol.name == inferred
                            && candidate.symbol.kind == CompletionItemKind::FUNCTION
                    })
                    .collect::<Vec<_>>();
                if functions.len() != 1 {
                    return None;
                }
                candidate_value_type(index, uri, &functions[0], signatures)?
            }
        } else if root.ends_with("()") {
            let functions = index
                .completion_candidates(uri)
                .into_iter()
                .filter(|candidate| {
                    candidate.symbol.name == root_name
                        && candidate.symbol.kind == CompletionItemKind::FUNCTION
                })
                .collect::<Vec<_>>();
            if functions.len() != 1 {
                return None;
            }
            candidate_value_type(index, uri, &functions[0], signatures)?
        } else {
            return None;
        };
    for segment in segments {
        let name = segment.trim_end_matches("()");
        let matches = index
            .completion_candidates(uri)
            .into_iter()
            .filter(|candidate| {
                candidate.symbol.name == name
                    && candidate_member_type_identity(index, candidate).as_deref()
                        == Some(current.as_str())
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return None;
        }
        current = candidate_value_type(index, uri, &matches[0], signatures)?;
    }
    Some(current)
}

fn visible_bindings(content: &str, position: Position) -> Vec<Binding> {
    let Some(cursor) = position_to_byte_index(content, position) else {
        return Vec::new();
    };
    let prefix = &content[..cursor];
    let prefix_mask = code_mask(prefix);
    let Some(def_start) = prefix
        .match_indices("def ")
        .filter(|(start, _)| prefix_mask[*start..*start + 4].iter().all(|code| *code))
        .map(|(start, _)| start)
        .last()
    else {
        return Vec::new();
    };
    let function = &prefix[def_start..];
    let Some(body_open) = function.find('{') else {
        return Vec::new();
    };
    let mut bindings = function
        .find('(')
        .and_then(|open| function[open + 1..].split(')').next())
        .map(|params| {
            params
                .split(',')
                .filter_map(|part| parse_binding(part, CompletionCategory::Parameter))
                .filter(|binding| binding.name != "self")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let body = &function[body_open..];
    let mask = code_mask(body);
    let bytes = body.as_bytes();
    let mut scopes = Vec::<Vec<Binding>>::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if !mask[i] {
            i += 1;
            continue;
        }
        match bytes[i] {
            b'{' => {
                let header_start = body[..i]
                    .rfind(['{', '}', '\n'])
                    .map_or(0, |start| start + 1);
                let pattern_bindings = bindings_for_block_header(&body[header_start..i]);
                scopes.push(pattern_bindings);
            }
            b'}' => {
                scopes.pop();
            }
            _ if bytes[i..].starts_with(b"let ")
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) =>
            {
                let end = bytes[i + 4..]
                    .iter()
                    .position(|byte| matches!(byte, b';' | b'\n' | b'\r'))
                    .map_or(bytes.len(), |offset| i + 4 + offset);
                if let (Some(scope), Some(binding)) = (
                    scopes.last_mut(),
                    parse_binding(&body[i + 4..end], CompletionCategory::LocalVariable),
                ) {
                    scope.retain(|known| known.name != binding.name);
                    scope.push(binding);
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    if scopes.is_empty() {
        return Vec::new();
    }
    for scope in scopes {
        for binding in scope {
            bindings.retain(|known| known.name != binding.name);
            bindings.push(binding);
        }
    }
    bindings
}

fn bindings_for_block_header(header: &str) -> Vec<Binding> {
    let mut names = Vec::new();
    let header = header.trim();
    if let Some(rest) = header.rsplit_once("for ").map(|(_, rest)| rest) {
        if let Some(name) = rest.split_whitespace().next() {
            names.push(name);
        }
    }
    if let Some(rest) = header.rsplit_once("if let ").map(|(_, rest)| rest) {
        if let Some(name) = rest
            .split(|ch: char| ch.is_whitespace() || ch == '=')
            .find(|name| !name.is_empty())
        {
            names.push(name);
        }
    }
    if let Some((pattern, _)) = header.rsplit_once("=>") {
        let pattern = pattern.rsplit_once('{').map_or(pattern, |(_, tail)| tail);
        for token in pattern.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
            if token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
                && token != "_"
            {
                names.push(token);
            }
        }
    }
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| parse_binding(name, CompletionCategory::LocalVariable))
        .collect()
}

fn code_mask(content: &str) -> Vec<bool> {
    let bytes = content.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut i = 0usize;
    let mut string = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while i < bytes.len() {
        if line_comment {
            mask[i] = false;
            if bytes[i] == b'\n' {
                line_comment = false;
            }
            i += 1;
        } else if block_comment {
            mask[i] = false;
            if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                mask[i + 1] = false;
                block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
        } else if string {
            mask[i] = false;
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                mask[i + 1] = false;
                i += 2;
            } else {
                if bytes[i] == b'"' {
                    string = false;
                }
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            mask[i] = false;
            mask[i + 1] = false;
            line_comment = true;
            i += 2;
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            mask[i] = false;
            mask[i + 1] = false;
            block_comment = true;
            i += 2;
        } else if bytes[i] == b'"' {
            mask[i] = false;
            string = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    mask
}

fn enclosing_impl_type(content: &str, position: Position) -> Option<String> {
    let cursor = position_to_byte_index(content, position)?;
    let prefix = &content[..cursor];
    let impl_start = prefix.rfind("impl ")?;
    let def_start = prefix.rfind("def ")?;
    if impl_start >= def_start {
        return None;
    }
    let impl_prefix = &prefix[impl_start..def_start];
    let mask = code_mask(impl_prefix);
    let brace_balance = impl_prefix
        .bytes()
        .enumerate()
        .filter(|(index, _)| mask[*index])
        .fold(0i32, |balance, (_, byte)| match byte {
            b'{' => balance + 1,
            b'}' => balance - 1,
            _ => balance,
        });
    if brace_balance <= 0 {
        return None;
    }
    let target = prefix[impl_start + 5..]
        .split('{')
        .next()?
        .split_whitespace()
        .last()?;
    let target = base_type(target);
    (!target.is_empty()).then_some(target)
}

pub(crate) fn signature_receiver_type(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    position: Position,
    expression: &str,
    signatures: &[FunctionSignatureInfo],
) -> Option<String> {
    if expression == "self" {
        let ty = enclosing_impl_type(content, position)?;
        return resolve_type_identity(index, uri, &ty, Some(signatures));
    }
    let bindings = visible_bindings(content, position);
    receiver_expression_type(index, uri, &bindings, expression, Some(signatures))
}

fn category_rank(category: CompletionCategory) -> u8 {
    match category {
        CompletionCategory::LocalVariable => 0,
        CompletionCategory::Parameter => 1,
        CompletionCategory::Field => 2,
        CompletionCategory::ImportedSymbol => 3,
        CompletionCategory::ProjectSymbol => 4,
        CompletionCategory::StandardLibrary => 5,
        CompletionCategory::Keyword => 6,
    }
}

fn origin_rank(origin: SymbolOrigin) -> u8 {
    match origin {
        SymbolOrigin::CurrentDocument => 0,
        SymbolOrigin::Workspace => 1,
        SymbolOrigin::Dependency => 2,
        SymbolOrigin::StandardLibrary => 3,
    }
}

fn sort_key(
    category: CompletionCategory,
    prefix: &str,
    label: &str,
    origin: SymbolOrigin,
) -> String {
    let prefix_rank = u8::from(label != prefix) + u8::from(!label.starts_with(prefix));
    format!(
        "{:02}:{prefix_rank}:{}:{:02}",
        category_rank(category),
        label.to_ascii_lowercase(),
        origin_rank(origin)
    )
}

#[allow(clippy::too_many_arguments)]
fn item(
    uri: &Url,
    revision: Option<i32>,
    symbol_id: String,
    origin: SymbolOrigin,
    category: CompletionCategory,
    label: String,
    kind: CompletionItemKind,
    detail: String,
    prefix: &str,
    range: Range,
) -> CompletionItem {
    CompletionItem {
        sort_text: Some(sort_key(category, prefix, &label, origin)),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: label.clone(),
        })),
        data: completion_data(
            uri,
            revision,
            symbol_id,
            origin,
            category,
            ResolveKind::Documentation,
        ),
        label,
        kind: Some(kind),
        detail: Some(detail),
        ..Default::default()
    }
}

fn keyword_items(
    uri: &Url,
    revision: Option<i32>,
    prefix: &str,
    range: Range,
) -> Vec<CompletionItem> {
    [
        ("def", "def ${1:name}(${2}) -> ${3:i64} {\n    ${0}\n}"),
        ("struct", "struct ${1:Name} {\n    ${0}\n}"),
        ("let", "let ${1:name} = ${0};"),
        ("const", "const ${1:NAME}: ${2:i64} = ${0};"),
        ("match", "match ${1:value} {\n    ${0}\n}"),
    ]
    .into_iter()
    .filter(|(label, _)| prefix.is_empty() || label.starts_with(prefix))
    .map(|(label, snippet)| CompletionItem {
        label: label.into(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some("Sengoo keyword template".into()),
        sort_text: Some(sort_key(
            CompletionCategory::Keyword,
            prefix,
            label,
            SymbolOrigin::StandardLibrary,
        )),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: snippet.into(),
        })),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        data: completion_data(
            uri,
            revision,
            format!("keyword:{label}"),
            SymbolOrigin::StandardLibrary,
            CompletionCategory::Keyword,
            ResolveKind::Documentation,
        ),
        ..Default::default()
    })
    .collect()
}

#[allow(clippy::too_many_arguments)]
fn import_items(
    index: &WorkspaceIndex,
    uri: &Url,
    document: &crate::workspace_index::IndexedDocument,
    position: Position,
    form: &ImportForm,
    path: &str,
    prefix: &str,
    token_range: Range,
) -> Vec<CompletionItem> {
    let cursor = position_to_byte_index(&document.content, position).unwrap_or_default();
    let before_cursor = &document.content[..cursor];
    if matches!(form, ImportForm::Alias | ImportForm::Wildcard)
        && !before_cursor.trim_end().ends_with(path)
    {
        return Vec::new();
    }
    if matches!(form, ImportForm::Selective) && before_cursor.contains('{') {
        if let Some(module) = path.strip_prefix("std::") {
            return stdlib_symbols_for_module(module)
                .into_iter()
                .filter(|symbol| prefix.is_empty() || symbol.name.starts_with(prefix))
                .map(|symbol| {
                    item(
                        uri,
                        document.revision,
                        format!("stdlib:{module}:{}", symbol.name),
                        SymbolOrigin::StandardLibrary,
                        CompletionCategory::StandardLibrary,
                        symbol.name,
                        symbol.kind,
                        format!("stdlib symbol from std::{module}"),
                        prefix,
                        token_range,
                    )
                })
                .collect();
        }
        return index
            .completion_candidates(uri)
            .into_iter()
            .filter(|candidate| {
                index
                    .module_identity(&candidate.definition_uri)
                    .is_some_and(|identity| identity.import_path == path)
                    && !matches!(
                        candidate.symbol.kind,
                        CompletionItemKind::FIELD | CompletionItemKind::METHOD
                    )
                    && (prefix.is_empty() || candidate.symbol.name.starts_with(prefix))
            })
            .map(|candidate| {
                item(
                    uri,
                    document.revision,
                    candidate.symbol_id,
                    candidate.origin,
                    CompletionCategory::ImportedSymbol,
                    candidate.symbol.name,
                    candidate.symbol.kind,
                    format!("export from {path}"),
                    prefix,
                    token_range,
                )
            })
            .collect();
    }

    let path_start = before_cursor.rfind(path).unwrap_or(cursor);
    let path_range = span_to_range(&document.content, path_start as u32, cursor as u32);
    let mut modules = stdlib_module_names()
        .map(|module| (format!("std::{module}"), SymbolOrigin::StandardLibrary))
        .chain(
            index
                .module_paths(uri)
                .into_iter()
                .map(|module| (module, SymbolOrigin::Workspace)),
        )
        .filter(|(module, _)| path.is_empty() || module.starts_with(path))
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.0.cmp(&right.0));
    modules.dedup();
    modules
        .into_iter()
        .map(|(module, origin)| {
            item(
                uri,
                document.revision,
                format!("module:{module}"),
                origin,
                if origin == SymbolOrigin::StandardLibrary {
                    CompletionCategory::StandardLibrary
                } else {
                    CompletionCategory::ProjectSymbol
                },
                module.clone(),
                CompletionItemKind::MODULE,
                "compiler-accepted Sengoo import path".into(),
                path,
                path_range,
            )
        })
        .collect()
}

fn attribute_items(
    uri: &Url,
    revision: Option<i32>,
    target: &AttributeTarget,
    nesting: &AttributeNesting,
    prefix: &str,
    range: Range,
) -> Vec<CompletionItem> {
    let entries: Vec<(&str, &str, &str)> = match nesting {
        AttributeNesting::Arguments { attribute }
            if attribute == "derive"
                && matches!(
                    target,
                    AttributeTarget::Struct | AttributeTarget::Enum | AttributeTarget::Class
                ) => vec![
            ("Clone", "Clone", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Copy", "Copy", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("PartialEq", "PartialEq", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Eq", "Eq", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("PartialOrd", "PartialOrd", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Ord", "Ord", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Hash", "Hash", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Debug", "Debug", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
            ("Default", "Default", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
        ],
        AttributeNesting::Arguments { attribute } if attribute == "cfg" => vec![
            ("target_os", "target_os = \"${1:windows}\"", "evidence:compiler::tests::attribute_tests::cfg_target_os_filters_false_declarations"),
            ("target_family", "target_family = \"${1:unix}\"", "evidence:compiler::tests::attribute_tests::cfg_target_family_keeps_current_family"),
            ("feature", "feature = \"${1:name}\"", "evidence:compiler::tests::attribute_tests::cfg_feature_can_be_enabled_by_package_feature_context"),
            ("all", "all(${1})", "evidence:compiler::tests::attribute_tests::cfg_all_any_not_composes_predicates"),
            ("any", "any(${1})", "evidence:compiler::tests::attribute_tests::cfg_all_any_not_composes_predicates"),
            ("not", "not(${1})", "evidence:compiler::tests::attribute_tests::cfg_all_any_not_composes_predicates"),
        ],
        AttributeNesting::Arguments { .. } => Vec::new(),
        AttributeNesting::Name => match target {
            AttributeTarget::Struct | AttributeTarget::Enum | AttributeTarget::Class => vec![
                ("derive", "derive(${1:Debug})", "evidence:compiler::tests::generic_typeck_tests::builtin_derives_register_core_trait_impls_for_bounds"),
                ("cfg", "cfg(${1:target_os = \"windows\"})", "evidence:compiler::tests::attribute_tests::cfg_target_os_filters_false_declarations"),
                ("deprecated", "deprecated", "evidence:compiler::tests::attribute_tests::deprecated_use_emits_warning"),
            ],
            AttributeTarget::Function => vec![
                ("cfg", "cfg(${1:target_os = \"windows\"})", "evidence:compiler::tests::attribute_tests::cfg_target_os_filters_false_declarations"),
                ("deprecated", "deprecated", "evidence:compiler::tests::attribute_tests::deprecated_use_emits_warning"),
                ("test", "test", "evidence:sgc::commands::test::discover_tests_expands_test_attributes_into_harnesses"),
                ("case", "case(\"${1:name}\", ${2:value})", "evidence:sgc::commands::test::discover_tests_expands_parameterized_cases_into_harnesses"),
                ("export_name", "export_name = \"${1:symbol}\"", "evidence:compiler::tests::ffi_tests::export_name_attribute_changes_emitted_symbol"),
            ],
            AttributeTarget::ExternBlock => vec![
                ("cfg", "cfg(${1:target_os = \"windows\"})", "evidence:compiler::tests::attribute_tests::cfg_target_os_filters_false_declarations"),
                ("deprecated", "deprecated", "evidence:compiler::tests::attribute_tests::deprecated_use_emits_warning"),
                ("link", "link(name = \"${1:library}\")", "evidence:compiler::native_link_tests::collect_native_link_libraries_dedupes_extern_blocks"),
            ],
            AttributeTarget::Impl => vec![(
                "cfg",
                "cfg(${1:target_os = \"windows\"})",
                "evidence:compiler::tests::attribute_tests::cfg_target_os_filters_false_declarations",
            )],
            AttributeTarget::Unknown => Vec::new(),
        },
    };
    entries
        .into_iter()
        .filter(|(label, _, _)| prefix.is_empty() || label.starts_with(prefix))
        .map(|(label, insertion, detail)| {
            let mut completion = item(
                uri,
                revision,
                format!("attribute:{label}"),
                SymbolOrigin::StandardLibrary,
                CompletionCategory::Keyword,
                label.into(),
                CompletionItemKind::PROPERTY,
                detail.into(),
                prefix,
                range,
            );
            completion.text_edit = Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: insertion.into(),
            }));
            completion.insert_text_format = Some(InsertTextFormat::SNIPPET);
            completion
        })
        .collect()
}

pub(crate) fn safe_import_edit(
    document: &crate::workspace_index::IndexedDocument,
    module: &str,
    symbol: &str,
) -> Option<TextEdit> {
    if document.imports.iter().any(|import| {
        import_exposes_symbol(import, module, symbol)
            || match &import.kind {
                crate::workspace_index::ImportFactKind::Alias { alias } => {
                    import.path != module && alias == symbol
                }
                crate::workspace_index::ImportFactKind::Selective { names } => {
                    import.path != module && names.iter().any(|name| name == symbol)
                }
                _ => false,
            }
    }) {
        return None;
    }
    let newline = if document.content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let lines = document.content.lines().collect::<Vec<_>>();
    let import_path = |source: &str| {
        let body = source.trim().strip_prefix("import ")?;
        body.trim_end_matches(';')
            .split(|ch: char| ch.is_whitespace() || ch == '{' || ch == '*')
            .next()
            .map(str::to_string)
    };
    let mut imports = Vec::new();
    let mut last_header_import = None;
    for (line, source) in lines.iter().enumerate() {
        if let Some(path) = import_path(source) {
            imports.push((line as u32, path));
            last_header_import = Some(line);
        } else if !source.trim_start().starts_with("//") {
            break;
        }
    }
    if imports.is_empty() {
        return Some(TextEdit {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            new_text: format!("import {module};{newline}"),
        });
    }
    let first_import = imports.first()?.0 as usize;
    let block_end = last_header_import? + 1;
    let mut block_start = first_import;
    while block_start > 0 && lines[block_start - 1].trim_start().starts_with("//") {
        block_start -= 1;
    }
    let mut entries = Vec::<(String, String)>::new();
    let mut leading = Vec::new();
    for (line, source) in lines.iter().enumerate().take(block_end).skip(block_start) {
        if source.trim_start().starts_with("//") {
            leading.push(*source);
            continue;
        }
        if let Some((_, path)) = imports
            .iter()
            .find(|(import_line, _)| *import_line == line as u32)
        {
            let mut text = String::new();
            for comment in leading.drain(..) {
                text.push_str(comment);
                text.push_str(newline);
            }
            text.push_str(source);
            entries.push((path.clone(), text));
        }
    }
    entries.push((module.to_string(), format!("import {module};")));
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut replacement = entries
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join(newline);
    replacement.push_str(newline);
    Some(TextEdit {
        range: Range::new(
            Position::new(block_start as u32, 0),
            Position::new(block_end as u32, 0),
        ),
        new_text: replacement,
    })
}

fn import_exposes_symbol(
    import: &crate::workspace_index::ImportFact,
    module: &str,
    symbol: &str,
) -> bool {
    if import.path != module {
        return false;
    }
    match &import.kind {
        crate::workspace_index::ImportFactKind::Simple
        | crate::workspace_index::ImportFactKind::Wildcard => true,
        crate::workspace_index::ImportFactKind::Alias { .. } => false,
        crate::workspace_index::ImportFactKind::Selective { names } => {
            names.iter().any(|name| name == symbol)
        }
    }
}

pub(crate) fn resolve_completion_item(
    index: &WorkspaceIndex,
    mut completion: CompletionItem,
) -> CompletionItem {
    let Some(value) = completion.data.clone() else {
        return completion;
    };
    let Ok(data) = serde_json::from_value::<SengooCompletionDataV1>(value) else {
        return completion;
    };
    if data.schema_version != COMPLETION_SCHEMA_VERSION {
        return completion;
    }
    let Some(document) = index.document(&data.document_uri) else {
        return completion;
    };
    if document.revision != Some(data.document_revision) {
        return completion;
    }
    let candidates = index.completion_candidates(&data.document_uri);
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.symbol_id == data.symbol_id && candidate.origin == data.origin)
    else {
        if data.symbol_id.starts_with("keyword:") || data.symbol_id.starts_with("attribute:") {
            completion.documentation = completion.detail.clone().map(Documentation::String);
        }
        return completion;
    };
    if matches!(
        data.resolve_kind,
        ResolveKind::Documentation | ResolveKind::DocumentationAndAutoImport
    ) {
        if let Some(documentation) =
            index.symbol_documentation(&candidate.definition_uri, &candidate.symbol.name)
        {
            completion.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: documentation,
            }));
        }
    }
    if matches!(
        data.resolve_kind,
        ResolveKind::AutoImport | ResolveKind::DocumentationAndAutoImport
    ) {
        let same_name = candidates
            .iter()
            .filter(|known| known.symbol.name == candidate.symbol.name)
            .count();
        if same_name == 1 {
            if let Some(module) = index
                .module_identity(&candidate.definition_uri)
                .map(|identity| identity.import_path)
            {
                completion.additional_text_edits =
                    safe_import_edit(&document, &module, &candidate.symbol.name)
                        .map(|edit| vec![edit]);
            }
        }
    }
    completion
}

pub(crate) fn completion_items_for_request(
    index: &WorkspaceIndex,
    uri: &Url,
    position: Position,
) -> Vec<CompletionItem> {
    let Some(document) = index.document(uri) else {
        return Vec::new();
    };
    let Some(context) = CompletionContext::classify(&document.content, position) else {
        return Vec::new();
    };
    let Some((prefix, replacement_range)) = current_token(&document.content, position) else {
        return Vec::new();
    };
    let revision = document.revision;
    match &context {
        CompletionContext::ImportPath { form, path } => {
            return import_items(
                index,
                uri,
                &document,
                position,
                form,
                path,
                &prefix,
                replacement_range,
            );
        }
        CompletionContext::Attribute { target, nesting } => {
            return attribute_items(uri, revision, target, nesting, &prefix, replacement_range);
        }
        CompletionContext::Namespace { path } => {
            if let Some(module) = path.strip_prefix("std::") {
                return stdlib_symbols_for_module(module)
                    .into_iter()
                    .filter(|symbol| prefix.is_empty() || symbol.name.starts_with(&prefix))
                    .map(|symbol| {
                        item(
                            uri,
                            revision,
                            format!("stdlib:{module}:{}", symbol.name),
                            SymbolOrigin::StandardLibrary,
                            CompletionCategory::StandardLibrary,
                            symbol.name,
                            symbol.kind,
                            format!("export from std::{module}"),
                            &prefix,
                            replacement_range,
                        )
                    })
                    .collect();
            }
            return index
                .completion_candidates(uri)
                .into_iter()
                .filter(|candidate| {
                    index
                        .module_identity(&candidate.definition_uri)
                        .is_some_and(|identity| identity.import_path == *path)
                        && !matches!(
                            candidate.symbol.kind,
                            CompletionItemKind::FIELD | CompletionItemKind::METHOD
                        )
                        && (prefix.is_empty() || candidate.symbol.name.starts_with(&prefix))
                })
                .map(|candidate| {
                    item(
                        uri,
                        revision,
                        candidate.symbol_id,
                        candidate.origin,
                        CompletionCategory::ProjectSymbol,
                        candidate.symbol.name,
                        candidate.symbol.kind,
                        format!("export from {path}"),
                        &prefix,
                        replacement_range,
                    )
                })
                .collect();
        }
        _ => {}
    }
    let bindings = visible_bindings(&document.content, position);
    let mut items = Vec::new();

    if matches!(context, CompletionContext::General) {
        for binding in &bindings {
            if !prefix.is_empty() && !binding.name.starts_with(&prefix) {
                continue;
            }
            items.push(item(
                uri,
                revision,
                format!("binding:{}", binding.name),
                SymbolOrigin::CurrentDocument,
                binding.category,
                binding.name.clone(),
                CompletionItemKind::VARIABLE,
                binding.ty.clone().unwrap_or_else(|| "binding".into()),
                &prefix,
                replacement_range,
            ));
        }
    }

    let mut seen = HashSet::new();
    let candidates = index.completion_candidates(uri);
    let mut label_counts = HashMap::<String, usize>::new();
    for candidate in &candidates {
        *label_counts
            .entry(candidate.symbol.name.clone())
            .or_default() += 1;
    }
    for candidate in candidates {
        let category = match &context {
            CompletionContext::Member { receiver } => {
                let receiver_type = if receiver == "self" {
                    enclosing_impl_type(&document.content, position)
                        .as_deref()
                        .and_then(|ty| resolve_type_identity(index, uri, ty, None))
                } else {
                    receiver_expression_type(index, uri, &bindings, receiver, None)
                };
                if receiver_type != candidate_member_type_identity(index, &candidate)
                    || !matches!(
                        candidate.symbol.kind,
                        CompletionItemKind::FIELD | CompletionItemKind::METHOD
                    )
                {
                    continue;
                }
                CompletionCategory::Field
            }
            CompletionContext::Namespace { path } => {
                let namespace = path.rsplit("::").next().unwrap_or(path);
                if candidate.symbol.container.as_deref() != Some(namespace) {
                    continue;
                }
                CompletionCategory::ProjectSymbol
            }
            CompletionContext::ImportPath { .. } | CompletionContext::Attribute { .. } => continue,
            CompletionContext::General => {
                if matches!(
                    candidate.symbol.kind,
                    CompletionItemKind::FIELD | CompletionItemKind::METHOD
                ) {
                    continue;
                }
                let imported = index
                    .module_identity(&candidate.definition_uri)
                    .is_some_and(|module| {
                        document.imports.iter().any(|import| {
                            import_exposes_symbol(
                                import,
                                &module.import_path,
                                &candidate.symbol.name,
                            )
                        })
                    });
                if imported {
                    CompletionCategory::ImportedSymbol
                } else if candidate.origin == SymbolOrigin::StandardLibrary {
                    CompletionCategory::StandardLibrary
                } else {
                    CompletionCategory::ProjectSymbol
                }
            }
        };
        if !prefix.is_empty() && !candidate.symbol.name.starts_with(&prefix) {
            continue;
        }
        let identity = (
            candidate.symbol.name.clone(),
            candidate.origin,
            candidate.definition_uri.clone(),
            candidate.symbol_id.clone(),
        );
        if !seen.insert(identity) {
            continue;
        }
        let resolve_kind = if category == CompletionCategory::ProjectSymbol
            && candidate.origin != SymbolOrigin::CurrentDocument
        {
            ResolveKind::DocumentationAndAutoImport
        } else {
            ResolveKind::Documentation
        };
        let symbol_id = candidate.symbol_id.clone();
        let origin = candidate.origin;
        let ambiguous = label_counts
            .get(&candidate.symbol.name)
            .is_some_and(|count| *count > 1);
        let detail = if ambiguous {
            format!(
                "{} — ambiguous origin: {}",
                candidate.symbol.detail, candidate.definition_uri
            )
        } else {
            format!(
                "{} from {}",
                candidate.symbol.detail, candidate.definition_uri
            )
        };
        let mut completion = item(
            uri,
            revision,
            symbol_id.clone(),
            origin,
            category,
            candidate.symbol.name,
            candidate.symbol.kind,
            detail,
            &prefix,
            replacement_range,
        );
        completion.data = completion_data(uri, revision, symbol_id, origin, category, resolve_kind);
        items.push(completion);
    }
    if matches!(context, CompletionContext::General) {
        items.extend(keyword_items(uri, revision, &prefix, replacement_range));
    }
    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SengooCompletionDataV1;
    use crate::workspace_index::IndexCancellation;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Instant;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{Documentation, MarkupKind};

    fn temp_workspace() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sglsp-completion-{suffix}"));
        fs::create_dir_all(root.join("src")).unwrap();
        root
    }

    fn data(item: &CompletionItem) -> SengooCompletionDataV1 {
        serde_json::from_value(item.data.clone().expect("schema-v1 completion data")).unwrap()
    }

    #[test]
    fn every_attribute_capability_names_an_existing_executable_test() {
        let uri = Url::parse("file:///catalog.sg").unwrap();
        let targets = [
            AttributeTarget::Struct,
            AttributeTarget::Enum,
            AttributeTarget::Class,
            AttributeTarget::Function,
            AttributeTarget::ExternBlock,
            AttributeTarget::Impl,
        ];
        let mut capabilities = targets
            .iter()
            .flat_map(|target| {
                attribute_items(
                    &uri,
                    Some(1),
                    target,
                    &AttributeNesting::Name,
                    "",
                    Range::default(),
                )
            })
            .collect::<Vec<_>>();
        for attribute in ["derive", "cfg"] {
            capabilities.extend(attribute_items(
                &uri,
                Some(1),
                &AttributeTarget::Struct,
                &AttributeNesting::Arguments {
                    attribute: attribute.into(),
                },
                "",
                Range::default(),
            ));
        }
        assert!(!capabilities.is_empty());
        let catalog_ids = capabilities
            .into_iter()
            .map(|capability| {
                let detail = capability.detail.expect("evidence detail");
                detail
                    .strip_prefix("evidence:")
                    .expect("stable evidence id")
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../attribute-evidence.json")).unwrap();
        let manifest_ids = manifest
            .iter()
            .map(|entry| {
                for field in ["id", "package", "filter", "expected"] {
                    assert!(
                        entry[field].as_str().is_some_and(|value| !value.is_empty()),
                        "attribute evidence manifest requires {field}"
                    );
                }
                entry["id"].as_str().unwrap().to_string()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(catalog_ids, manifest_ids);
        assert!(attribute_items(
            &uri,
            Some(1),
            &AttributeTarget::Function,
            &AttributeNesting::Arguments {
                attribute: "derive".into(),
            },
            "",
            Range::default(),
        )
        .is_empty());
    }

    #[test]
    fn semantic_completion_is_contextual_ranked_and_utf16_safe() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        let valid = concat!(
            "struct Point {\n",
            "    x: i64,\n",
            "    y: i64,\n",
            "}\n",
            "impl Point {\n",
            "    def norm(self) -> i64 { self.x }\n",
            "}\n",
            "def demo(point: Point) -> i64 {\n",
            "    let local_point: Point = point;\n",
            "    \"😀\";\n",
            "    local_point.norm();\n",
            "}\n",
        );
        assert!(
            sgfmt::format_source(valid, &sgfmt::FormatOptions::default()).is_ok(),
            "semantic completion fixture must parse"
        );
        fs::write(&path, valid).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let indexed = index
            .completion_candidates(&uri)
            .into_iter()
            .map(|candidate| (candidate.symbol.name, candidate.symbol.container))
            .collect::<Vec<_>>();
        assert!(
            indexed.iter().any(|(name, container)| {
                name == "norm" && container.as_deref() == Some("Point")
            }),
            "indexed symbols: {indexed:?}"
        );
        let incomplete = valid
            .replace("\"😀\";", "\"😀\"; loc")
            .replace("local_point.norm();", "local_point.no");
        assert!(index.open(uri.clone(), 7, incomplete, &IndexCancellation::default()));

        let ranked = completion_items_for_request(&index, &uri, Position::new(9, 10));
        let ranked_local = ranked
            .iter()
            .find(|candidate| candidate.label == "local_point")
            .unwrap();
        assert_eq!(
            data(ranked_local).category,
            CompletionCategory::LocalVariable
        );
        let parameter = ranked
            .iter()
            .find(|candidate| candidate.label == "point")
            .unwrap();
        assert_eq!(data(parameter).category, CompletionCategory::Parameter);
        assert!(ranked_local.sort_text < parameter.sort_text);

        let filtered = completion_items_for_request(&index, &uri, Position::new(9, 13));
        let local = filtered
            .iter()
            .find(|candidate| candidate.label == "local_point")
            .unwrap();
        assert_eq!(
            local.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(9, 10), Position::new(9, 13)),
                new_text: "local_point".into(),
            }))
        );

        let members = completion_items_for_request(&index, &uri, Position::new(10, 16));
        assert!(
            members.iter().any(|candidate| candidate.label == "norm"),
            "member labels: {:?}",
            members.iter().map(|item| &item.label).collect::<Vec<_>>()
        );
        assert!(members.iter().any(|candidate| candidate.label == "x"));
        assert!(members
            .iter()
            .all(|candidate| data(candidate).category == CompletionCategory::Field));
        assert!(!members.iter().any(|candidate| candidate.label == "match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_and_attribute_completion_use_executable_capability_catalogs() {
        let root = temp_workspace();
        let path = root.join("src/main.sg");
        fs::write(&path, "def main() -> i64 { 0 }\n").unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();

        assert!(index.open(
            uri.clone(),
            1,
            "import std::co".into(),
            &IndexCancellation::default(),
        ));
        let imports = completion_items_for_request(&index, &uri, Position::new(0, 14));
        let collections = imports
            .iter()
            .find(|item| item.label == "std::collections")
            .expect("known compiler-shipped stdlib module");
        assert_eq!(
            collections.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(0, 7), Position::new(0, 14)),
                new_text: "std::collections".into(),
            }))
        );

        assert!(index.open(
            uri.clone(),
            2,
            "#[der\nstruct Demo { value: i64, }\n".into(),
            &IndexCancellation::default(),
        ));
        let attributes = completion_items_for_request(&index, &uri, Position::new(0, 5));
        let derive = attributes
            .iter()
            .find(|item| item.label == "derive")
            .expect("derive is compiler-backed for structs");
        assert!(derive.detail.as_deref().unwrap().contains("compiler"));
        assert!(!attributes.iter().any(|item| item.label == "test"));

        assert!(index.open(
            uri.clone(),
            3,
            "#[derive(Cl\nstruct Demo { value: i64, }\n".into(),
            &IndexCancellation::default(),
        ));
        let derives = completion_items_for_request(&index, &uri, Position::new(0, 11));
        assert!(derives.iter().any(|item| item.label == "Clone"));
        assert!(!derives.iter().any(|item| item.label == "Serde"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_adds_documentation_and_unique_revision_safe_auto_import() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let main_path = root.join("src/main.sg");
        fs::write(&main_path, "import zeta;\n\ndef main() -> i64 { 0 }\n").unwrap();
        fs::write(
            root.join("src/helper.sg"),
            "/// Returns the project answer.\ndef helper() -> i64 { 42 }\n",
        )
        .unwrap();
        fs::write(root.join("src/zeta.sg"), "def zeta() -> i64 { 0 }\n").unwrap();
        let uri = Url::from_file_path(&main_path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let edited = "import zeta;\n\ndef main() -> i64 { hel }\n";
        assert!(index.open(uri.clone(), 4, edited.into(), &IndexCancellation::default(),));
        let helper = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .expect("unique project completion");

        let resolved = resolve_completion_item(&index, helper.clone());
        assert!(matches!(
            resolved.documentation,
            Some(Documentation::MarkupContent(ref markup))
                if markup.kind == MarkupKind::Markdown
                    && markup.value.contains("Returns the project answer")
        ));
        assert_eq!(
            resolved.additional_text_edits,
            Some(vec![TextEdit {
                range: Range::new(Position::new(0, 0), Position::new(1, 0)),
                new_text: "import demo::helper;\nimport zeta;\n".into(),
            }])
        );

        let mut wrong_origin = helper.clone();
        let mut wrong_data = data(&wrong_origin);
        wrong_data.origin = SymbolOrigin::Dependency;
        wrong_origin.data = Some(serde_json::to_value(wrong_data).unwrap());
        let rejected = resolve_completion_item(&index, wrong_origin);
        assert!(rejected.documentation.is_none());
        assert!(rejected.additional_text_edits.is_none());

        assert!(index.open(
            uri,
            5,
            edited.replace("hel", "helper"),
            &IndexCancellation::default(),
        ));
        let stale = resolve_completion_item(&index, helper);
        assert!(stale.additional_text_edits.is_none());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn auto_import_dedupes_aliases_and_refuses_name_conflicts() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let main_path = root.join("src/main.sg");
        fs::write(
            &main_path,
            "import demo::helper as helper_alias;\n\ndef main() -> i64 { hel }\n",
        )
        .unwrap();
        fs::write(root.join("src/helper.sg"), "def helper() -> i64 { 42 }\n").unwrap();
        let uri = Url::from_file_path(&main_path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.open(
            uri.clone(),
            1,
            "import demo::helper as helper_alias;\n\ndef main() -> i64 { hel }\n".into(),
            &IndexCancellation::default(),
        ));
        let imported = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .unwrap();
        assert_eq!(data(&imported).category, CompletionCategory::ProjectSymbol);
        assert!(resolve_completion_item(&index, imported)
            .additional_text_edits
            .is_some());

        assert!(index.open(
            uri.clone(),
            2,
            "import other as helper;\n\ndef main() -> i64 { hel }\n".into(),
            &IndexCancellation::default(),
        ));
        let conflicting = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .unwrap();
        assert!(resolve_completion_item(&index, conflicting)
            .additional_text_edits
            .is_none());

        assert!(index.open(
            uri.clone(),
            3,
            "import other * from;\n\ndef main() -> i64 { hel }\n".into(),
            &IndexCancellation::default(),
        ));
        let wildcard = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .unwrap();
        assert!(resolve_completion_item(&index, wildcard)
            .additional_text_edits
            .is_some());

        assert!(index.open(
            uri.clone(),
            4,
            "import demo::helper { other };\n\ndef main() -> i64 { hel }\n".into(),
            &IndexCancellation::default(),
        ));
        let not_selected = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .unwrap();
        assert_eq!(
            data(&not_selected).category,
            CompletionCategory::ProjectSymbol
        );

        assert!(index.open(
            uri.clone(),
            5,
            "import demo::helper { helper };\n\ndef main() -> i64 { hel }\n".into(),
            &IndexCancellation::default(),
        ));
        let selected = completion_items_for_request(&index, &uri, Position::new(2, 23))
            .into_iter()
            .find(|item| item.label == "helper")
            .unwrap();
        assert_eq!(data(&selected).category, CompletionCategory::ImportedSymbol);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn auto_import_sorts_comment_attached_import_entries_as_units() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let path = root.join("src/main.sg");
        let source = concat!(
            "// zeta docs\n",
            "import zeta;\n",
            "// beta docs\n",
            "import beta;\n",
            "\n",
            "def main() -> i64 { 0 }\n",
        );
        fs::write(&path, source).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let document = index.document(&uri).unwrap();
        let edit = safe_import_edit(&document, "gamma", "gamma").unwrap();
        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 0), Position::new(4, 0))
        );
        assert_eq!(
            edit.new_text,
            concat!(
                "// beta docs\n",
                "import beta;\n",
                "import gamma;\n",
                "// zeta docs\n",
                "import zeta;\n",
            )
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn auto_import_rewrites_only_the_contiguous_header_import_block() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let path = root.join("src/main.sg");
        let source = concat!(
            "// zeta docs\n",
            "import zeta;\n",
            "\n",
            "def keep() -> i64 { 7 }\n",
            "\n",
            "// later docs\n",
            "import later;\n",
            "def main() -> i64 { keep() }\n",
        );
        fs::write(&path, source).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let document = index.document(&uri).unwrap();
        let edit = safe_import_edit(&document, "alpha", "alpha").unwrap();

        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 0), Position::new(2, 0))
        );
        assert_eq!(edit.new_text, "import alpha;\n// zeta docs\nimport zeta;\n");
        assert_eq!(
            &source[source.find("\ndef keep").unwrap()..],
            "\ndef keep() -> i64 { 7 }\n\n// later docs\nimport later;\ndef main() -> i64 { keep() }\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn auto_import_preserves_crlf_for_a_single_contiguous_header_block() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let path = root.join("src/main.sg");
        let source =
            "// beta docs\r\nimport beta;\r\n\r\ndef keep() -> i64 { 7 }\r\nimport later;\r\n";
        fs::write(&path, source).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        let document = index.document(&uri).unwrap();
        let edit = safe_import_edit(&document, "alpha", "alpha").unwrap();

        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 0), Position::new(2, 0))
        );
        assert_eq!(
            edit.new_text,
            "import alpha;\r\n// beta docs\r\nimport beta;\r\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ambiguous_origins_remain_separate_and_never_auto_import() {
        let root = temp_workspace();
        let main_path = root.join("src/main.sg");
        fs::write(&main_path, "def main() -> i64 { dup }\n").unwrap();
        fs::write(root.join("src/a.sg"), "def duplicate() -> i64 { 1 }\n").unwrap();
        fs::write(root.join("src/b.sg"), "def duplicate() -> i64 { 2 }\n").unwrap();
        let uri = Url::from_file_path(&main_path).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.open(
            uri.clone(),
            1,
            "def main() -> i64 { dup }\n".into(),
            &IndexCancellation::default(),
        ));
        let duplicates = completion_items_for_request(&index, &uri, Position::new(0, 23))
            .into_iter()
            .filter(|item| item.label == "duplicate")
            .collect::<Vec<_>>();
        assert_eq!(duplicates.len(), 2);
        assert!(duplicates.iter().all(|item| {
            item.detail
                .as_deref()
                .is_some_and(|detail| detail.contains("ambiguous origin"))
        }));
        assert!(duplicates.into_iter().all(|item| {
            resolve_completion_item(&index, item)
                .additional_text_edits
                .is_none()
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_module_identity_drives_auto_import_and_selective_exports() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src/nested")).unwrap();
        fs::create_dir_all(dep.join("src/nested")).unwrap();
        fs::write(app.join("Sengoo.toml"), "[package]\nname = \"demo-app\"\n").unwrap();
        fs::write(dep.join("Sengoo.toml"), "[package]\nname = \"dep-pkg\"\n").unwrap();
        fs::write(app.join("src/main.sg"), "def main() -> i64 { dep }\n").unwrap();
        fs::write(
            app.join("src/nested/shared.sg"),
            "def project_export() -> i64 { 1 }\n",
        )
        .unwrap();
        fs::write(
            dep.join("src/nested/shared.sg"),
            "def dependency_export() -> i64 { 2 }\n",
        )
        .unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "demo-app"
[[package]]
name = "dep-pkg"
version = "0.1.0"
source = "path+../dep"
manifest = "../dep/Sengoo.toml"
"#,
        )
        .unwrap();
        let uri = Url::from_file_path(app.join("src/main.sg")).unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        assert!(index.open(
            uri.clone(),
            1,
            "def main() -> i64 { dep }\n".into(),
            &IndexCancellation::default(),
        ));
        let dependency = completion_items_for_request(&index, &uri, Position::new(0, 23))
            .into_iter()
            .find(|item| item.label == "dependency_export")
            .unwrap();
        assert_eq!(
            resolve_completion_item(&index, dependency)
                .additional_text_edits
                .unwrap()[0]
                .new_text,
            "import dep_pkg::nested::shared;\n"
        );

        assert!(index.open(
            uri.clone(),
            2,
            "import demo_app::nested::shared { pro".into(),
            &IndexCancellation::default(),
        ));
        let project = completion_items_for_request(&index, &uri, Position::new(0, 37));
        assert!(project.iter().any(|item| item.label == "project_export"));
        assert!(!project.iter().any(|item| item.label == "dependency_export"));

        assert!(index.open(
            uri.clone(),
            3,
            "import dep_pkg::nested::shared { dep".into(),
            &IndexCancellation::default(),
        ));
        let dependency = completion_items_for_request(&index, &uri, Position::new(0, 36));
        assert!(dependency
            .iter()
            .any(|item| item.label == "dependency_export"));
        assert!(!dependency.iter().any(|item| item.label == "project_export"));

        assert!(index.open(
            uri.clone(),
            4,
            "dep_pkg::nested::shared::dep".into(),
            &IndexCancellation::default(),
        ));
        let namespace = completion_items_for_request(&index, &uri, Position::new(0, 28));
        assert!(namespace
            .iter()
            .any(|item| item.label == "dependency_export"));

        assert!(index.open(
            uri.clone(),
            5,
            "std::collections::Ve".into(),
            &IndexCancellation::default(),
        ));
        let std_namespace = completion_items_for_request(&index, &uri, Position::new(0, 20));
        assert!(std_namespace.iter().any(|item| item.label == "Vec"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lockfile_v2_edges_preserve_registry_and_git_dependency_aliases() {
        let root = temp_workspace();
        let app = root.join("app");
        let registry_dep = root.join("registry-cache/registry-pkg-1.2.3");
        let git_dep = root.join("git-cache/git-pkg-rev123");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(registry_dep.join("src")).unwrap();
        fs::create_dir_all(git_dep.join("src")).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            concat!(
                "[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\n",
                "[dependencies]\n",
                "registry_alias = { package = \"registry-pkg\", version = \"1.2.3\", registry = \"local\" }\n",
                "git_alias = { package = \"git-pkg\", git = \"https://example.invalid/git-pkg\", rev = \"rev123\" }\n",
            ),
        )
        .unwrap();
        fs::write(
            registry_dep.join("Sengoo.toml"),
            "[package]\nname = \"registry-pkg\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        fs::write(
            git_dep.join("Sengoo.toml"),
            "[package]\nname = \"git-pkg\"\nversion = \"0.4.0\"\n",
        )
        .unwrap();
        fs::write(app.join("src/main.sg"), "def main() -> i64 { git }\n").unwrap();
        fs::write(
            registry_dep.join("src/widget.sg"),
            "def registry_export() -> i64 { 1 }\n",
        )
        .unwrap();
        fs::write(
            git_dep.join("src/tools.sg"),
            "def git_export() -> i64 { 2 }\n",
        )
        .unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 2
root = "demo-app"

[[package]]
id = "demo-app@0.1.0#path+."
name = "demo-app"
version = "0.1.0"
source.kind = "path"
source.path = "."
manifest = "Sengoo.toml"

[[package]]
id = "registry-pkg@1.2.3#registry+local"
name = "registry-pkg"
version = "1.2.3"
source.kind = "registry"
source.registry = "local"
source.version = "1.2.3"
manifest = "../registry-cache/registry-pkg-1.2.3/Sengoo.toml"

[[package]]
id = "git-pkg@0.4.0#git+rev123"
name = "git-pkg"
version = "0.4.0"
source.kind = "git"
source.url = "https://example.invalid/git-pkg"
source.rev = "rev123"
manifest = "../git-cache/git-pkg-rev123/Sengoo.toml"

[[dependency]]
from = "demo-app@0.1.0#path+."
alias = "registry_alias"
to = "registry-pkg@1.2.3#registry+local"

[[dependency]]
from = "demo-app@0.1.0#path+."
alias = "git_alias"
to = "git-pkg@0.4.0#git+rev123"
"#,
        )
        .unwrap();

        let uri = Url::from_file_path(app.join("src/main.sg")).unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();
        assert!(index.open(
            uri.clone(),
            1,
            "registry_alias::widget::reg".into(),
            &IndexCancellation::default(),
        ));
        let namespace = completion_items_for_request(&index, &uri, Position::new(0, 27));
        assert!(namespace.iter().any(|item| item.label == "registry_export"));

        assert!(index.open(
            uri.clone(),
            2,
            "def main() -> i64 { git }\n".into(),
            &IndexCancellation::default(),
        ));
        let git_export = completion_items_for_request(&index, &uri, Position::new(0, 23))
            .into_iter()
            .find(|item| item.label == "git_export")
            .expect("git dependency export");
        assert_eq!(
            resolve_completion_item(&index, git_export)
                .additional_text_edits
                .unwrap()[0]
                .new_text,
            "import git_alias::tools;\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn visible_bindings_are_limited_to_enclosing_function_and_open_brace_scopes() {
        let inside = concat!(
            "def old(old_param: i64) -> i64 { let leaked: i64 = 1; leaked }\n",
            "def current(current_param: i64) -> i64 {\n",
            "    let outer: Outer = value;\n",
            "    { let closed: i64 = 0; }\n",
            "    let shadow: Outer = value;\n",
            "    {\n",
            "        let shadow: Inner = value;\n",
            "        sha",
        );
        let inside_position = Position::new(7, 11);
        let bindings = visible_bindings(inside, inside_position);
        let names = bindings
            .iter()
            .map(|binding| binding.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"current_param"));
        assert!(names.contains(&"outer"));
        assert!(!names.contains(&"old_param"));
        assert!(!names.contains(&"leaked"));
        assert!(!names.contains(&"closed"));
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.name == "shadow")
                .and_then(|binding| binding.ty.as_deref()),
            Some("Inner")
        );

        let outside = format!("{inside}\n    }}\n    sha");
        let outside_bindings = visible_bindings(&outside, Position::new(9, 7));
        assert_eq!(
            outside_bindings
                .iter()
                .find(|binding| binding.name == "shadow")
                .and_then(|binding| binding.ty.as_deref()),
            Some("Outer")
        );

        let patterns = concat!(
            "def current(current_param: i64) -> i64 {\n",
            "    // def fake(comment_param: i64) {\n",
            "    let text = \"def fake(string_param: i64) {\";\n",
            "    for loop_item in values {\n",
            "        match value { Event::Pair(number, enabled) => {\n",
            "            if let selected = value { sel",
        );
        let pattern_bindings = visible_bindings(
            patterns,
            Position::new(
                5,
                patterns.lines().nth(5).unwrap().encode_utf16().count() as u32,
            ),
        );
        let pattern_names = pattern_bindings
            .iter()
            .map(|binding| binding.name.as_str())
            .collect::<Vec<_>>();
        for expected in [
            "current_param",
            "loop_item",
            "number",
            "enabled",
            "selected",
        ] {
            assert!(
                pattern_names.contains(&expected),
                "missing {expected}: {pattern_names:?}"
            );
        }
        assert!(!pattern_names.contains(&"comment_param"));
        assert!(!pattern_names.contains(&"string_param"));

        let closed_impl = concat!(
            "impl Old { def old(self) -> i64 { 0 } }\n",
            "def main() -> i64 { self. }\n",
        );
        assert_eq!(
            enclosing_impl_type(
                closed_impl,
                Position::new(
                    1,
                    closed_impl.lines().nth(1).unwrap().encode_utf16().count() as u32,
                ),
            ),
            None
        );
    }

    #[test]
    fn member_completion_uses_qualified_type_identity_and_rejects_ambiguous_bare_types() {
        let root = temp_workspace();
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = \"app\"\n[dependencies]\ndep_alias = { path = \"../dep\" }\n",
        )
        .unwrap();
        fs::write(dep.join("Sengoo.toml"), "[package]\nname = \"dep\"\n").unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            "version = 1\nroot = \"app\"\n[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\nsource = \"path+../dep\"\nmanifest = \"../dep/Sengoo.toml\"\n",
        )
        .unwrap();
        fs::write(
            app.join("src/point.sg"),
            "struct Point { app_only: i64, }\n",
        )
        .unwrap();
        fs::write(
            dep.join("src/point.sg"),
            concat!(
                "struct Child { dep_child: i64, }\n",
                "struct Point { dep_only: i64, child: Child, }\n",
                "impl Point { def make_child(self) -> Child { Child { dep_child: 1 } } }\n",
                "def make_dep_point() -> Point { Point { dep_only: 1, child: Child { dep_child: 1 } } }\n",
            ),
        )
        .unwrap();
        let main = app.join("src/main.sg");
        fs::write(&main, "def main(value: i64) -> i64 { value }\n").unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index = WorkspaceIndex::build(&[app], IndexCancellation::default()).unwrap();

        let bare_source = "def main(point: Point) -> i64 { point. }\n";
        assert!(index.open(
            uri.clone(),
            1,
            bare_source.into(),
            &IndexCancellation::default(),
        ));
        assert!(completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (bare_source.find("point.").unwrap() + 6) as u32),
        )
        .is_empty());

        let qualified_source = "def main(point: dep_alias::point::Point) -> i64 { point. }\n";
        assert!(index.open(
            uri.clone(),
            2,
            qualified_source.into(),
            &IndexCancellation::default(),
        ));
        let qualified = completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (qualified_source.find("point.").unwrap() + 6) as u32),
        );
        assert!(qualified.iter().any(|item| item.label == "dep_only"));
        assert!(!qualified.iter().any(|item| item.label == "app_only"));

        let inferred_source =
            "def main() -> i64 { let point = dep_alias::point::Point { dep_only: 1 }; point. }\n";
        assert!(index.open(
            uri.clone(),
            3,
            inferred_source.into(),
            &IndexCancellation::default(),
        ));
        let inferred = completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (inferred_source.rfind("point.").unwrap() + 6) as u32),
        );
        assert!(inferred.iter().any(|item| item.label == "dep_only"));
        assert!(!inferred.iter().any(|item| item.label == "app_only"));

        let field_chain = "def main(point: dep_alias::point::Point) -> i64 { point.child.dep }\n";
        assert!(index.open(
            uri.clone(),
            4,
            field_chain.into(),
            &IndexCancellation::default(),
        ));
        let fields = completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (field_chain.find(".dep").unwrap() + 4) as u32),
        );
        assert!(
            fields.iter().any(|item| item.label == "dep_child"),
            "fields={:?}; candidates={:?}",
            fields.iter().map(|item| &item.label).collect::<Vec<_>>(),
            index
                .completion_candidates(&uri)
                .into_iter()
                .map(|candidate| (
                    candidate.symbol.name,
                    candidate.semantic_detail,
                    candidate.symbol.container
                ))
                .collect::<Vec<_>>()
        );

        let call_chain =
            "def main(point: dep_alias::point::Point) -> i64 { point.make_child().dep }\n";
        assert!(index.open(
            uri.clone(),
            5,
            call_chain.into(),
            &IndexCancellation::default(),
        ));
        let calls = completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (call_chain.find(".dep").unwrap() + 4) as u32),
        );
        assert!(calls.iter().any(|item| item.label == "dep_child"));

        let return_inference = "def main() -> i64 { let point = make_dep_point(); point.dep }\n";
        assert!(index.open(
            uri.clone(),
            6,
            return_inference.into(),
            &IndexCancellation::default(),
        ));
        let returned = completion_items_for_request(
            &index,
            &uri,
            Position::new(0, (return_inference.rfind(".dep").unwrap() + 4) as u32),
        );
        assert!(returned.iter().any(|item| item.label == "dep_only"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn representative_warm_completion_p95_stays_below_eighty_ms() {
        let root = temp_workspace();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"bench\"\n").unwrap();
        for index in 0..40 {
            fs::write(
                root.join("src").join(format!("module_{index}.sg")),
                format!("def project_symbol_{index}(value: i64) -> i64 {{ value }}\n"),
            )
            .unwrap();
        }
        let main = root.join("src/main.sg");
        let source = "def main(parameter: i64) -> i64 { let local_value: i64 = parameter; loc }\n";
        fs::write(&main, source).unwrap();
        let uri = Url::from_file_path(&main).unwrap();
        let index =
            WorkspaceIndex::build(std::slice::from_ref(&root), IndexCancellation::default())
                .unwrap();
        assert!(index.open(uri.clone(), 1, source.into(), &IndexCancellation::default(),));
        let position = Position::new(0, source.find("loc }").unwrap() as u32 + 3);
        for _ in 0..10 {
            let _ = completion_items_for_request(&index, &uri, position);
        }
        let mut samples = (0..100)
            .map(|_| {
                let started = Instant::now();
                let _ = completion_items_for_request(&index, &uri, position);
                started.elapsed()
            })
            .collect::<Vec<_>>();
        samples.sort_unstable();
        let p95 = samples[94];
        assert!(p95.as_millis() < 80, "warm completion p95 was {p95:?}");
        let _ = fs::remove_dir_all(root);
    }
}
