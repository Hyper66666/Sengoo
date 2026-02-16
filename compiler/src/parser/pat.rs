//! 模式解析

use crate::ast::pattern::{Pattern, PatternKind, RangeEnd, StructPatternField};
use crate::ast::{Ident, Literal, Path};
use crate::error::{CompileError, ParseError};
use crate::lexer::{Keyword, TokenKind};
use crate::Result;
use miette::SourceSpan;

use super::Parser;

impl<'source> Parser<'source> {
    /// 解析模式
    pub(super) fn parse_pattern(&mut self) -> Result<Pattern> {
        self.parse_pattern_impl(0)
    }

    fn parse_pattern_impl(&mut self, precedence: u8) -> Result<Pattern> {
        let lo = self.current_span().lo;
        let mut kind = self.parse_pattern_primary()?;

        // 处理或模式 `A | B`
        while precedence < 1 && self.consume(TokenKind::BitOr).is_some() {
            let right = self.parse_pattern_impl(1)?;
            let span = self.span_at(lo);
            kind = PatternKind::Or(vec![Pattern::new(kind, span), right]);
        }

        let span = self.span_at(lo);
        Ok(Pattern::new(kind, span))
    }

    fn parse_pattern_primary(&mut self) -> Result<PatternKind> {
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // 通配符 `_`
                TokenKind::Underscore => {
                    self.advance();
                    PatternKind::Wildcard
                }

                // 字面量
                TokenKind::Int(Some(n)) => {
                    self.advance();
                    PatternKind::Literal(Literal::Int(*n))
                }
                TokenKind::Float(Some(f)) => {
                    self.advance();
                    PatternKind::Literal(Literal::Float(*f))
                }
                TokenKind::String(Some(s)) => {
                    self.advance();
                    PatternKind::Literal(Literal::String(s.clone()))
                }
                TokenKind::Char(Some(c)) => {
                    self.advance();
                    PatternKind::Literal(Literal::Char(*c))
                }
                TokenKind::TrueKw => {
                    self.advance();
                    PatternKind::Literal(Literal::Bool(true))
                }
                TokenKind::FalseKw => {
                    self.advance();
                    PatternKind::Literal(Literal::Bool(false))
                }
                TokenKind::NullKw => {
                    self.advance();
                    PatternKind::Literal(Literal::Null)
                }

                // 标识符或路径
                TokenKind::Ident => {
                    // 尝试解析路径
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            // 结构体模式 `Point { x, y }`
                            TokenKind::LBrace => {
                                return self.parse_struct_pattern(path);
                            }
                            // 元组结构体模式 `Some(x)`
                            TokenKind::LParen => {
                                return self.parse_tuple_struct_pattern(path);
                            }
                            _ => {}
                        }
                    }
                    // 简单标识符
                    if path.is_simple() {
                        PatternKind::Ident(path.as_simple().unwrap().clone())
                    } else {
                        PatternKind::Path(path)
                    }
                }

                // 元组模式 `(a, b, c)`
                TokenKind::LParen => {
                    return self.parse_tuple_pattern();
                }

                // 切片模式 `[a, b, ..rest]`
                TokenKind::LBracket => {
                    return self.parse_slice_pattern();
                }

                // 范围模式 `1..=100`
                TokenKind::DotDot => {
                    return self.parse_range_pattern();
                }

                _ => {
                    let span = source_span(token.span);
                    return Err(CompileError::ParseError(
                        ParseError::unexpected_token_in_pattern(),
                    ));
                }
            },

            None => {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            }
        };

        Ok(kind)
    }

    /// 解析路径
    pub(super) fn parse_path(&mut self) -> Result<Path> {
        let lo = self.current_span().lo;
        let mut segments = Vec::new();

        loop {
            if let Some(token) = self.current() {
                if matches!(token.kind, TokenKind::Ident) {
                    let span = token.span;
                    let name = self.extract_ident(span);
                    self.advance();
                    segments.push(Ident::new(name, span));

                    if self.consume(TokenKind::ColonColon).is_none() {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if segments.is_empty() {
            return Err(CompileError::ParseError(ParseError::expected_identifier()));
        }

        Ok(Path::new(segments, self.span_at(lo)))
    }

    /// 解析结构体模式 `Point { x, y: y2, .. }`
    fn parse_struct_pattern(&mut self, path: Path) -> Result<PatternKind> {
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut rest = false;

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            if self.consume(TokenKind::DotDot).is_some() {
                rest = true;
                self.consume(TokenKind::RBrace);
                break;
            }

            let name = self.expect_ident()?;
            let (pattern, shorthand) = if self.consume(TokenKind::Colon).is_some() {
                (self.parse_pattern()?, false)
            } else {
                // 简写形式 `{ x }` 等价于 `{ x: x }`
                (
                    Pattern::new(PatternKind::Ident(name.clone()), name.span),
                    true,
                )
            };

            fields.push(StructPatternField::new(name, pattern, shorthand));

            self.consume(TokenKind::Comma);
        }

        Ok(PatternKind::Struct { path, fields, rest })
    }

    /// 解析元组结构体模式 `Some(x, y)`
    fn parse_tuple_struct_pattern(&mut self, path: Path) -> Result<PatternKind> {
        self.expect(TokenKind::LParen)?;

        let mut patterns = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RParen).is_some() {
                break;
            }

            patterns.push(self.parse_pattern()?);

            self.consume(TokenKind::Comma);
        }

        Ok(PatternKind::TupleStruct { path, patterns })
    }

    /// 解析元组模式 `(a, b, c)`
    fn parse_tuple_pattern(&mut self) -> Result<PatternKind> {
        self.advance(); // LParen

        let mut patterns = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RParen).is_some() {
                break;
            }

            patterns.push(self.parse_pattern()?);

            self.consume(TokenKind::Comma);
        }

        Ok(PatternKind::Tuple(patterns))
    }

    /// 解析切片模式 `[a, b, ..rest]`
    fn parse_slice_pattern(&mut self) -> Result<PatternKind> {
        self.advance(); // LBracket

        let mut patterns = Vec::new();
        let mut rest_pattern = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBracket).is_some() {
                break;
            }

            if self.consume(TokenKind::DotDot).is_some() {
                // `..` 或 `..rest`
                if let Some(token) = self.current() {
                    if matches!(token.kind, TokenKind::Ident) {
                        let span = token.span;
                        let name = self.extract_ident(span);
                        self.advance();
                        rest_pattern = Some(Box::new(Pattern::new(
                            PatternKind::Ident(Ident::new(name, span)),
                            span,
                        )));
                    }
                }
                self.consume(TokenKind::RBracket);
                break;
            }

            patterns.push(self.parse_pattern()?);

            self.consume(TokenKind::Comma);
        }

        Ok(PatternKind::Slice(patterns, rest_pattern))
    }

    /// 解析范围模式 `1..100`
    fn parse_range_pattern(&mut self) -> Result<PatternKind> {
        let start = self.parse_pattern_primary()?;
        // TODO: 支持 `..=` 包含范围
        let end = self.parse_pattern_primary()?;

        Ok(PatternKind::Range(
            Box::new(Pattern::new(start, self.current_span())),
            Box::new(Pattern::new(end, self.current_span())),
            RangeEnd::Exclusive,
        ))
    }

    /// 期望标识符
    pub(super) fn expect_ident(&mut self) -> Result<Ident> {
        if let Some(token) = self.current() {
            // 同时接受普通标识符和 self 关键字
            let is_ident = token.kind == TokenKind::Ident || token.kind == TokenKind::SelfLowerKw;
            if is_ident {
                let span = token.span;
                let name = self.extract_ident(span);
                self.advance();
                return Ok(Ident::new(name, span));
            }
        }

        let span = self.current_span();
        Err(CompileError::ParseError(ParseError::expected_identifier()))
    }
}

/// 将 AST Span 转换为 miette SourceSpan
fn source_span(span: crate::lexer::Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
