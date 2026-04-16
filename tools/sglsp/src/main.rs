//! sglsp - Sengoo language server.

use miette::Result;
use sengoo_compiler::ast::{Decl, DeclKind, Function, SelfParam, Type};
use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::{compile_to_ir, CompileError, Parser as SgParser};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SengooLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}

#[derive(Debug, Clone)]
struct ServerConfig {
    max_problems: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { max_problems: 128 }
    }
}

#[derive(Debug, Clone, Copy)]
enum SemanticKind {
    Keyword = 0,
    Function = 1,
    Struct = 2,
    Type = 3,
    Variable = 4,
    Number = 5,
    String = 6,
    Comment = 7,
}

#[derive(Debug, Clone, Copy)]
struct RawSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    kind: SemanticKind,
}

#[derive(Debug, Clone)]
struct SymbolAt {
    name: String,
    range: Range,
}

#[derive(Debug, Clone)]
struct AstSymbol {
    name: String,
    detail: String,
    kind: CompletionItemKind,
    range: Range,
}

#[derive(Debug, Clone)]
struct FunctionSignatureInfo {
    name: String,
    label: String,
    params: Vec<String>,
    range: Range,
}

#[derive(Debug, Deserialize)]
struct SgcErrorSpanPayload {
    lo: u32,
    hi: u32,
}

#[derive(Debug, Deserialize)]
struct SgcErrorLocationPayload {
    #[serde(default)]
    line: Option<u32>,
    #[serde(default, alias = "col")]
    column: Option<u32>,
    #[serde(default)]
    span: Option<SgcErrorSpanPayload>,
}

#[derive(Debug, Deserialize)]
struct SgcErrorPayload {
    #[allow(dead_code)]
    ok: Option<bool>,
    #[allow(dead_code)]
    kind: Option<String>,
    stage: Option<String>,
    message: Option<String>,
    #[serde(default)]
    details: Vec<String>,
    #[serde(default)]
    location: Option<SgcErrorLocationPayload>,
}

fn semantic_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::STRUCT,
            SemanticTokenType::TYPE,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::NUMBER,
            SemanticTokenType::STRING,
            SemanticTokenType::COMMENT,
        ],
        token_modifiers: vec![],
    }
}

fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn char_to_byte_index(s: &str, character: u32) -> usize {
    let target = character as usize;
    if target == 0 {
        return 0;
    }

    for (seen, (idx, _)) in s.char_indices().enumerate() {
        if seen == target {
            return idx;
        }
    }
    s.len()
}

fn byte_to_char_index(s: &str, byte_idx: usize) -> u32 {
    s[..byte_idx].chars().count() as u32
}

fn line_char_len(line: &str) -> u32 {
    line.chars().count() as u32
}

fn clamp_to_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn position_to_byte_index(content: &str, position: Position) -> Option<usize> {
    let mut line_start = 0usize;
    let mut current_line = 0u32;

    while current_line < position.line {
        let next_rel = content[line_start..].find('\n')?;
        line_start += next_rel + 1;
        current_line += 1;
    }

    let line_end = content[line_start..]
        .find('\n')
        .map(|idx| line_start + idx)
        .unwrap_or(content.len());
    let line = &content[line_start..line_end];

    let mut utf16_units = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_units >= position.character {
            return Some(line_start + byte_idx);
        }
        utf16_units += ch.len_utf16() as u32;
        if utf16_units == position.character {
            return Some(line_start + byte_idx + ch.len_utf8());
        }
    }

    if position.character <= utf16_units {
        Some(line_end)
    } else {
        None
    }
}

fn byte_index_to_position(content: &str, byte_idx: usize) -> Position {
    let byte_idx = clamp_to_char_boundary(content, byte_idx);
    let mut line = 0u32;
    let mut line_start = 0usize;

    for (idx, ch) in content.char_indices() {
        if idx >= byte_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + 1;
        }
    }

    let line_prefix = &content[line_start..byte_idx];
    Position {
        line,
        character: line_prefix.encode_utf16().count() as u32,
    }
}

fn span_to_range(content: &str, lo: u32, hi: u32) -> Range {
    Range {
        start: byte_index_to_position(content, lo as usize),
        end: byte_index_to_position(content, hi as usize),
    }
}

fn apply_content_changes(content: &mut String, changes: Vec<TextDocumentContentChangeEvent>) {
    for change in changes {
        if let Some(range) = change.range {
            let Some(start) = position_to_byte_index(content, range.start) else {
                *content = change.text;
                continue;
            };
            let Some(end) = position_to_byte_index(content, range.end) else {
                *content = change.text;
                continue;
            };

            if start <= end && end <= content.len() {
                let start = clamp_to_char_boundary(content, start);
                let end = clamp_to_char_boundary(content, end);
                if start <= end {
                    content.replace_range(start..end, &change.text);
                    continue;
                }
            }
        }

        *content = change.text;
    }
}

fn declaration_kind(kind: &DeclKind) -> (&'static str, CompletionItemKind) {
    match kind {
        DeclKind::Function(_) => ("function", CompletionItemKind::FUNCTION),
        DeclKind::Struct(_) => ("struct", CompletionItemKind::STRUCT),
        DeclKind::Enum(_) => ("enum", CompletionItemKind::ENUM),
        DeclKind::Class(_) => ("class", CompletionItemKind::CLASS),
        DeclKind::Trait(_) => ("trait", CompletionItemKind::INTERFACE),
        DeclKind::TypeAlias(_) => ("type alias", CompletionItemKind::TYPE_PARAMETER),
        DeclKind::Const(_) => ("const", CompletionItemKind::CONSTANT),
        DeclKind::Static(_) => ("static", CompletionItemKind::VARIABLE),
        DeclKind::Module(_) => ("module", CompletionItemKind::MODULE),
        _ => ("declaration", CompletionItemKind::VARIABLE),
    }
}

