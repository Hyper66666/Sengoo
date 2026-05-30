use tower_lsp::lsp_types::{
    FoldingRange, FoldingRangeKind, Position, Range, TextDocumentContentChangeEvent,
};

pub(super) fn char_to_byte_index(s: &str, character: u32) -> usize {
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

pub(super) fn clamp_to_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(super) fn position_to_byte_index(content: &str, position: Position) -> Option<usize> {
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

pub(super) fn span_to_range(content: &str, lo: u32, hi: u32) -> Range {
    Range {
        start: byte_index_to_position(content, lo as usize),
        end: byte_index_to_position(content, hi as usize),
    }
}

pub(super) fn apply_content_changes(
    content: &mut String,
    changes: Vec<TextDocumentContentChangeEvent>,
) {
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

pub(super) fn folding_ranges_for(content: &str) -> Vec<FoldingRange> {
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
