use sgfmt::{format_source, FormatOptions};
use tower_lsp::lsp_types::{Position, Range, TextEdit};

pub(crate) fn full_document_range(content: &str) -> Range {
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

pub(crate) fn normalized_format(content: &str) -> String {
    format_source(content, &FormatOptions::default())
        .unwrap_or_else(|_| trim_trailing_whitespace(content))
}

pub(crate) fn range_format_edit(content: &str, range: Range) -> Option<TextEdit> {
    let lines = content.split('\n').collect::<Vec<_>>();
    let (start_line, end_line) = selected_line_bounds(&lines, range)?;
    let replacement = formatted_lines_for_range(content, lines.len(), start_line, end_line)
        .unwrap_or_else(|| trim_trailing_whitespace_lines(&lines[start_line..=end_line]));
    let original = lines[start_line..=end_line].join("\n");

    if replacement == original {
        return None;
    }

    Some(TextEdit {
        range: whole_line_range(&lines, start_line, end_line),
        new_text: replacement,
    })
}

fn selected_line_bounds(lines: &[&str], range: Range) -> Option<(usize, usize)> {
    let start_line = range.start.line as usize;
    if start_line >= lines.len() {
        return None;
    }

    let requested_end = range.end.line as usize;
    let mut end_line = requested_end.min(lines.len().saturating_sub(1));
    if range.end.character == 0 && requested_end > start_line {
        end_line = requested_end
            .saturating_sub(1)
            .min(lines.len().saturating_sub(1));
    }

    if end_line < start_line {
        return None;
    }

    Some((start_line, end_line))
}

fn formatted_lines_for_range(
    content: &str,
    expected_line_count: usize,
    start_line: usize,
    end_line: usize,
) -> Option<String> {
    let formatted = format_source(content, &FormatOptions::default()).ok()?;
    let formatted_lines = formatted.split('\n').collect::<Vec<_>>();
    if formatted_lines.len() != expected_line_count {
        return None;
    }
    Some(formatted_lines[start_line..=end_line].join("\n"))
}

fn trim_trailing_whitespace_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

fn whole_line_range(lines: &[&str], start_line: usize, end_line: usize) -> Range {
    Range {
        start: Position {
            line: start_line as u32,
            character: 0,
        },
        end: Position {
            line: end_line as u32,
            character: line_char_len(lines[end_line]),
        },
    }
}

fn trim_trailing_whitespace(content: &str) -> String {
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end_matches([' ', '\t']));
    }
    out
}

fn line_char_len(line: &str) -> u32 {
    u32::try_from(line.chars().count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_format_uses_sgfmt_style_for_parseable_source() {
        let source = "def main()->i64{\nlet x=1+2\nx\n}";

        let formatted = normalized_format(source);

        assert_eq!(
            formatted,
            "def main() -> i64 {\n    let x = 1 + 2;\n    x;\n}"
        );
    }

    #[test]
    fn range_format_edit_limits_replacement_to_requested_lines() {
        let source = "def first() -> i64 {\n    1\n}\n\ndef second()->i64{\nlet x=1+2\nx\n}";
        let range = Range {
            start: Position {
                line: 4,
                character: 0,
            },
            end: Position {
                line: 7,
                character: 1,
            },
        };

        let edit = range_format_edit(source, range).expect("range should format");

        assert_eq!(edit.range, range);
        assert_eq!(
            edit.new_text,
            "def second() -> i64 {\n    let x = 1 + 2;\n    x;\n}"
        );
    }
}