fn collect_decl_symbols(content: &str, decl: &Decl, out: &mut Vec<AstSymbol>) {
    if let Some(name) = decl.name() {
        let (detail, kind) = declaration_kind(&decl.kind);
        out.push(AstSymbol {
            name: name.name.clone(),
            detail: detail.to_string(),
            kind,
            range: span_to_range(content, name.span.lo, name.span.hi),
        });
    }

    if let DeclKind::Module(module_decl) = &decl.kind {
        for nested in &module_decl.items {
            collect_decl_symbols(content, nested, out);
        }
    }
}

fn collect_ast_symbols(content: &str) -> Vec<AstSymbol> {
    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };

    let mut symbols = Vec::new();
    for decl in &program.decls {
        collect_decl_symbols(content, decl, &mut symbols);
    }
    symbols
}

fn span_text(content: &str, lo: u32, hi: u32) -> String {
    let len = content.len();
    let mut start = (lo as usize).min(len);
    let mut end = (hi as usize).min(len);

    start = clamp_to_char_boundary(content, start);
    end = clamp_to_char_boundary(content, end);
    if end < start {
        std::mem::swap(&mut start, &mut end);
    }

    content[start..end].trim().to_string()
}

fn type_snippet(content: &str, ty: &Type) -> String {
    let text = span_text(content, ty.span.lo, ty.span.hi);
    if text.is_empty() {
        "_".to_string()
    } else {
        text
    }
}

fn self_param_snippet(self_param: SelfParam) -> &'static str {
    match self_param {
        SelfParam::Borrowed => "self",
        SelfParam::BorrowedMut => "mut self",
        SelfParam::Owned => "self",
        SelfParam::OwnedMut => "mut self",
    }
}

fn function_signature_label(content: &str, function: &Function) -> (String, Vec<String>) {
    let mut params = Vec::new();
    if let Some(self_param) = function.self_param {
        params.push(self_param_snippet(self_param).to_string());
    }

    for param in &function.params {
        params.push(format!(
            "{}: {}",
            param.name.name,
            type_snippet(content, &param.ty)
        ));
    }

    let generic_suffix = if function.type_params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            function
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let ret = function
        .return_type
        .as_ref()
        .map(|ty| type_snippet(content, ty))
        .unwrap_or_else(|| "unit".to_string());

    let async_prefix = if function.is_async { "async " } else { "" };
    (
        format!(
            "{}def {}{}({}) -> {}",
            async_prefix,
            function.name.name,
            generic_suffix,
            params.join(", "),
            ret
        ),
        params,
    )
}

fn collect_function_signatures_from_decl(
    content: &str,
    decl: &Decl,
    out: &mut Vec<FunctionSignatureInfo>,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            let (label, params) = function_signature_label(content, function);
            out.push(FunctionSignatureInfo {
                name: function.name.name.clone(),
                label,
                params,
                range: span_to_range(content, function.name.span.lo, function.name.span.hi),
            });
        }
        DeclKind::Module(module_decl) => {
            for nested in &module_decl.items {
                collect_function_signatures_from_decl(content, nested, out);
            }
        }
        DeclKind::Impl(impl_decl) => {
            let target = span_text(content, impl_decl.target_type.span.lo, impl_decl.target_type.span.hi);
            let target = if target.is_empty() { "_".to_string() } else { target };
            for method in &impl_decl.items {
                let (base_label, params) = function_signature_label(content, method);
                out.push(FunctionSignatureInfo {
                    name: method.name.name.clone(),
                    label: format!("{} [impl {}]", base_label, target),
                    params,
                    range: span_to_range(content, method.name.span.lo, method.name.span.hi),
                });
            }
        }
        _ => {}
    }
}

fn collect_function_signatures(content: &str) -> Vec<FunctionSignatureInfo> {
    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };

    let mut signatures = Vec::new();
    for decl in &program.decls {
        collect_function_signatures_from_decl(content, decl, &mut signatures);
    }
    signatures
}

