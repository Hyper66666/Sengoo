//! 濡€崇础鐟欙絾鐎?

use crate::ast::pattern::{Pattern, PatternKind, RangeEnd, StructPatternField};
use crate::ast::{Ident, Literal, Path};
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

impl<'source> Parser<'source> {
    /// 鐟欙絾鐎藉Ο鈥崇础
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

        // 婢跺嫮鎮婇幋鏍佸?`A | B`
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
                // 闁岸鍘ょ粭?`_`
                TokenKind::Underscore => {
                    self.advance();
                    PatternKind::Wildcard
                }

                // 鐎涙娼伴柌?
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

                // 閺嶅洩鐦戠粭锔藉灗鐠侯垰绶?
                TokenKind::Ident => {
                    // 鐏忔繆鐦憴锝嗙€界捄顖氱窞
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            // 缂佹挻鐎担鎾茨佸?`Point { x, y }`
                            TokenKind::LBrace => {
                                return self.parse_struct_pattern(path);
                            }
                            // 閸忓啰绮嶇紒鎾寸€担鎾茨佸?`Some(x)`
                            TokenKind::LParen => {
                                return self.parse_tuple_struct_pattern(path);
                            }
                            _ => {}
                        }
                    }
                    // 缁犫偓閸楁洘鐖ｇ拠鍡欘儊
                    if path.is_simple() {
                        PatternKind::Ident(path.as_simple().unwrap().clone())
                    } else {
                        PatternKind::Path(path)
                    }
                }

                // 閸忓啰绮嶅Ο鈥崇础 `(a, b, c)`
                TokenKind::LParen => {
                    return self.parse_tuple_pattern();
                }

                // 閸掑洨澧栧Ο鈥崇础 `[a, b, ..rest]`
                TokenKind::LBracket => {
                    return self.parse_slice_pattern();
                }

                // 閼煎啫娲垮Ο鈥崇础 `1..=100`
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

    /// 鐟欙絾鐎界捄顖氱窞
    pub(super) fn parse_path(&mut self) -> Result<Path> {
        let lo = self.current_span().lo;
        let mut segments = Vec::new();

        loop {
            if let Some(token) = self.current() {
                if matches!(token.kind, TokenKind::Ident) {
                    let span = token.span;
                    self.advance();
                    segments.push(self.intern_ident(span));

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

    /// 鐟欙絾鐎界紒鎾寸€担鎾茨佸?`Point { x, y: y2, .. }`
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
                // 缁犫偓閸愭瑥鑸板?`{ x }` 缁涘鐜禍?`{ x: x }`
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

    /// 鐟欙絾鐎介崗鍐矋缂佹挻鐎担鎾茨佸?`Some(x, y)`
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

    /// 鐟欙絾鐎介崗鍐矋濡€崇础 `(a, b, c)`
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

    /// 鐟欙絾鐎介崚鍥╁濡€崇础 `[a, b, ..rest]`
    fn parse_slice_pattern(&mut self) -> Result<PatternKind> {
        self.advance(); // LBracket

        let mut patterns = Vec::new();
        let mut rest_pattern = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBracket).is_some() {
                break;
            }

            if self.consume(TokenKind::DotDot).is_some() {
                // `..` 閹?`..rest`
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

    /// 鐟欙絾鐎介懠鍐ㄦ纯濡€崇础 `1..100`
    /// 瑙ｆ瀽鑼冨洿妯″紡 `..100` 鎴?`..=100`
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

    /// 閺堢喐婀滈弽鍥槕缁?
    pub(super) fn expect_ident(&mut self) -> Result<Ident> {
        if let Some(token) = self.current() {
            // 閸氬本妞傞幒銉ュ綀閺咁噣鈧碍鐖ｇ拠鍡欘儊閸?self 閸忔娊鏁€?
            let is_ident = token.kind == TokenKind::Ident || token.kind == TokenKind::SelfLowerKw;
            if is_ident {
                let span = token.span;
                self.advance();
                return Ok(self.intern_ident(span));
            }
        }

        Err(CompileError::ParseError(ParseError::expected_identifier()))
    }
}
