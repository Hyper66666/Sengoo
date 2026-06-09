use super::*;

const ASSERT_HELPERS: &[&str] = &[
    "assert",
    "assert_true",
    "assert_false",
    "assert_eq_i64",
    "assert_ne_i64",
    "assert_eq_bool",
    "assert_ne_bool",
    "assert_eq_str",
    "assert_ne_str",
    "assert_eq_f64",
    "assert_ne_f64",
];

pub(super) fn is_assert_helper(name: &str) -> bool {
    ASSERT_HELPERS.contains(&name)
}

pub(super) fn append_assert_callsite_args(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    site_lo: Option<u32>,
    arg_locals: &mut Vec<Local>,
) {
    if !is_assert_helper(name) {
        return;
    }
    let ctx_data = (*ctx.options.assert_callsite).clone();
    let source_file = ctx_data.source_file.as_deref().unwrap_or("").to_string();
    let file_local = ctx.lower_literal(&HIRLiteral::String(source_file));
    arg_locals.push(file_local);

    let line = site_lo
        .and_then(|lo| {
            ctx_data
                .source_text
                .as_deref()
                .map(|source| line_number_at_offset(source, ctx_data.user_base_offset, lo))
        })
        .unwrap_or(0);
    let line_local = ctx.lower_literal(&HIRLiteral::Int(line as i64));
    arg_locals.push(line_local);
}

pub(crate) fn line_number_at_offset(source: &str, base_offset: u32, span_lo: u32) -> u32 {
    let start = base_offset as usize;
    let end = span_lo as usize;
    if end < start || end > source.len() {
        return 1;
    }
    source[start..end]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u32
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_number_counts_from_user_base_offset() {
        let source = "line1\nline2\nline3\n";
        assert_eq!(line_number_at_offset(source, 0, 7), 2);
        assert_eq!(line_number_at_offset("prefix\nline1\n", 7, 13), 2);
    }
}