fn active_call_site(content: &str, offset: usize) -> Option<(String, u32)> {
    let bytes = content.as_bytes();
    let limit = offset.min(bytes.len());
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0usize;

    while i < limit {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < limit {
                    if bytes[i] == b'\\' && i + 1 < limit {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < limit && bytes[i + 1] == b'/' => {
                while i < limit && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'(' => stack.push(i),
            b')' => {
                stack.pop();
            }
            _ => {}
        }
        i += 1;
    }

    let open = *stack.last()?;

    let mut end = open;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    let mut start = end;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    if start == end {
        return None;
    }

    let name = content[start..end].to_string();
    let mut nested_depth = 0u32;
    let mut active_param = 0u32;
    let mut j = open + 1;

    while j < limit {
        match bytes[j] {
            b'"' => {
                j += 1;
                while j < limit {
                    if bytes[j] == b'\\' && j + 1 < limit {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == b'"' {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                continue;
            }
            b'/' if j + 1 < limit && bytes[j + 1] == b'/' => {
                while j < limit && bytes[j] != b'\n' {
                    j += 1;
                }
                continue;
            }
            b'(' => nested_depth += 1,
            b')' => {
                if nested_depth == 0 {
                    break;
                }
                nested_depth -= 1;
            }
            b',' if nested_depth == 0 => active_param += 1,
            _ => {}
        }

        j += 1;
    }

    Some((name, active_param))
}

fn completion_kind_to_symbol_kind(kind: CompletionItemKind) -> SymbolKind {
    match kind {
        CompletionItemKind::FUNCTION => SymbolKind::FUNCTION,
        CompletionItemKind::STRUCT => SymbolKind::STRUCT,
        CompletionItemKind::ENUM => SymbolKind::ENUM,
        CompletionItemKind::CLASS => SymbolKind::CLASS,
        CompletionItemKind::INTERFACE => SymbolKind::INTERFACE,
        CompletionItemKind::TYPE_PARAMETER => SymbolKind::TYPE_PARAMETER,
        CompletionItemKind::CONSTANT => SymbolKind::CONSTANT,
        CompletionItemKind::MODULE => SymbolKind::MODULE,
        _ => SymbolKind::VARIABLE,
    }
}

fn folding_ranges_for(content: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let mut block_stack: Vec<u32> = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx as u32;
        for ch in line.chars() {
            match ch {
                '{' => block_stack.push(line_num),
                '}' => {
                    if let Some(start_line) = block_stack.pop() {
                        if line_num > start_line {
                            ranges.push(FoldingRange {
                                start_line,
                                start_character: None,
                                end_line: line_num,
                                end_character: None,
                                kind: Some(FoldingRangeKind::Region),
                                collapsed_text: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut comment_start: Option<u32> = None;
    let mut last_comment_line = 0u32;
    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx as u32;
        if line.trim_start().starts_with("//") {
            if comment_start.is_none() {
                comment_start = Some(line_num);
            }
            last_comment_line = line_num;
            continue;
        }

        if let Some(start_line) = comment_start.take() {
            if last_comment_line > start_line {
                ranges.push(FoldingRange {
                    start_line,
                    start_character: None,
                    end_line: last_comment_line,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Comment),
                    collapsed_text: None,
                });
            }
        }
    }

    if let Some(start_line) = comment_start {
        if last_comment_line > start_line {
            ranges.push(FoldingRange {
                start_line,
                start_character: None,
                end_line: last_comment_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Comment),
                collapsed_text: None,
            });
        }
    }

    ranges
}

fn extract_identifier_at(content: &str, position: Position) -> Option<SymbolAt> {
    let line = content.lines().nth(position.line as usize)?;
    if line.is_empty() {
        return None;
    }

    let mut cursor = char_to_byte_index(line, position.character);
    if cursor >= line.len() {
        cursor = line.len().saturating_sub(1);
    }

    let bytes = line.as_bytes();
    if !is_identifier_byte(bytes[cursor]) {
        if cursor == 0 || !is_identifier_byte(bytes[cursor - 1]) {
            return None;
        }
        cursor -= 1;
    }

    let mut start = cursor;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = cursor;
    while end < bytes.len() && is_identifier_byte(bytes[end]) {
        end += 1;
    }

    let start_char = byte_to_char_index(line, start);
    let end_char = byte_to_char_index(line, end);

    Some(SymbolAt {
        name: line[start..end].to_string(),
        range: Range {
            start: Position {
                line: position.line,
                character: start_char,
            },
            end: Position {
                line: position.line,
                character: end_char,
            },
        },
    })
}

fn find_symbol_occurrences(content: &str, symbol: &str) -> Vec<Range> {
    if symbol.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let symbol_bytes = symbol.as_bytes();

    for (line_idx, line) in content.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0usize;

        while i + symbol_bytes.len() <= bytes.len() {
            let matched = &bytes[i..i + symbol_bytes.len()] == symbol_bytes;
            let left_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
            let right_bound = i + symbol_bytes.len();
            let right_ok = right_bound == bytes.len() || !is_identifier_byte(bytes[right_bound]);

            if matched && left_ok && right_ok {
                ranges.push(Range {
                    start: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, i),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, right_bound),
                    },
                });
                i += symbol_bytes.len();
            } else {
                i += 1;
            }
        }
    }

    ranges
}

fn find_definition_in_text(content: &str, symbol: &str) -> Option<Range> {
    let declaration_keywords = ["fn", "struct", "let", "const", "type", "enum"];

    for keyword in declaration_keywords {
        let pattern = format!("{keyword} {symbol}");
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(pos) = line.find(&pattern) {
                let symbol_start = pos + keyword.len() + 1;
                let symbol_end = symbol_start + symbol.len();
                let bytes = line.as_bytes();

                let left_ok = symbol_start == 0 || !is_identifier_byte(bytes[symbol_start - 1]);
                let right_ok = symbol_end == bytes.len() || !is_identifier_byte(bytes[symbol_end]);
                if left_ok && right_ok {
                    return Some(Range {
                        start: Position {
                            line: line_idx as u32,
                            character: byte_to_char_index(line, symbol_start),
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: byte_to_char_index(line, symbol_end),
                        },
                    });
                }
            }
        }
    }

    find_symbol_occurrences(content, symbol).into_iter().next()
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "struct"
            | "enum"
            | "type"
            | "let"
            | "const"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "match"
            | "return"
            | "break"
            | "continue"
            | "import"
            | "use"
            | "pub"
            | "true"
            | "false"
    )
}

fn is_builtin_type(word: &str) -> bool {
    matches!(
        word,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "bool"
            | "str"
            | "char"
            | "unit"
    )
}

fn semantic_tokens_for(content: &str) -> Vec<SemanticToken> {
    let mut raw = Vec::<RawSemanticToken>::new();

    for (line_idx, line) in content.lines().enumerate() {
        let bytes = line.as_bytes();
        let comment_start = line.find("//");
        let scan_end = comment_start.unwrap_or(line.len());
        let mut i = 0usize;
        let mut pending_decl: Option<&str> = None;

        while i < scan_end {
            let b = bytes[i];
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }

            if b == b'"' {
                let start = i;
                i += 1;
                while i < scan_end {
                    if bytes[i] == b'\\' && i + 1 < scan_end {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind: SemanticKind::String,
                });
                continue;
            }

            if b.is_ascii_digit() {
                let start = i;
                while i < scan_end && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind: SemanticKind::Number,
                });
                pending_decl = None;
                continue;
            }

            if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < scan_end && is_identifier_byte(bytes[i]) {
                    i += 1;
                }
                let word = &line[start..i];
                let kind = if is_keyword(word) {
                    pending_decl = Some(word);
                    SemanticKind::Keyword
                } else if matches!(pending_decl, Some("fn")) {
                    pending_decl = None;
                    SemanticKind::Function
                } else if matches!(pending_decl, Some("struct" | "enum")) {
                    pending_decl = None;
                    SemanticKind::Struct
                } else if matches!(pending_decl, Some("let" | "const")) {
                    pending_decl = None;
                    SemanticKind::Variable
                } else if is_builtin_type(word)
                    || word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                {
                    pending_decl = None;
                    SemanticKind::Type
                } else {
                    pending_decl = None;
                    SemanticKind::Variable
                };

                raw.push(RawSemanticToken {
                    line: line_idx as u32,
                    start: byte_to_char_index(line, start),
                    length: byte_to_char_index(line, i) - byte_to_char_index(line, start),
                    kind,
                });
                continue;
            }

            pending_decl = None;
            i += 1;
        }

        if let Some(start) = comment_start {
            raw.push(RawSemanticToken {
                line: line_idx as u32,
                start: byte_to_char_index(line, start),
                length: line_char_len(line) - byte_to_char_index(line, start),
                kind: SemanticKind::Comment,
            });
        }
    }

    raw.sort_by_key(|t| (t.line, t.start));

    let mut result = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for token in raw {
        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.start - prev_start
        } else {
            token.start
        };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.kind as u32,
            token_modifiers_bitset: 0,
        });

        prev_line = token.line;
        prev_start = token.start;
    }

    result
}

