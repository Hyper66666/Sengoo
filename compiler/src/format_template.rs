//! Shared parser for the `format`/f-string mini-language template.
//!
//! This round supports `{}`, `{:?}`, positional `{0}` / `{0:?}` placeholders,
//! right-aligned width `{:>8}`, plus the `{{`/`}}` brace escapes. Precision
//! remains a follow-up.

/// A single piece of a parsed format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatSegment {
    /// Literal text to emit verbatim (brace escapes already resolved).
    Literal(String),
    /// A placeholder consuming either the next automatic or explicit argument.
    Placeholder(FormatPlaceholder),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatPlaceholder {
    pub position: Option<usize>,
    pub style: FormatStyle,
    pub align: FormatAlign,
    pub width: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStyle {
    Display,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAlign {
    None,
    Right,
}

/// Why a format template failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatTemplateError {
    /// A `{` without a matching `}` (or a stray `}` without a `{`).
    UnmatchedBrace,
    /// A non-empty format spec (e.g. `{:?}` / `{name}`), deferred to a later round.
    UnsupportedSpec(String),
}

impl FormatTemplateError {
    pub fn message(&self) -> String {
        match self {
            FormatTemplateError::UnmatchedBrace => {
                "format template has an unmatched `{` or `}` (use `{{`/`}}` to emit a literal brace)"
                    .to_string()
            }
            FormatTemplateError::UnsupportedSpec(spec) => format!(
                "format spec `{{{spec}}}` is not supported yet; only `{{}}` is available in this round"
            ),
        }
    }
}

fn parse_placeholder_body(body: &str) -> Result<FormatPlaceholder, FormatTemplateError> {
    let (position_part, spec_part) = body
        .split_once(':')
        .map_or((body, ""), |(pos, spec)| (pos, spec));
    let position = if position_part.is_empty() {
        None
    } else {
        Some(
            position_part
                .parse::<usize>()
                .map_err(|_| FormatTemplateError::UnsupportedSpec(body.to_string()))?,
        )
    };

    let mut placeholder = FormatPlaceholder {
        position,
        style: FormatStyle::Display,
        align: FormatAlign::None,
        width: None,
    };

    if spec_part.is_empty() {
        return Ok(placeholder);
    }
    if spec_part == "?" {
        placeholder.style = FormatStyle::Debug;
        return Ok(placeholder);
    }
    if let Some(width) = spec_part.strip_prefix('>') {
        let width = width
            .parse::<usize>()
            .map_err(|_| FormatTemplateError::UnsupportedSpec(body.to_string()))?;
        placeholder.align = FormatAlign::Right;
        placeholder.width = Some(width);
        return Ok(placeholder);
    }

    Err(FormatTemplateError::UnsupportedSpec(body.to_string()))
}

/// Parse a format template into its literal/placeholder segments.
pub fn parse_format_template(template: &str) -> Result<Vec<FormatSegment>, FormatTemplateError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                // Collect the placeholder body up to the closing `}`.
                let mut body = String::new();
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    body.push(inner);
                }
                if !closed {
                    return Err(FormatTemplateError::UnmatchedBrace);
                }
                let placeholder = parse_placeholder_body(&body)?;
                if !literal.is_empty() {
                    segments.push(FormatSegment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(FormatSegment::Placeholder(placeholder));
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    return Err(FormatTemplateError::UnmatchedBrace);
                }
            }
            other => literal.push(other),
        }
    }

    if !literal.is_empty() {
        segments.push(FormatSegment::Literal(literal));
    }
    Ok(segments)
}

/// Number of `{}` placeholders the template consumes.
pub fn placeholder_count(segments: &[FormatSegment]) -> usize {
    segments
        .iter()
        .filter(|segment| matches!(segment, FormatSegment::Placeholder(_)))
        .count()
}

/// Minimum number of value arguments required by the template.
pub fn required_arg_count(segments: &[FormatSegment]) -> usize {
    let mut auto_count = 0usize;
    let mut explicit_required = 0usize;
    for segment in segments {
        let FormatSegment::Placeholder(placeholder) = segment else {
            continue;
        };
        if let Some(position) = placeholder.position {
            explicit_required = explicit_required.max(position + 1);
        } else {
            auto_count += 1;
        }
    }
    auto_count.max(explicit_required)
}

#[cfg(test)]
fn display_placeholder() -> FormatPlaceholder {
    FormatPlaceholder {
        position: None,
        style: FormatStyle::Display,
        align: FormatAlign::None,
        width: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text() {
        assert_eq!(
            parse_format_template("hello").unwrap(),
            vec![FormatSegment::Literal("hello".to_string())]
        );
    }

    #[test]
    fn parses_placeholders_and_literals() {
        assert_eq!(
            parse_format_template("a={} b={}").unwrap(),
            vec![
                FormatSegment::Literal("a=".to_string()),
                FormatSegment::Placeholder(display_placeholder()),
                FormatSegment::Literal(" b=".to_string()),
                FormatSegment::Placeholder(display_placeholder()),
            ]
        );
    }

    #[test]
    fn resolves_brace_escapes() {
        assert_eq!(
            parse_format_template("{{{}}}").unwrap(),
            vec![
                FormatSegment::Literal("{".to_string()),
                FormatSegment::Placeholder(display_placeholder()),
                FormatSegment::Literal("}".to_string()),
            ]
        );
    }

    #[test]
    fn rejects_unmatched_brace() {
        assert_eq!(
            parse_format_template("a={"),
            Err(FormatTemplateError::UnmatchedBrace)
        );
        assert_eq!(
            parse_format_template("a}"),
            Err(FormatTemplateError::UnmatchedBrace)
        );
    }

    #[test]
    fn parses_debug_placeholder() {
        assert_eq!(
            parse_format_template("{:?}").unwrap(),
            vec![FormatSegment::Placeholder(FormatPlaceholder {
                position: None,
                style: FormatStyle::Debug,
                align: FormatAlign::None,
                width: None,
            })]
        );
    }

    #[test]
    fn parses_positional_placeholders() {
        assert_eq!(
            parse_format_template("{1}:{0:?}").unwrap(),
            vec![
                FormatSegment::Placeholder(FormatPlaceholder {
                    position: Some(1),
                    style: FormatStyle::Display,
                    align: FormatAlign::None,
                    width: None,
                }),
                FormatSegment::Literal(":".to_string()),
                FormatSegment::Placeholder(FormatPlaceholder {
                    position: Some(0),
                    style: FormatStyle::Debug,
                    align: FormatAlign::None,
                    width: None,
                }),
            ]
        );
    }

    #[test]
    fn parses_right_aligned_width() {
        assert_eq!(
            parse_format_template("{:>8}").unwrap(),
            vec![FormatSegment::Placeholder(FormatPlaceholder {
                position: None,
                style: FormatStyle::Display,
                align: FormatAlign::Right,
                width: Some(8),
            })]
        );
    }

    #[test]
    fn rejects_unsupported_precision_spec() {
        assert_eq!(
            parse_format_template("{:.3}"),
            Err(FormatTemplateError::UnsupportedSpec(":.3".to_string()))
        );
    }

    #[test]
    fn counts_placeholders() {
        let segments = parse_format_template("{} and {}").unwrap();
        assert_eq!(placeholder_count(&segments), 2);
    }

    #[test]
    fn counts_required_positional_args() {
        let segments = parse_format_template("{1}:{0}").unwrap();
        assert_eq!(required_arg_count(&segments), 2);
    }
}
