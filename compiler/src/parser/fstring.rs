//! f-string token 的解析。
//!
//! 词法阶段将 `f"..."` 整体产出为一个 [`crate::lexer::TokenKind::FString`]
//! token，载荷携带模板与每个插值表达式在 token 内的字节区间。parser
//! 在此把它降级为 `format(template, args...)` 调用；每个插值表达式在
//! 原始源码的绝对位置上单独解析，因此其 AST span 与后续诊断都精确
//! 指向原始 f-string 中对应的 `{...}` 文本。

use crate::ast::{Expr, ExprKind, Literal, Path};
use crate::error::{CompileError, ParseError};
use crate::lexer::{FStringLiteral, Span};
use crate::Result;

use super::{source_span, Parser};

impl<'source> Parser<'source> {
    /// 把一个 f-string token 构造为 `format(...)` 调用表达式。
    pub(super) fn parse_fstring_expr(
        &mut self,
        span: Span,
        payload: Option<FStringLiteral>,
    ) -> Result<Expr> {
        let Some(literal) = payload else {
            return Err(CompileError::ParseError(ParseError::InvalidPatternAt {
                message: "unterminated f-string literal".to_string(),
                span: source_span(span),
            }));
        };

        let mut args = Vec::with_capacity(literal.interpolations.len() + 1);
        args.push(Expr::new(
            ExprKind::Literal(Literal::String(literal.template)),
            span,
        ));
        for relative in literal.interpolations {
            let absolute = Span::new(span.lo + relative.lo, span.lo + relative.hi);
            args.push(self.parse_interpolation_expr(absolute)?);
        }

        let func_ident = self.intern_named_ident("format", span);
        let func = Expr::new(ExprKind::Path(Path::new(vec![func_ident], span)), span);
        Ok(Expr::call(func, args, span))
    }

