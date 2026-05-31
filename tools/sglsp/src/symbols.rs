use sengoo_compiler::ast::{ClassMember, Decl, DeclKind, Function, TraitItem};
use sengoo_compiler::Parser as SgParser;
use tower_lsp::lsp_types::{CompletionItemKind, Position, Range, SymbolKind};

use super::semantic::{byte_to_char_index, is_identifier_byte};
use super::text_editing::{char_to_byte_index, span_to_range};

#[derive(Debug, Clone)]
pub(super) struct SymbolAt {
    pub(super) name: String,
    pub(super) range: Range,
}

#[derive(Debug, Clone)]
pub(super) struct AstSymbol {
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) kind: CompletionItemKind,
    pub(super) range: Range,
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

fn push_function_symbol(
    content: &str,
    function: &Function,
    detail: &'static str,
    kind: CompletionItemKind,
    out: &mut Vec<AstSymbol>,
) {
    out.push(AstSymbol {
        name: function.name.name.clone(),
        detail: detail.to_string(),
        kind,
        range: span_to_range(content, function.name.span.lo, function.name.span.hi),
    });
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

    match &decl.kind {
        DeclKind::Module(module_decl) => {
            for nested in &module_decl.items {
                collect_decl_symbols(content, nested, out);
            }
        }
        DeclKind::Class(class_decl) => {
            for member in &class_decl.members {
                if let ClassMember::Method(method) = member {
                    push_function_symbol(
                        content,
                        method,
                        "method",
                        CompletionItemKind::METHOD,
                        out,
                    );
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    push_function_symbol(
                        content,
                        function,
                        "trait method",
                        CompletionItemKind::METHOD,
                        out,
                    );
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            for method in &impl_decl.items {
                push_function_symbol(content, method, "method", CompletionItemKind::METHOD, out);
            }
        }
        _ => {}
    }
}

pub(super) fn collect_ast_symbols(content: &str) -> Vec<AstSymbol> {
    let Ok(program) = SgParser::parse(content) else {
        return Vec::new();
    };

    let mut symbols = Vec::new();
    for decl in &program.decls {
        collect_decl_symbols(content, decl, &mut symbols);
    }
    symbols
}

pub(super) fn completion_kind_to_symbol_kind(kind: CompletionItemKind) -> SymbolKind {
    match kind {
        CompletionItemKind::FUNCTION => SymbolKind::FUNCTION,
        CompletionItemKind::STRUCT => SymbolKind::STRUCT,
        CompletionItemKind::ENUM => SymbolKind::ENUM,
        CompletionItemKind::CLASS => SymbolKind::CLASS,
        CompletionItemKind::INTERFACE => SymbolKind::INTERFACE,
        CompletionItemKind::TYPE_PARAMETER => SymbolKind::TYPE_PARAMETER,
        CompletionItemKind::CONSTANT => SymbolKind::CONSTANT,
        CompletionItemKind::MODULE => SymbolKind::MODULE,
        CompletionItemKind::METHOD => SymbolKind::METHOD,
        _ => SymbolKind::VARIABLE,
    }
}

pub(super) fn extract_identifier_at(content: &str, position: Position) -> Option<SymbolAt> {
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

pub(super) fn find_symbol_occurrences(content: &str, symbol: &str) -> Vec<Range> {
    if symbol.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let symbol_bytes = symbol.as_bytes();

    for (line_idx, line) in content.lines().enumerate() {
        let bytes = line.as_bytes();
        let code_mask = code_byte_mask(line);
        let mut i = 0usize;

        while i + symbol_bytes.len() <= bytes.len() {
            let matched = &bytes[i..i + symbol_bytes.len()] == symbol_bytes;
            let left_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
            let right_bound = i + symbol_bytes.len();
            let right_ok = right_bound == bytes.len() || !is_identifier_byte(bytes[right_bound]);

            if matched && left_ok && right_ok && is_code_span(&code_mask, i, right_bound) {
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

fn code_byte_mask(line: &str) -> Vec<bool> {
    let bytes = line.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut in_string = false;
    let mut i = 0usize;

    while i < bytes.len() {
        if in_string {
            mask[i] = false;
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                mask[i + 1] = false;
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            for item in &mut mask[i..] {
                *item = false;
            }
            break;
        }

        if bytes[i] == b'"' {
            mask[i] = false;
            in_string = true;
        }
        i += 1;
    }

    mask
}

fn is_code_span(mask: &[bool], start: usize, end: usize) -> bool {
    start < end && end <= mask.len() && mask[start..end].iter().all(|is_code| *is_code)
}

pub(super) fn find_declaration_in_text(content: &str, symbol: &str) -> Option<Range> {
    let declaration_keywords = ["def", "fn", "struct", "let", "const", "type", "enum"];

    for keyword in declaration_keywords {
        let pattern = format!("{keyword} {symbol}");
        for (line_idx, line) in content.lines().enumerate() {
            if let Some(pos) = line.find(&pattern) {
                let code_mask = code_byte_mask(line);
                let symbol_start = pos + keyword.len() + 1;
                let symbol_end = symbol_start + symbol.len();
                let bytes = line.as_bytes();

                let left_ok = symbol_start == 0 || !is_identifier_byte(bytes[symbol_start - 1]);
                let right_ok = symbol_end == bytes.len() || !is_identifier_byte(bytes[symbol_end]);
                if left_ok && right_ok && is_code_span(&code_mask, pos, symbol_end) {
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

    None
}

pub(super) fn find_definition_in_text(content: &str, symbol: &str) -> Option<Range> {
    if let Some(range) = find_declaration_in_text(content, symbol) {
        return Some(range);
    }

    find_symbol_occurrences(content, symbol).into_iter().next()
}

pub(super) fn valid_identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
