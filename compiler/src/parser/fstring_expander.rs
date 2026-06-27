//! Source-to-source expansion of f-string literals.
//!
//! `f"a {x} b {y}"` is rewritten to `format("a {} b {}", x, y)` before lexing,
//! so the interpolated expressions become ordinary source tokens parsed by the
//! normal expression grammar and the template reuses the `format` mini-language.
//!
//! Only single-line `f"..."` literals are recognized. Brace escapes `{{`/`}}`
//! are preserved verbatim for the `format` template parser. Interpolation
//! bodies may themselves contain braces, string, and char literals.

use crate::error::{CompileError, ParseError};
use crate::Result;
use std::borrow::Cow;

pub(super) fn expand_fstrings(source: &str) -> Result<Cow<'_, str>> {
    // Cheap short-circuit: no `f"` anywhere means nothing to expand.
    if !contains_fstring_prefix(source) {
        return Ok(Cow::Borrowed(source));
    }

    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 16);
    let mut i = 0usize;
    let mut prev_significant: Option<char> = None;

    while i < chars.len() {
        let ch = chars[i];

        // Skip over comments and non-f string/char literals verbatim so we do
        // not misinterpret their contents as f-strings.
        if ch == '/' && chars.get(i + 1) == Some(&'/') {
            copy_line_comment(&chars, &mut i, &mut out);
            prev_significant = None;
            continue;
        }
        if ch == '/' && chars.get(i + 1) == Some(&'*') {
            copy_block_comment(&chars, &mut i, &mut out);
            prev_significant = None;
            continue;
        }
        if ch == '"' {
            copy_string_literal(&chars, &mut i, &mut out);
            prev_significant = Some('"');
            continue;
        }
        if ch == '\'' {
            copy_char_literal(&chars, &mut i, &mut out);
            prev_significant = Some('\'');
            continue;
        }

        // An `f"..."` is only an f-string when the `f` does not continue an
        // identifier (e.g. `xf"..."` is the identifier `xf` then a string).
        if ch == 'f'
            && chars.get(i + 1) == Some(&'"')
            && chars.get(i + 2) != Some(&'"')
            && !prev_significant.is_some_and(is_ident_continue)
        {
            let expanded = expand_one_fstring(&chars, &mut i)?;
            out.push_str(&expanded);
            prev_significant = Some(')');
            continue;
        }

        out.push(ch);
        if !ch.is_whitespace() {
            prev_significant = Some(ch);
        }
        i += 1;
    }

    Ok(Cow::Owned(out))
}

