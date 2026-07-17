use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};

#[derive(Debug, Clone, Copy)]
pub(super) enum SemanticKind {
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

pub(super) fn semantic_legend() -> SemanticTokensLegend {
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

pub(super) fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

pub(super) fn byte_to_char_index(s: &str, byte_idx: usize) -> u32 {
    s[..byte_idx].encode_utf16().count() as u32
}

pub(super) fn line_char_len(line: &str) -> u32 {
    line.chars().count() as u32
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

pub(super) fn semantic_tokens_for(content: &str) -> Vec<SemanticToken> {
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
