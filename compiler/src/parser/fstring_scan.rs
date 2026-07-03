//! f-string 的字节级跳过扫描，供各文本展开阶段共享。
//!
//! 展开阶段（宏/属性/derive）在词法之前对源码做逐字节扫描，
//! 必须把 `f"..."` 整体视为字面量跳过，否则插值内部的嵌套引号
//! 会让朴素的字符串跳过在错误位置提前结束。

/// 若 `start` 处是 `f"..."` 字面量，返回其闭合引号之后的下标。
///
/// `f` 前若是标识符字符（如 `xf"..."`），则不是 f-string。
/// 未闭合（行尾/EOF 前无闭合引号）时返回 `None`，交由词法阶段诊断。
pub(super) fn skip_fstring_literal(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'f') || bytes.get(start + 1) != Some(&b'"') {
        return None;
    }
    if start > 0 && is_ident_continue(bytes[start - 1]) {
        return None;
    }

    let mut i = start + 2;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(i + 1),
            b'\n' => return None,
            b'\\' => i += 2,
            b'{' if bytes.get(i + 1) == Some(&b'{') => i += 2,
            b'{' => {
                i += 1;
                skip_interpolation(bytes, &mut i)?;
            }
            _ => i += 1,
        }
    }
    None
}

/// 跳过一段插值表达式（含嵌套花括号与字符串/字符字面量），
/// 游标停在闭合 `}` 之后。
fn skip_interpolation(bytes: &[u8], i: &mut usize) -> Option<()> {
    let mut depth = 0usize;
    while *i < bytes.len() {
        match bytes[*i] {
            b'}' if depth == 0 => {
                *i += 1;
                return Some(());
            }
            b'{' => {
                depth += 1;
                *i += 1;
            }
            b'}' => {
                depth -= 1;
                *i += 1;
            }
            quote @ (b'"' | b'\'') => skip_quoted(bytes, i, quote)?,
            b'\n' => return None,
            _ => *i += 1,
        }
    }
    None
}

/// 跳过插值内部的引号字面量。
fn skip_quoted(bytes: &[u8], i: &mut usize, quote: u8) -> Option<()> {
    *i += 1;
    while *i < bytes.len() {
        match bytes[*i] {
            b'\\' => *i += 2,
            b'\n' => return None,
            b if b == quote => {
                *i += 1;
                return Some(());
            }
            _ => *i += 1,
        }
    }
    None
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_simple_fstring() {
        let src = br#"f"a {x} b" rest"#;
        assert_eq!(skip_fstring_literal(src, 0), Some(10));
    }

    #[test]
    fn skips_nested_quotes_in_interpolation() {
        let src = br#"f"a {g("x}")} b" rest"#;
        assert_eq!(skip_fstring_literal(src, 0), Some(16));
    }

    #[test]
    fn rejects_identifier_prefix() {
        let src = br#"xf"nope""#;
        assert_eq!(skip_fstring_literal(src, 1), None);
    }

    #[test]
    fn rejects_unterminated() {
        let src = br#"f"never ends"#;
        assert_eq!(skip_fstring_literal(src, 0), None);
    }
}
