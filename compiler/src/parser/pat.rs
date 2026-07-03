//! 模式解析。

use crate::ast::pattern::{Pattern, PatternKind, RangeEnd, StructPatternField};
use crate::ast::{Ident, Literal, Path};
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

impl<'source> Parser<'source> {
    /// 解析模式表达式。
    pub(super) fn parse_pattern(&mut self) -> Result<Pattern> {
        self.parse_pattern_impl(0)
    }

    fn parse_pattern_impl(&mut self, precedence: u8) -> Result<Pattern> {
        let lo = self.current_span().lo;
        let mut kind = self.parse_pattern_primary()?;

        if self.consume(TokenKind::DotDot).is_some() {
            let inclusive = self.consume(TokenKind::Eq).is_some();
            let end = self.parse_pattern_primary()?;
            let start_span = self.span_at(lo);
            let end_span = self.current_span();
            kind = PatternKind::Range(
                Box::new(Pattern::new(kind, start_span)),
                Box::new(Pattern::new(end, end_span)),
                if inclusive {
                    RangeEnd::Inclusive
                } else {
                    RangeEnd::Exclusive
                },
            );
        }

        // 解析或模式 `A | B`。
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
                // 解析通配符 `_`。
                TokenKind::Underscore => {
                    self.advance();
                    PatternKind::Wildcard
                }

                // 解析字面量模式。
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

                // 解析标识符、路径或构造模式。
                TokenKind::Ident => {
                    // 先解析路径，再根据后续 Token 判断具体模式形态。
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            // 结构体模式 `Point { x, y }`。
                            TokenKind::LBrace => {
                                return self.parse_struct_pattern(path);
                            }
                            // 元组结构体模式 `Some(x)`。
                            TokenKind::LParen => {
                                return self.parse_tuple_struct_pattern(path);
                            }
                            _ => {}
                        }
                    }
                    // 简单路径在模式中优先视为标识符绑定。
                    if path.is_simple() {
                        PatternKind::Ident(path.as_simple().unwrap().clone())
                    } else {
                        PatternKind::Path(path)
                    }
                }

                // 元组模式 `(a, b, c)`。
                TokenKind::LParen => {
                    return self.parse_tuple_pattern();
                }

                // 切片模式 `[a, b, ..rest]`。
                TokenKind::LBracket => {
                    return self.parse_slice_pattern();
                }

                // 前缀范围模式 `..100` / `..=100`。
                TokenKind::DotDot => {
                    return self.parse_range_pattern();
                }

                _ => {
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

    /// 解析路径。
    pub(super) fn parse_path(&mut self) -> Result<Path> {
        let lo = self.current_span().lo;
        let mut segments = Vec::new();

        loop {
            let Some(token) = self.current() else {
                break;
            };
            let segment = match token.kind {
                TokenKind::Ident => {
                    let span = token.span;
                    self.advance();
                    self.intern_ident(span)
                }
                TokenKind::AsyncKw => {
                    let span = token.span;
                    self.advance();
                    self.intern_named_ident("async", span)
                }
                TokenKind::DefaultKw => {
                    let span = token.span;
                    self.advance();
                    self.intern_named_ident("default", span)
                }
                _ => break,
            };
            segments.push(segment);
            if self.consume(TokenKind::ColonColon).is_none() {
                break;
            }
        }

        if segments.is_empty() {
            return Err(CompileError::ParseError(ParseError::expected_identifier()));
        }

        Ok(Path::new(segments, self.span_at(lo)))
    }

    /// 解析结构体模式 `Point { x, y: y2, .. }`。
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
                // 简写字段 `{ x }` 等价于 `{ x: x }`。
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

    /// 解析元组结构体模式 `Some(x, y)`。
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

    /// 解析元组模式 `(a, b, c)`。
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

    /// 解析切片模式 `[a, b, ..rest]`。
    fn parse_slice_pattern(&mut self) -> Result<PatternKind> {
        self.advance(); // LBracket

        let mut patterns = Vec::new();
        let mut rest_pattern = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBracket).is_some() {
                break;
            }

            if self.consume(TokenKind::DotDot).is_some() {
                // 支持 `..` 或 `..rest`。
                if let Some(token) = self.current() {
                    if matches!(token.kind, TokenKind::Ident) {
                        let span = token.span;
                        self.advance();
                        rest_pattern = Some(Box::new(Pattern::new(
                            PatternKind::Ident(self.intern_ident(span)),
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

    /// 解析前缀范围模式 `..100`。
    /// 也支持闭区间 `..=100`。
    fn parse_range_pattern(&mut self) -> Result<PatternKind> {
        self.expect(TokenKind::DotDot)?;
        let inclusive = self.consume(TokenKind::Eq).is_some();
        let end = self.parse_pattern_primary()?;
        let span = self.current_span();

        Ok(PatternKind::Range(
            Box::new(Pattern::new(PatternKind::Wildcard, span)),
            Box::new(Pattern::new(end, span)),
            if inclusive {
                RangeEnd::Inclusive
            } else {
                RangeEnd::Exclusive
            },
        ))
    }

    /// 期望并解析一个标识符。
    pub(super) fn expect_ident(&mut self) -> Result<Ident> {
        if let Some(token) = self.current() {
            // 允许 `self` 作为特殊标识符被消费。
            let is_ident = matches!(
                token.kind,
                TokenKind::Ident | TokenKind::SelfLowerKw | TokenKind::DefaultKw
            );
            if is_ident {
                let span = token.span;
                let is_default = token.kind == TokenKind::DefaultKw;
                self.advance();
                if is_default {
                    return Ok(self.intern_named_ident("default", span));
                }
                return Ok(self.intern_ident(span));
            }
        }

        Err(CompileError::ParseError(ParseError::expected_identifier()))
    }
}