fn build_diagnostics(content: &str, max_problems: usize) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        if diagnostics.len() >= max_problems {
            break;
        }

        if line.contains("TODO") || line.contains("FIXME") {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::HINT),
                code: Some(NumberOrString::String("todo-item".to_string())),
                source: Some("sglsp".to_string()),
                message: "TODO/FIXME marker found".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        if let Some(tab_pos) = line.find('\t') {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos + 1),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("tab-indentation".to_string())),
                source: Some("sglsp".to_string()),
                message: "Tab indentation detected (prefer spaces)".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.len() < line.len() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: line_char_len(trimmed),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("trailing-whitespace".to_string())),
                source: Some("sglsp".to_string()),
                message: "Trailing whitespace".to_string(),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn parse_sgc_payload(raw: &str) -> Option<SgcErrorPayload> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(payload) = serde_json::from_str::<SgcErrorPayload>(trimmed) {
        return Some(payload);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if start >= end {
        return None;
    }
    serde_json::from_str::<SgcErrorPayload>(&trimmed[start..=end]).ok()
}

fn extract_u32_numbers(text: &str) -> Vec<u32> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }

        if !current.is_empty() {
            if let Ok(value) = current.parse::<u32>() {
                numbers.push(value);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(value) = current.parse::<u32>() {
            numbers.push(value);
        }
    }

    numbers
}

fn parse_line_col_hint(text: &str) -> Option<(u32, u32)> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("line") {
        return None;
    }
    if !(lower.contains("col") || lower.contains("column")) {
        return None;
    }

    let numbers = extract_u32_numbers(&lower);
    if numbers.len() < 2 {
        return None;
    }

    Some((numbers[0], numbers[1]))
}

fn line_col_to_position(content: &str, line_1_based: u32, col_1_based: u32) -> Position {
    let lines = content.split('\n').collect::<Vec<_>>();
    if lines.is_empty() {
        return Position {
            line: 0,
            character: 0,
        };
    }

    let max_line = lines.len().saturating_sub(1) as u32;
    let line = line_1_based.saturating_sub(1).min(max_line);
    let line_text = lines[line as usize];
    let max_col = line_text.encode_utf16().count() as u32;
    let character = col_1_based.saturating_sub(1).min(max_col);

    Position { line, character }
}

fn range_from_line_col(content: &str, line_1_based: u32, col_1_based: u32) -> Range {
    let start = line_col_to_position(content, line_1_based, col_1_based);
    let line_text = content
        .split('\n')
        .nth(start.line as usize)
        .unwrap_or_default();
    let max_col = line_text.encode_utf16().count() as u32;
    let end_character = (start.character + 1).min(max_col);

    Range {
        start,
        end: Position {
            line: start.line,
            character: end_character,
        },
    }
}

