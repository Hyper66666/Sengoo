use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;
use miette::SourceSpan;

use super::super::Parser;

impl<'source> Parser<'source> {
    /// 解析数组字面量表达式。
    pub(super) fn parse_array_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LBracket)?;

        let mut elements = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBracket).is_some() {
                break;
            }

            elements.push(self.parse_expr()?);

            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(ExprKind::Array(elements), self.span_at(lo)))
    }

    /// 解析函数调用或元组表达式，以路径为起点。
    pub(super) fn parse_call_or_tuple_expr(&mut self, path: Path) -> Result<Expr> {
        let lo = path.span.lo;
        self.expect(TokenKind::LParen)?;

        let mut args = Vec::new();
        while !self.is_eof() {
            if self.consume(TokenKind::RParen).is_some() {
                break;
            }
            args.push(self.parse_expr()?);
            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(
            ExprKind::Call {
                func: Box::new(Expr::new(ExprKind::Path(path.clone()), path.span)),
                args,
            },
            self.span_at(lo),
        ))
    }

    /// 解析结构体字面量初始化表达式。
    pub(super) fn parse_struct_expr(&mut self, path: Path) -> Result<Expr> {
        let lo = path.span.lo;
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut base = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 解析结构体更新语法（..base）。
            if self.consume(TokenKind::DotDot).is_some() {
                base = Some(Box::new(self.parse_expr()?));
                self.consume(TokenKind::RBrace);
                break;
            }

            let (name, name_span) = if let Some(token) = self.current() {
                match &token.kind {
                    TokenKind::Ident => {
                        let span = token.span;
                        self.advance();
                        (FieldName::Ident(self.intern_ident(span)), span)
                    }
                    TokenKind::String(Some(s)) => {
                        let span = token.span;
                        let s = s.clone();
                        self.advance();
                        (FieldName::String(s), span)
                    }
                    _ => {
                        return Err(CompileError::ParseError(ParseError::InvalidStructField {
                            found: format!("{:?}", token.kind),
                            span: source_span(token.span),
                        }));
                    }
                }
            } else {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            };

            if self.consume(TokenKind::Colon).is_some() {
                let value = self.parse_expr()?;
                fields.push(FieldValue::new(name, value, self.current_span()));
            } else {
                // 允许字段名为标识符或整数（元组结构体）。
                if let FieldName::Ident(ref ident) = name {
                    fields.push(FieldValue::shorthand(ident.clone(), self.current_span()));
                } else {
                    return Err(CompileError::ParseError(
                        ParseError::InvalidStructFieldShorthand {
                            span: source_span(name_span),
                        },
                    ));
                }
            }

            self.consume(TokenKind::Comma);
        }

        Ok(Expr::new(
            ExprKind::Struct { path, fields, base },
            self.span_at(lo),
        ))
    }
}

/// 将词法分析器的Span转换为AST的SourceSpan。
fn source_span(span: crate::lexer::Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