    /// 在原始源码的绝对区间上解析一个插值表达式。
    fn parse_interpolation_expr(&mut self, span: Span) -> Result<Expr> {
        let text = self.source_slice(span);
        if text.trim().is_empty() {
            return Err(CompileError::ParseError(ParseError::InvalidPatternAt {
                message: "f-string interpolation `{}` is empty".to_string(),
                span: source_span(span),
            }));
        }

        // 用空白前缀把插值文本对齐到原始偏移，使子解析产出的所有
        // span（含内部子表达式）与原始源码逐字节对应。
        let mut padded = String::with_capacity(span.hi as usize);
        padded.extend(std::iter::repeat_n(' ', span.lo as usize));
        padded.push_str(text);

        let mut sub = Parser::new(&padded);
        std::mem::swap(&mut sub.interner, &mut self.interner);
        let parsed = sub.parse_expr().and_then(|expr| {
            if sub.is_eof() {
                Ok(expr)
            } else {
                Err(CompileError::ParseError(ParseError::InvalidPatternAt {
                    message: "unexpected token in f-string interpolation".to_string(),
                    span: source_span(sub.current_span()),
                }))
            }
        });
        std::mem::swap(&mut sub.interner, &mut self.interner);

        parsed.map_err(|err| match err {
            CompileError::ParseError(parse_err) => {
                let spanned = match parse_err {
                    ParseError::UnexpectedEof => ParseError::InvalidPatternAt {
                        message: "incomplete expression in f-string interpolation".to_string(),
                        span: source_span(span),
                    },
                    other => other.with_span(source_span(span)),
                };
                CompileError::ParseError(spanned)
            }
            other => other,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{DeclKind, ExprKind, Literal, StmtKind};
    use crate::error::{CompileError, ParseError};
    use crate::parser::Parser;

    fn first_fn_body_exprs(source: &str) -> Vec<crate::ast::Expr> {
        let program = Parser::parse(source).expect("program should parse");
        let decl = program.decls.into_iter().next().expect("expected a decl");
        let DeclKind::Function(func) = decl.kind else {
            panic!("expected a function declaration");
        };
        func.body
            .stmts
            .into_iter()
            .filter_map(|stmt| match stmt.kind {
                StmtKind::Let { value, .. } => value.map(|boxed| *boxed),
                StmtKind::Expr(expr) => Some(*expr),
                _ => None,
            })
            .collect()
    }

    fn expect_format_call(expr: &crate::ast::Expr) -> (&str, &[crate::ast::Expr]) {
        let ExprKind::Call { func, args } = &expr.kind else {
            panic!("expected a call expression, got {:?}", expr.kind);
        };
        let ExprKind::Path(path) = &func.kind else {
            panic!("expected a path callee, got {:?}", func.kind);
        };
        assert_eq!(path.segments[0].name, "format");
        let ExprKind::Literal(Literal::String(template)) = &args[0].kind else {
            panic!("expected a string template, got {:?}", args[0].kind);
        };
        (template.as_str(), &args[1..])
    }

    #[test]
    fn lowers_debug_spec_to_format_placeholder() {
        let source = "def main() -> i64 { let s = f\"{p:?}\"; 0 }";
        let exprs = first_fn_body_exprs(source);
        let (template, args) = expect_format_call(&exprs[0]);
        assert_eq!(template, "{:?}");
        assert_eq!(args.len(), 1);
        let span = args[0].span;
        assert_eq!(
            &source[span.lo as usize..span.hi as usize],
            "p",
            "debug spec should not be part of the interpolation expression span"
        );
    }

    #[test]
    fn lowers_simple_interpolation_to_format_call() {
        let exprs = first_fn_body_exprs("def main() -> i64 { let s = f\"x={x}\"; 0 }");
        let (template, args) = expect_format_call(&exprs[0]);
        assert_eq!(template, "x={}");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn lowers_multiple_and_compound_interpolations() {
        let exprs = first_fn_body_exprs("def main() -> i64 { let s = f\"{a}, {b + c * 2}!\"; 0 }");
        let (template, args) = expect_format_call(&exprs[0]);
        assert_eq!(template, "{}, {}!");
        assert_eq!(args.len(), 2);
        assert!(matches!(args[1].kind, ExprKind::Binary { .. }));
    }

    #[test]
    fn preserves_brace_escapes_in_template() {
        let exprs = first_fn_body_exprs("def main() -> i64 { let s = f\"{{literal}} {v}\"; 0 }");
        let (template, args) = expect_format_call(&exprs[0]);
        assert_eq!(template, "{{literal}} {}");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn interpolation_span_maps_to_original_source() {
        let source = "def main() -> i64 { let s = f\"x={value}\"; 0 }";
        let exprs = first_fn_body_exprs(source);
        let (_, args) = expect_format_call(&exprs[0]);
        let span = args[0].span;
        assert_eq!(
            &source[span.lo as usize..span.hi as usize],
            "value",
            "interpolation expression span should point at the original text"
        );
    }

    #[test]
    fn empty_interpolation_error_points_at_original_braces() {
        let source = "def main() -> i64 { let s = f\"bad {} spot\"; 0 }";
        let err = Parser::parse(source).expect_err("empty interpolation should be rejected");
        let CompileError::ParseError(ParseError::InvalidPatternAt { message, span }) = err else {
            panic!("expected a spanned parse error, got {err:?}");
        };
        assert!(message.contains("empty"));
        let expected = source.find("{}").unwrap() + 1;
        assert_eq!(span.offset(), expected);
    }

    #[test]
    fn interpolation_parse_error_points_into_original_source() {
        let source = "def main() -> i64 { let s = f\"sum={1 +}\"; 0 }";
        let err = Parser::parse(source).expect_err("malformed interpolation should be rejected");
        let CompileError::ParseError(ParseError::InvalidPatternAt { span, .. }) = err else {
            panic!("expected a spanned parse error, got {err:?}");
        };
        let lo = source.find("1 +").unwrap();
        let hi = lo + "1 +".len();
        assert!(
            span.offset() >= lo && span.offset() <= hi,
            "error span {span:?} should fall inside the original interpolation {lo}..{hi}"
        );
    }

    #[test]
    fn unterminated_fstring_reports_stable_diagnostic() {
        let source = "def main() -> i64 { let s = f\"never ends; 0 }";
        let err = Parser::parse(source).expect_err("unterminated f-string should be rejected");
        let CompileError::ParseError(ParseError::InvalidPatternAt { message, .. }) = err else {
            panic!("expected a spanned parse error, got {err:?}");
        };
        assert!(message.contains("unterminated f-string"));
    }

    #[test]
    fn nested_string_literal_inside_interpolation() {
        let exprs =
            first_fn_body_exprs("def main() -> i64 { let s = f\"v={pick(\"a\", name)}\"; 0 }");
        let (template, args) = expect_format_call(&exprs[0]);
        assert_eq!(template, "v={}");
        assert_eq!(args.len(), 1);
        assert!(matches!(args[0].kind, ExprKind::Call { .. }));
    }
}
