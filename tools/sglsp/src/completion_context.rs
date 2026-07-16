use tower_lsp::lsp_types::Position;

use crate::text_editing::position_to_byte_index;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportForm {
    Simple,
    Alias,
    Selective,
    Wildcard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttributeTarget {
    Struct,
    Enum,
    Class,
    Function,
    ExternBlock,
    Impl,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttributeNesting {
    Name,
    Arguments { attribute: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    General,
    Member {
        receiver: String,
    },
    Namespace {
        path: String,
    },
    ImportPath {
        form: ImportForm,
        path: String,
    },
    Attribute {
        target: AttributeTarget,
        nesting: AttributeNesting,
    },
}

impl CompletionContext {
    pub(crate) fn classify(content: &str, position: Position) -> Option<Self> {
        let cursor = position_to_byte_index(content, position)?;
        let prefix = &content[..cursor];
        let suffix = &content[cursor..];
        if cursor_is_in_comment_or_string(prefix) {
            return None;
        }

        let line = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
        if line.contains(".(") {
            return None;
        }
        let trimmed = line.trim_start();
        if let Some(attribute) = attribute_context(trimmed, suffix) {
            return Some(attribute);
        }
        let full_line = format!(
            "{}{}",
            trimmed,
            suffix.split('\n').next().unwrap_or_default()
        );
        if let Some(import) = import_context(trimmed, &full_line) {
            return Some(import);
        }

        let token_start = line
            .char_indices()
            .rev()
            .find(|(_, ch)| !is_identifier_char(*ch))
            .map_or(0, |(idx, ch)| idx + ch.len_utf8());
        if line[token_start..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        let before_token = &line[..token_start];
        if let Some(namespace_prefix) = before_token.strip_suffix("::") {
            let path = trailing_path(namespace_prefix);
            return (!path.is_empty()).then_some(Self::Namespace { path });
        }
        if let Some(member_prefix) = before_token.strip_suffix('.') {
            let receiver = trailing_member_expression(member_prefix);
            return (!receiver.is_empty()).then_some(Self::Member { receiver });
        }

        delimiters_are_recoverable(prefix).then_some(Self::General)
    }
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn trailing_member_expression(text: &str) -> String {
    text.chars()
        .rev()
        .take_while(|ch| is_identifier_char(*ch) || matches!(ch, '.' | ':' | '(' | ')' | '&'))
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn trailing_path(text: &str) -> String {
    text.chars()
        .rev()
        .take_while(|ch| is_identifier_char(*ch) || *ch == ':')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .trim_matches(':')
        .to_string()
}

fn import_context(line: &str, full_line: &str) -> Option<CompletionContext> {
    let body = line.strip_prefix("import")?;
    if !body.starts_with(char::is_whitespace) || body.contains(';') {
        return None;
    }
    let body = body.trim_start();
    let form = if full_line.contains(" * from") {
        ImportForm::Wildcard
    } else if full_line.contains('{') {
        ImportForm::Selective
    } else if full_line.contains(" as ") {
        ImportForm::Alias
    } else {
        ImportForm::Simple
    };
    let path = body
        .split(|ch: char| ch.is_whitespace() || ch == '{' || ch == '*')
        .next()
        .unwrap_or_default()
        .to_string();
    Some(CompletionContext::ImportPath { form, path })
}

fn attribute_context(line: &str, suffix: &str) -> Option<CompletionContext> {
    let hash = line.rfind('#')?;
    let tail = &line[hash + 1..];
    if tail.contains(']') {
        return None;
    }
    let nesting = if let Some(paren) = tail.find('(') {
        AttributeNesting::Arguments {
            attribute: tail[..paren].trim_start_matches('[').trim().to_string(),
        }
    } else {
        AttributeNesting::Name
    };
    Some(CompletionContext::Attribute {
        target: attribute_target(suffix),
        nesting,
    })
}

fn attribute_target(suffix: &str) -> AttributeTarget {
    let declaration = suffix
        .lines()
        .skip_while(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with("]") || line.starts_with(')')
        })
        .map(str::trim_start)
        .next()
        .unwrap_or_default();
    if declaration.starts_with("struct ") {
        AttributeTarget::Struct
    } else if declaration.starts_with("enum ") {
        AttributeTarget::Enum
    } else if declaration.starts_with("class ") {
        AttributeTarget::Class
    } else if declaration.starts_with("impl ") {
        AttributeTarget::Impl
    } else if declaration.starts_with("def ")
        || declaration.contains(" def ")
        || (declaration.contains("extern ")
            && (declaration.contains(" fn ") || declaration.ends_with(" fn"))
            && declaration
                .find(" fn ")
                .is_some_and(|function| declaration.find('{').is_none_or(|brace| function < brace)))
    {
        AttributeTarget::Function
    } else if declaration.contains("extern ") && declaration.contains('{') {
        AttributeTarget::ExternBlock
    } else {
        AttributeTarget::Unknown
    }
}

fn cursor_is_in_comment_or_string(prefix: &str) -> bool {
    let bytes = prefix.as_bytes();
    let mut i = 0;
    let mut string = false;
    let mut block_comment = 0u32;
    let mut line_comment = false;
    while i < bytes.len() {
        if line_comment {
            if bytes[i] == b'\n' {
                line_comment = false;
            }
            i += 1;
            continue;
        }
        if block_comment > 0 {
            if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                block_comment -= 1;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if string {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
            } else {
                if bytes[i] == b'"' {
                    string = false;
                }
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            line_comment = true;
            i += 2;
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            block_comment += 1;
            i += 2;
        } else {
            if bytes[i] == b'"' {
                string = true;
            }
            i += 1;
        }
    }
    string || block_comment > 0 || line_comment
}

fn delimiters_are_recoverable(prefix: &str) -> bool {
    let mut stack = Vec::new();
    let bytes = prefix.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => stack.push(bytes[i]),
            b')' | b']' | b'}' => {
                let expected = match bytes[i] {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                if stack.pop() != Some(expected) {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(source: &str, character: u32) -> Option<CompletionContext> {
        CompletionContext::classify(source, Position::new(0, character))
    }

    #[test]
    fn classifies_all_five_contexts_in_incomplete_source() {
        assert_eq!(classify("pri", 3), Some(CompletionContext::General));
        assert_eq!(
            classify("value.na", 8),
            Some(CompletionContext::Member {
                receiver: "value".into()
            })
        );
        assert_eq!(
            classify("std::co", 7),
            Some(CompletionContext::Namespace { path: "std".into() })
        );
        assert!(matches!(
            classify("import std::co", 14),
            Some(CompletionContext::ImportPath { .. })
        ));
        assert!(matches!(
            classify("#[der", 5),
            Some(CompletionContext::Attribute { .. })
        ));
    }

    #[test]
    fn utf16_positions_and_invalid_regions_are_bounded() {
        assert_eq!(
            classify("😀 value.na", 11),
            Some(CompletionContext::Member {
                receiver: "value".into()
            })
        );
        assert_eq!(classify("\"value.na\"", 9), None);
        assert_eq!(classify("value.na // comment", 19), None);
        assert_eq!(classify("value.(na", 9), None);
        assert_eq!(classify("123", 3), None);
        assert_eq!(classify("value.12", 8), None);
    }

    #[test]
    fn recognizes_parser_accepted_import_forms_without_guessing_from_syntax() {
        assert!(matches!(
            classify("import std::io", 14),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Simple,
                ..
            })
        ));
        assert!(matches!(
            classify("import std::io as alias", 23),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Alias,
                ..
            })
        ));
        assert!(matches!(
            classify("import std::io { read", 21),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Selective,
                ..
            })
        ));
        assert!(matches!(
            classify("import std::io * from", 21),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Wildcard,
                ..
            })
        ));
    }

    #[test]
    fn import_form_is_recovered_from_suffix_while_cursor_is_in_path() {
        assert!(matches!(
            CompletionContext::classify("import std::co as coll", Position::new(0, 14)),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Alias,
                path,
            }) if path == "std::co"
        ));
        assert!(matches!(
            CompletionContext::classify("import legacy * from", Position::new(0, 13)),
            Some(CompletionContext::ImportPath {
                form: ImportForm::Wildcard,
                path,
            }) if path == "legacy"
        ));
    }

    #[test]
    fn delimiters_inside_completed_strings_and_comments_do_not_poison_later_lines() {
        assert_eq!(
            CompletionContext::classify("\")\";\nvalue", Position::new(1, 5)),
            Some(CompletionContext::General)
        );
        assert_eq!(
            CompletionContext::classify("// )\nvalue", Position::new(1, 5)),
            Some(CompletionContext::General)
        );
        assert_eq!(
            CompletionContext::classify("broken.(value\nitem.na", Position::new(1, 7)),
            Some(CompletionContext::Member {
                receiver: "item".into()
            })
        );
    }

    #[test]
    fn attribute_target_recovery_distinguishes_ffi_function_and_block() {
        assert!(matches!(
            CompletionContext::classify(
                "#[exp\npub extern \"C\" fn exported() -> i64 { 0 }",
                Position::new(0, 5),
            ),
            Some(CompletionContext::Attribute {
                target: AttributeTarget::Function,
                ..
            })
        ));
        assert!(matches!(
            CompletionContext::classify(
                "#[lin\nextern \"C\" { fn native(); }",
                Position::new(0, 5),
            ),
            Some(CompletionContext::Attribute {
                target: AttributeTarget::ExternBlock,
                ..
            })
        ));
    }
}