fn range_from_byte_span(content: &str, lo: u32, hi: u32) -> Range {
    let max = u32::try_from(content.len()).unwrap_or(u32::MAX);
    let lo = lo.min(max);
    let mut hi = hi.min(max);
    if hi <= lo && lo < max {
        hi = lo + 1;
    }
    span_to_range(content, lo, hi)
}

fn diagnostic_range_from_location(content: &str, location: &SgcErrorLocationPayload) -> Option<Range> {
    if let Some(span) = &location.span {
        return Some(range_from_byte_span(content, span.lo, span.hi));
    }

    if let (Some(line), Some(column)) = (location.line, location.column) {
        return Some(range_from_line_col(content, line, column));
    }

    None
}

fn diagnostic_range_from_payload(content: &str, payload: &SgcErrorPayload) -> Option<Range> {
    if let Some(location) = payload.location.as_ref() {
        if let Some(range) = diagnostic_range_from_location(content, location) {
            return Some(range);
        }
    }

    for hint in payload
        .details
        .iter()
        .chain(payload.message.as_ref().into_iter())
    {
        if let Some((line, col)) = parse_line_col_hint(hint) {
            return Some(range_from_line_col(content, line, col));
        }
    }
    None
}

fn source_span_to_range(content: &str, span: &miette::SourceSpan) -> Option<Range> {
    let lo: usize = span.offset();
    if lo > content.len() {
        return None;
    }

    let mut hi = lo.saturating_add(span.len()).min(content.len());
    if hi == lo && lo < content.len() {
        hi = lo + 1;
    }

    let lo = u32::try_from(lo).ok()?;
    let hi = u32::try_from(hi).ok()?;
    Some(span_to_range(content, lo, hi))
}

fn diagnostic_range_from_parse_error(content: &str, error: &ParseError) -> Option<Range> {
    match error {
        ParseError::UnexpectedToken { span, .. }
        | ParseError::UnclosedBlock(span)
        | ParseError::UnclosedParen(span)
        | ParseError::InvalidStructField { span, .. }
        | ParseError::InvalidStructFieldShorthand { span }
        | ParseError::InvalidPatternAt { span, .. } => source_span_to_range(content, span),
        ParseError::InvalidPattern(_) | ParseError::DuplicateParam(_) | ParseError::UnexpectedEof => None,
    }
}

fn diagnostic_range_from_type_error(content: &str, error: &TypeError) -> Option<Range> {
    match error {
        TypeError::Mismatch { span, .. } => source_span_to_range(content, span),
        TypeError::UndefinedVar { _span, .. } => source_span_to_range(content, _span),
        TypeError::UndefinedType(_)
        | TypeError::UndefinedMethod(_)
        | TypeError::ArgCountMismatch { .. }
        | TypeError::TraitNotImplemented { .. } => None,
    }
}

fn diagnostic_range_from_compile_error(content: &str, error: &CompileError) -> Option<Range> {
    match error {
        CompileError::ParseError(error) => diagnostic_range_from_parse_error(content, error),
        CompileError::TypeError(error) => diagnostic_range_from_type_error(content, error),
        _ => None,
    }
}

fn fallback_diagnostic_range_from_compiler(content: &str) -> Option<Range> {
    compile_to_ir(content)
        .err()
        .and_then(|error| diagnostic_range_from_compile_error(content, &error))
}
fn temporary_source_path(uri: &Url) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    if let Ok(file_path) = uri.to_file_path() {
        if let Some(parent) = file_path.parent() {
            return parent.join(format!(".sglsp_tmp_{}.sg", now));
        }
    }

    std::env::temp_dir().join(format!("sglsp_tmp_{}.sg", now))
}

fn compiler_diagnostics_from_sgc_json(uri: &Url, content: &str) -> Vec<Diagnostic> {
    let scratch = temporary_source_path(uri);
    if fs::write(&scratch, content).is_err() {
        return Vec::new();
    }

    let output = Command::new("sgc")
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&scratch)
        .output();

    let _ = fs::remove_file(&scratch);

    let Ok(output) = output else {
        return Vec::new();
    };
    if output.status.success() {
        return Vec::new();
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let payload = parse_sgc_payload(&stderr);
    let fallback_range = fallback_diagnostic_range_from_compiler(content);

    let (message, code, range) = if let Some(payload) = payload {
        let range = diagnostic_range_from_payload(content, &payload)
            .or(fallback_range)
            .unwrap_or_else(|| full_document_range(content));
        let mut message = payload
            .message
            .unwrap_or_else(|| "compilation failed".to_string());
        if !payload.details.is_empty() {
            message.push('\n');
            message.push_str(&payload.details.join("\n"));
        }
        (message, payload.stage, range)
    } else {
        let summary = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "compilation failed".to_string());
        let range = fallback_range.unwrap_or_else(|| full_document_range(content));
        (summary, None, range)
    };

    vec![Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: code.map(NumberOrString::String),
        source: Some("sgc".to_string()),
        message,
        ..Default::default()
    }]
}

fn full_document_range(content: &str) -> Range {
    let mut last_line_idx = 0u32;
    let mut last_line = "";

    for (idx, line) in content.lines().enumerate() {
        last_line_idx = idx as u32;
        last_line = line;
    }

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: last_line_idx,
            character: line_char_len(last_line),
        },
    }
}

