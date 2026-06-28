//! Shared parser for the `format`/f-string mini-language template.
//!
//! This round supports the empty placeholder `{}` (auto-positional `Display`),
//! the scalar `{:?}` Debug placeholder, plus the `{{`/`}}` brace escapes. Richer
//! specs such as `{:>8}` are intentionally rejected here and tracked as a
//! follow-up.

/// A single piece of a parsed format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatSegment {
    /// Literal text to emit verbatim (brace escapes already resolved).
    Literal(String),
    /// An `{}` placeholder consuming the next positional argument.
    Placeholder(FormatStyle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStyle {
    Display,
    Debug,
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
                let style = if body.is_empty() {
                    FormatStyle::Display
                } else if body == ":?" {
                    FormatStyle::Debug
                } else {
                    return Err(FormatTemplateError::UnsupportedSpec(body));
                };
                if !literal.is_empty() {
                    segments.push(FormatSegment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(FormatSegment::Placeholder(style));
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
                FormatSegment::Placeholder(FormatStyle::Display),
                FormatSegment::Literal(" b=".to_string()),
                FormatSegment::Placeholder(FormatStyle::Display),
            ]
        );
    }

    #[test]
    fn resolves_brace_escapes() {
        assert_eq!(
            parse_format_template("{{{}}}").unwrap(),
            vec![
                FormatSegment::Literal("{".to_string()),
                FormatSegment::Placeholder(FormatStyle::Display),
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
            vec![FormatSegment::Placeholder(FormatStyle::Debug)]
        );
    }

    #[test]
    fn rejects_unsupported_spec() {
        assert_eq!(
            parse_format_template("{:>8}"),
            Err(FormatTemplateError::UnsupportedSpec(":>8".to_string()))
        );
    }

    #[test]
    fn counts_placeholders() {
        let segments = parse_format_template("{} and {}").unwrap();
        assert_eq!(placeholder_count(&segments), 2);
    }
}
