use sgfmt::{format_source, FormatOptions};
use tower_lsp::lsp_types::{Position, Range};

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
}