fn normalized_format(content: &str) -> String {
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches([' ', '\t']));
    }
    out
}

fn valid_identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn quick_fix_action(
    uri: Url,
    edit: TextEdit,
    diagnostic: Diagnostic,
    title: &str,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri, vec![edit])])),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

#[derive(Debug)]
struct SengooLanguageServer {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
    config: ServerConfig,
}

impl SengooLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            config: ServerConfig::default(),
        }
    }

    async fn document_text(&self, uri: &Url) -> Option<String> {
        self.documents.read().await.get(uri).cloned()
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let content = self.document_text(uri).await.unwrap_or_default();
        let mut diagnostics = compiler_diagnostics_from_sgc_json(uri, &content);
        let mut style = build_diagnostics(&content, self.config.max_problems);
        diagnostics.append(&mut style);
        diagnostics.truncate(self.config.max_problems);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }

    async fn all_documents(&self) -> HashMap<Url, String> {
        self.documents.read().await.clone()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SengooLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: semantic_legend(),
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Sengoo LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        self.documents.write().await.insert(uri.clone(), content);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let content_changes = params.content_changes;
        let mut documents = self.documents.write().await;
        if let Some(current) = documents.get_mut(&uri) {
            apply_content_changes(current, content_changes);
        } else if let Some(last) = content_changes.last() {
            documents.insert(uri.clone(), last.text.clone());
        }
        drop(documents);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let content = self.document_text(&uri).await.unwrap_or_default();
        let ast_symbols = collect_ast_symbols(&content);

        let mut items = vec![
            CompletionItem::new_simple("fn".to_string(), "Define a function".to_string()),
            CompletionItem::new_simple("struct".to_string(), "Define a struct".to_string()),
            CompletionItem::new_simple("let".to_string(), "Declare a local variable".to_string()),
            CompletionItem::new_simple("const".to_string(), "Declare a constant".to_string()),
            CompletionItem::new_simple("match".to_string(), "Pattern matching".to_string()),
        ];

        let mut seen = std::collections::HashSet::new();
        for symbol in ast_symbols {
            if seen.insert(symbol.name.clone()) {
                items.push(CompletionItem {
                    label: symbol.name,
                    kind: Some(symbol.kind),
                    detail: Some(symbol.detail),
                    ..Default::default()
                });
            }
        }

        for line in content.lines() {
            for token in line.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                if token.len() < 2 || !valid_identifier_name(token) {
                    continue;
                }
                if seen.insert(token.to_string()) {
                    items.push(CompletionItem {
                        label: token.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };
        let local_symbols = collect_ast_symbols(current_content);
        if let Some(found) = local_symbols.iter().find(|item| item.name == symbol.name) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                uri,
                found.range,
            ))));
        }

        if let Some(range) = find_definition_in_text(current_content, &symbol.name) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                uri, range,
            ))));
        }

        for (doc_uri, doc_content) in documents {
            let symbols = collect_ast_symbols(&doc_content);
            if let Some(found) = symbols.iter().find(|item| item.name == symbol.name) {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                    doc_uri, found.range,
                ))));
            }
            if let Some(range) = find_definition_in_text(&doc_content, &symbol.name) {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                    doc_uri, range,
                ))));
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };

        let mut locations = Vec::new();
        for (doc_uri, doc_content) in documents {
            for range in find_symbol_occurrences(&doc_content, &symbol.name) {
                locations.push(Location::new(doc_uri.clone(), range));
            }
        }

        if !params.context.include_declaration {
            locations.retain(|loc| loc.range != symbol.range || loc.uri != uri);
        }

        Ok(Some(locations))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(&content, position) else {
            return Ok(None);
        };
        let ast_symbols = collect_ast_symbols(&content);
        if let Some(item) = ast_symbols.iter().find(|item| item.name == symbol.name) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("`{}` ({})", item.name, item.detail),
                }),
                range: Some(item.range),
            }));
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("`{}`", symbol.name),
            }),
            range: Some(symbol.range),
        }))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> LspResult<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let Some(offset) = position_to_byte_index(&content, position) else {
            return Ok(None);
        };
        let Some((call_name, active_param)) = active_call_site(&content, offset) else {
            return Ok(None);
        };

        let mut signatures = collect_function_signatures(&content)
            .into_iter()
            .filter(|sig| sig.name == call_name)
            .collect::<Vec<_>>();

        if call_name == "print" && signatures.is_empty() {
            signatures.push(FunctionSignatureInfo {
                name: "print".to_string(),
                label: "def print(value: Any) -> unit".to_string(),
                params: vec!["value: Any".to_string()],
                range: full_document_range(&content),
            });
        }

        if signatures.is_empty() {
            return Ok(None);
        }

        signatures.sort_by_key(|sig| (sig.range.start.line, sig.range.start.character));

        let signature_items = signatures
            .iter()
            .map(|sig| SignatureInformation {
                label: sig.label.clone(),
                documentation: None,
                parameters: Some(
                    sig.params
                        .iter()
                        .map(|param| ParameterInformation {
                            label: ParameterLabel::Simple(param.clone()),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect::<Vec<_>>();

        let first_param_count = signatures.first().map(|sig| sig.params.len()).unwrap_or(0);
        let clamped_active_param = if first_param_count == 0 {
            0
        } else {
            active_param.min((first_param_count.saturating_sub(1)) as u32)
        };

        Ok(Some(SignatureHelp {
            signatures: signature_items,
            active_signature: Some(0),
            active_parameter: Some(clamped_active_param),
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> LspResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let symbols = collect_ast_symbols(&content);
        if symbols.is_empty() {
            return Ok(None);
        }

        #[allow(deprecated)]
        let response = symbols
            .into_iter()
            .map(|symbol| SymbolInformation {
                name: symbol.name,
                kind: completion_kind_to_symbol_kind(symbol.kind),
                tags: None,
                deprecated: None,
                location: Location::new(uri.clone(), symbol.range),
                container_name: Some(symbol.detail),
            })
            .collect::<Vec<_>>();

        Ok(Some(DocumentSymbolResponse::Flat(response)))
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> LspResult<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        Ok(Some(folding_ranges_for(&content)))
    }
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let Some(content) = self.document_text(&params.text_document.uri).await else {
            return Ok(None);
        };

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens_for(&content),
        })))
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let mut actions = Vec::new();
        for diagnostic in params.context.diagnostics {
            let Some(code) = diagnostic.code.as_ref() else {
                continue;
            };

            let Some(code) = (match code {
                NumberOrString::String(s) => Some(s.as_str()),
                NumberOrString::Number(_) => None,
            }) else {
                continue;
            };

            match code {
                "todo-item" => {
                    let line_idx = diagnostic.range.start.line as usize;
                    if let Some(line) = content.lines().nth(line_idx) {
                        let fixed = line.replace("TODO", "").replace("FIXME", "");
                        if fixed != line {
                            actions.push(quick_fix_action(
                                uri.clone(),
                                TextEdit {
                                    range: Range {
                                        start: Position {
                                            line: diagnostic.range.start.line,
                                            character: 0,
                                        },
                                        end: Position {
                                            line: diagnostic.range.start.line,
                                            character: line_char_len(line),
                                        },
                                    },
                                    new_text: fixed,
                                },
                                diagnostic.clone(),
                                "Remove TODO/FIXME marker",
                            ));
                        }
                    }
                }
                "tab-indentation" => {
                    let line_idx = diagnostic.range.start.line as usize;
                    if let Some(line) = content.lines().nth(line_idx) {
                        let fixed = line.replace('\t', "    ");
                        if fixed != line {
                            actions.push(quick_fix_action(
                                uri.clone(),
                                TextEdit {
                                    range: Range {
                                        start: Position {
                                            line: diagnostic.range.start.line,
                                            character: 0,
                                        },
                                        end: Position {
                                            line: diagnostic.range.start.line,
                                            character: line_char_len(line),
                                        },
                                    },
                                    new_text: fixed,
                                },
                                diagnostic.clone(),
                                "Convert tabs to spaces",
                            ));
                        }
                    }
                }
                "trailing-whitespace" => {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        TextEdit {
                            range: diagnostic.range,
                            new_text: String::new(),
                        },
                        diagnostic.clone(),
                        "Remove trailing whitespace",
                    ));
                }
                _ => {}
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };

        let formatted = normalized_format(&content);
        if formatted == content {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&content),
            new_text: formatted,
        }]))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(content) = self.document_text(&uri).await else {
            return Ok(None);
        };
        let formatted = normalized_format(&content);
        if formatted == content {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(vec![TextEdit {
            range: full_document_range(&content),
            new_text: formatted,
        }]))
    }

    async fn rename(&self, params: RenameParams) -> LspResult<Option<WorkspaceEdit>> {
        if !valid_identifier_name(&params.new_name) {
            return Ok(None);
        }

        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let documents = self.all_documents().await;
        let Some(current_content) = documents.get(&uri) else {
            return Ok(None);
        };
        let Some(symbol) = extract_identifier_at(current_content, position) else {
            return Ok(None);
        };

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for (doc_uri, doc_content) in documents {
            let edits: Vec<TextEdit> = find_symbol_occurrences(&doc_content, &symbol.name)
                .into_iter()
                .map(|range| TextEdit {
                    range,
                    new_text: params.new_name.clone(),
                })
                .collect();
            if !edits.is_empty() {
                changes.insert(doc_uri, edits);
            }
        }

        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceEdit {
                changes: Some(changes),
                ..Default::default()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_identifier_handles_cursor_inside_word() {
        let text = "let answer = 42;";
        let symbol = extract_identifier_at(
            text,
            Position {
                line: 0,
                character: 5,
            },
        )
        .expect("identifier should exist");

        assert_eq!(symbol.name, "answer");
        assert_eq!(symbol.range.start.character, 4);
        assert_eq!(symbol.range.end.character, 10);
    }

    #[test]
    fn references_are_word_boundary_aware() {
        let text = "foo foobar foo_1\nfoo";
        let ranges = find_symbol_occurrences(text, "foo");
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn diagnostics_cover_three_quick_fix_kinds() {
        let text = "\tlet x = 1; // TODO   ";
        let diagnostics = build_diagnostics(text, 16);
        let mut codes = diagnostics
            .into_iter()
            .filter_map(|d| match d.code {
                Some(NumberOrString::String(code)) => Some(code),
                _ => None,
            })
            .collect::<Vec<_>>();
        codes.sort();
        assert_eq!(
            codes,
            vec![
                "tab-indentation".to_string(),
                "todo-item".to_string(),
                "trailing-whitespace".to_string()
            ]
        );
    }

    #[test]
    fn semantic_tokens_encode_fn_name_as_function() {
        let text = "fn solve(x: i32) {\n    return x\n}";
        let tokens = semantic_tokens_for(text);
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticKind::Function as u32));
        assert!(tokens
            .iter()
            .any(|t| t.token_type == SemanticKind::Type as u32));
    }

    #[test]
    fn incremental_change_applies_range_patch() {
        let mut content = "def main() -> i64 {\n    1\n}\n".to_string();
        apply_content_changes(
            &mut content,
            vec![TextDocumentContentChangeEvent {
                range: Some(Range {
                    start: Position {
                        line: 1,
                        character: 4,
                    },
                    end: Position {
                        line: 1,
                        character: 5,
                    },
                }),
                range_length: None,
                text: "2".to_string(),
            }],
        );
        assert!(content.contains("    2"));
    }

    #[test]
    fn ast_symbol_collection_reads_top_level_decls() {
        let src = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 { 0 }
"#;
        let symbols = collect_ast_symbols(src);
        assert!(symbols.iter().any(|s| s.name == "Point"));
        assert!(symbols.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn parse_line_col_hint_understands_compiler_format() {
        assert_eq!(parse_line_col_hint("line 12, col 34"), Some((12, 34)));
        assert_eq!(parse_line_col_hint("Line 2, Column 9"), Some((2, 9)));
        assert_eq!(parse_line_col_hint("note: expected `}`"), None);
    }

    #[test]
    fn diagnostic_range_from_payload_uses_line_col_details() {
        let payload = SgcErrorPayload {
            ok: Some(false),
            kind: Some("compile_error".to_string()),
            stage: Some("parse".to_string()),
            message: Some("unexpected token".to_string()),
            details: vec!["line 2, col 5".to_string()],
            location: None,
        };

        let src = "def main() -> i64 {\n    123\n}\n";
        let range = diagnostic_range_from_payload(src, &payload).expect("range should be parsed");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 4);
    }

    #[test]
    fn diagnostic_range_from_payload_prefers_structured_location_span() {
        let src = "def main() -> i64 {\n    let x = 1\n}\n";
        let lo = src.find('x').expect("x should exist") as u32;
        let payload = SgcErrorPayload {
            ok: Some(false),
            kind: Some("compile_error".to_string()),
            stage: Some("parse".to_string()),
            message: Some("unexpected token".to_string()),
            details: vec!["line 1, col 1".to_string()],
            location: Some(SgcErrorLocationPayload {
                line: None,
                column: None,
                span: Some(SgcErrorSpanPayload { lo, hi: lo + 1 }),
            }),
        };

        let range = diagnostic_range_from_payload(src, &payload).expect("range should be parsed");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }
    #[test]
    fn source_span_to_range_maps_offsets() {
        let src = "def main() -> i64 {\n    let x = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('x').expect("x should exist"), 1).into();
        let range = source_span_to_range(src, &span).expect("range should be computed");

        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
        assert_eq!(range.end.line, 1);
    }
    #[test]
    fn diagnostic_range_from_compile_error_handles_parse_span() {
        let src = "def main() -> i64 {\n    let x = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('x').expect("x should exist"), 1).into();
        let err = CompileError::ParseError(ParseError::UnexpectedToken {
            expected: "identifier".to_string(),
            found: ";".to_string(),
            span,
        });

        let range =
            diagnostic_range_from_compile_error(src, &err).expect("range should be extracted");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }

    #[test]
    fn diagnostic_range_from_compile_error_handles_invalid_pattern_with_span() {
        let src = "def main() -> i64 {\n    let = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('=').expect("equal should exist"), 1).into();
        let err = CompileError::ParseError(ParseError::InvalidPatternAt {
            message: "expected identifier".to_string(),
            span,
        });

        let range =
            diagnostic_range_from_compile_error(src, &err).expect("range should be extracted");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }
    #[test]
    fn active_call_site_counts_nested_arguments() {
        let src = "def main() -> i64 {\n    foo(1, bar(2, 3), 4)\n}\n";
        let cursor = src.find("4)").expect("third arg should exist");
        let (name, active_param) = active_call_site(src, cursor).expect("call site should exist");
        assert_eq!(name, "foo");
        assert_eq!(active_param, 2);
    }
    #[test]
    fn folding_ranges_include_regions_and_comment_blocks() {
        let src = "// one\n// two\ndef main() -> i64 {\n    if true {\n        1\n    }\n    0\n}\n";
        let ranges = folding_ranges_for(src);

        assert!(ranges.iter().any(|range|
            range.kind == Some(FoldingRangeKind::Comment)
                && range.start_line == 0
                && range.end_line == 1
        ));
        assert!(ranges.iter().any(|range|
            range.kind == Some(FoldingRangeKind::Region)
                && range.start_line == 2
                && range.end_line == 7
        ));
        assert!(ranges.iter().any(|range|
            range.kind == Some(FoldingRangeKind::Region)
                && range.start_line == 3
                && range.end_line == 5
        ));
    }

    #[test]
    fn collect_function_signatures_reads_function_labels() {
        let src = r#"
def add(a: i64, b: i64) -> i64 {
    a + b
}
"#;

        let signatures = collect_function_signatures(src);
        assert!(signatures.iter().any(|sig| sig.name == "add"));
        assert!(signatures
            .iter()
            .any(|sig| sig.label.contains("def add(")));
    }
}