fn contains_fstring_prefix(source: &str) -> bool {
    let bytes = source.as_bytes();
    bytes
        .windows(2)
        .any(|window| window[0] == b'f' && window[1] == b'"')
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Copy a `// ...` line comment verbatim, advancing `i` past it.
fn copy_line_comment(chars: &[char], i: &mut usize, out: &mut String) {
    while *i < chars.len() && chars[*i] != '\n' {
        out.push(chars[*i]);
        *i += 1;
    }
}

/// Copy a `/* ... */` block comment verbatim, advancing `i` past it.
fn copy_block_comment(chars: &[char], i: &mut usize, out: &mut String) {
    out.push(chars[*i]);
    out.push(chars[*i + 1]);
    *i += 2;
    while *i < chars.len() {
        if chars[*i] == '*' && chars.get(*i + 1) == Some(&'/') {
            out.push('*');
            out.push('/');
            *i += 2;
            return;
        }
        out.push(chars[*i]);
        *i += 1;
    }
}

/// Copy a normal `"..."` string literal verbatim, advancing `i` past it.
fn copy_string_literal(chars: &[char], i: &mut usize, out: &mut String) {
    out.push(chars[*i]); // opening quote
    *i += 1;
    while *i < chars.len() {
        let ch = chars[*i];
        out.push(ch);
        *i += 1;
        if ch == '\\' {
            if *i < chars.len() {
                out.push(chars[*i]);
                *i += 1;
            }
        } else if ch == '"' {
            return;
        }
    }
}

/// Copy a `'...'` char literal verbatim, advancing `i` past it.
fn copy_char_literal(chars: &[char], i: &mut usize, out: &mut String) {
    out.push(chars[*i]); // opening quote
    *i += 1;
    while *i < chars.len() {
        let ch = chars[*i];
        out.push(ch);
        *i += 1;
        if ch == '\\' {
            if *i < chars.len() {
                out.push(chars[*i]);
                *i += 1;
            }
        } else if ch == '\'' {
            return;
        }
    }
}

/// Expand a single f-string starting at `chars[*i] == 'f'`, leaving `*i` just
/// past the closing quote and returning the `format(...)` replacement text.
fn expand_one_fstring(chars: &[char], i: &mut usize) -> Result<String> {
    *i += 2; // skip `f"`
    let mut template = String::new();
    let mut exprs: Vec<String> = Vec::new();

    while *i < chars.len() {
        let ch = chars[*i];
        match ch {
            '"' => {
                *i += 1;
                return Ok(render_format_call(&template, &exprs));
            }
            '\\' => {
                template.push('\\');
                *i += 1;
                if *i < chars.len() {
                    template.push(chars[*i]);
                    *i += 1;
                }
            }
            '{' if chars.get(*i + 1) == Some(&'{') => {
                template.push_str("{{");
                *i += 2;
            }
            '}' if chars.get(*i + 1) == Some(&'}') => {
                template.push_str("}}");
                *i += 2;
            }
            '{' => {
                *i += 1;
                let expr = read_interpolation(chars, i)?;
                if expr.trim().is_empty() {
                    return Err(fstring_error("f-string interpolation `{}` is empty"));
                }
                template.push_str("{}");
                exprs.push(expr.trim().to_string());
            }
            other => {
                template.push(other);
                *i += 1;
            }
        }
    }

    Err(fstring_error("unterminated f-string literal"))
}

/// Read an interpolation body up to its matching `}`, tracking nested braces and
/// skipping string/char literals. `*i` is left just past the closing `}`.
fn read_interpolation(chars: &[char], i: &mut usize) -> Result<String> {
    let mut expr = String::new();
    let mut depth = 0usize;

    while *i < chars.len() {
        let ch = chars[*i];
        match ch {
            '}' if depth == 0 => {
                *i += 1;
                return Ok(expr);
            }
            '{' => {
                depth += 1;
                expr.push(ch);
                *i += 1;
            }
            '}' => {
                depth -= 1;
                expr.push(ch);
                *i += 1;
            }
            '"' => copy_string_literal(chars, i, &mut expr),
            '\'' => copy_char_literal(chars, i, &mut expr),
            other => {
                expr.push(other);
                *i += 1;
            }
        }
    }

    Err(fstring_error("unterminated interpolation in f-string"))
}

fn render_format_call(template: &str, exprs: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    out.push_str("format(\"");
    out.push_str(template);
    out.push('"');
    for expr in exprs {
        out.push_str(", ");
        out.push_str(expr);
    }
    out.push(')');
    out
}

fn fstring_error(message: &str) -> CompileError {
    CompileError::ParseError(ParseError::InvalidPattern(message.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(source: &str) -> String {
        expand_fstrings(source).unwrap().into_owned()
    }

    #[test]
    fn leaves_sources_without_fstrings_untouched() {
        let src = "let s = \"a {} b\";";
        assert!(matches!(expand_fstrings(src).unwrap(), Cow::Borrowed(_)));
    }

    #[test]
    fn rewrites_simple_interpolation() {
        assert_eq!(expand("f\"x={x}\""), "format(\"x={}\", x)");
    }

    #[test]
    fn rewrites_multiple_interpolations() {
        assert_eq!(expand("f\"{a} and {b}\""), "format(\"{} and {}\", a, b)");
    }

    #[test]
    fn preserves_brace_escapes() {
        assert_eq!(expand("f\"{{{x}}}\""), "format(\"{{{}}}\", x)");
    }

    #[test]
    fn allows_complex_expressions() {
        assert_eq!(expand("f\"sum={a + b}\""), "format(\"sum={}\", a + b)");
    }

    #[test]
    fn does_not_treat_identifier_suffix_as_fstring() {
        let src = "let xf\"nope\";";
        // `xf` is an identifier; the `f\"` here must not start an f-string.
        assert_eq!(expand(src), src);
    }

    #[test]
    fn ignores_fstring_lookalikes_in_strings() {
        let src = "let s = \"f\\\"inside\\\"\";";
        assert_eq!(expand(src), src);
    }

    #[test]
    fn rejects_empty_interpolation() {
        assert!(expand_fstrings("f\"{}\"").is_err());
    }
}
