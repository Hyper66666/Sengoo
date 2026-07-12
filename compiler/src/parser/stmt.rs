use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;
use crate::Span;

use super::Parser;

impl<'source> Parser<'source> {
    pub(super) fn parse_block(&mut self) -> Result<Block> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        let mut closed_hi: Option<u32> = None;

        while !self.is_eof() {
            if let Some(rbrace) = self.consume(TokenKind::RBrace) {
                closed_hi = Some(rbrace.span.hi);
                break;
            }

            stmts.push(self.parse_stmt()?);
        }

        let hi = closed_hi.unwrap_or_else(|| self.current_span().hi);
        Ok(Block::new(stmts, Span::new(lo, hi)))
    }

    pub(super) fn parse_stmt(&mut self) -> Result<Stmt> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                TokenKind::LetKw => {
                    self.advance();
                    let is_mut = self.consume(TokenKind::MutKw).is_some();
                    let name = self.expect_ident()?;

                    let ty = if self.consume(TokenKind::Colon).is_some() {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };

                    let value = if self.consume(TokenKind::Assign).is_some() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };

                    self.consume(TokenKind::Semicolon);

                    StmtKind::Let {
                        name,
                        ty,
                        value: value.map(Box::new),
                        is_mut,
                    }
                }
                TokenKind::ConstKw => {
                    self.advance();
                    let name = self.expect_ident()?;

                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type()?;

                    self.expect(TokenKind::Assign)?;
                    let value = self.parse_expr()?;

                    self.consume(TokenKind::Semicolon);

                    StmtKind::Const {
                        name,
                        ty,
                        value: Box::new(value),
                    }
                }
                _ => {
                    let expr = self.parse_expr()?;
                    self.consume(TokenKind::Semicolon);
                    StmtKind::Expr(Box::new(expr))
                }
            },
            None => {
                return Err(crate::error::CompileError::ParseError(
                    crate::error::ParseError::UnexpectedEof,
                ));
            }
        };

        Ok(Stmt::new(kind, self.span_at(lo)))
    }

    pub(super) fn parse_type(&mut self) -> Result<Type> {
        let lo = self.current_span().lo;
        let mut kind = self.parse_type_primary()?;

        if self.consume(TokenKind::Lt).is_some() {
            match kind {
                TypeKind::Path(path) => {
                    let args = self.parse_type_args()?;
                    kind = TypeKind::PathWithArgs { path, args };
                }
                _ => {
                    return Err(crate::error::CompileError::ParseError(
                        crate::error::ParseError::expected_type(),
                    ));
                }
            }
        }

        Ok(Type::new(kind, self.span_at(lo)))
    }

    fn parse_type_args(&mut self) -> Result<Vec<Type>> {
        let mut args = Vec::new();

        if self.consume_type_arg_end() {
            return Ok(args);
        }

        loop {
            args.push(self.parse_type()?);

            if self.consume(TokenKind::Comma).is_some() {
                continue;
            }

            if self.consume_type_arg_end() {
                break;
            }

            return Err(crate::error::CompileError::ParseError(
                crate::error::ParseError::expected_type(),
            ));
        }

        Ok(args)
    }

    pub(super) fn consume_type_arg_end(&mut self) -> bool {
        if self.pending_type_arg_gt > 0 {
            self.pending_type_arg_gt -= 1;
            return true;
        }

        if self.consume(TokenKind::Gt).is_some() {
            return true;
        }

        if self.consume(TokenKind::Shr).is_some() {
            self.pending_type_arg_gt += 1;
            return true;
        }

        false
    }

    fn parse_type_primary(&mut self) -> Result<TypeKind> {
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                TokenKind::LParen => {
                    self.advance();
                    let mut types = Vec::new();

                    while !self.is_eof() {
                        if self.consume(TokenKind::RParen).is_some() {
                            break;
                        }

                        types.push(self.parse_type()?);

                        if self.consume(TokenKind::Comma).is_some() {
                            continue;
                        }

                        self.expect(TokenKind::RParen)?;
                        break;
                    }

                    TypeKind::Tuple(types)
                }
                TokenKind::LBracket => {
                    self.advance();
                    let elem = self.parse_type()?;

                    if self.consume(TokenKind::Semicolon).is_some() {
                        let len = if let Some(token) = self.current() {
                            match &token.kind {
                                TokenKind::Int(Some(n)) => {
                                    let n = *n;
                                    self.advance();
                                    n
                                }
                                _ => {
                                    return Err(crate::error::CompileError::ParseError(
                                        crate::error::ParseError::expected_array_length(),
                                    ));
                                }
                            }
                        } else {
                            return Err(crate::error::CompileError::ParseError(
                                crate::error::ParseError::UnexpectedEof,
                            ));
                        };

                        self.expect(TokenKind::RBracket)?;
                        TypeKind::Array(Box::new(elem), len)
                    } else {
                        self.expect(TokenKind::RBracket)?;
                        TypeKind::Slice(Box::new(elem))
                    }
                }
                TokenKind::Star => {
                    self.advance();
                    let is_mut = self.consume(TokenKind::MutKw).is_some();
                    self.consume(TokenKind::ConstKw);
                    let base = self.parse_type()?;
                    TypeKind::Ptr {
                        base: Box::new(base),
                        is_mut,
                    }
                }
                TokenKind::BitAnd => {
                    self.advance();
                    let is_mut = self.consume(TokenKind::MutKw).is_some();
                    let base = self.parse_type()?;
                    TypeKind::Ref {
                        base: Box::new(base),
                        is_mut,
                    }
                }
                TokenKind::FnKw => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;

                    let mut params = Vec::new();

                    while !self.is_eof() {
                        if self.consume(TokenKind::RParen).is_some() {
                            break;
                        }

                        params.push(self.parse_type()?);
                        self.consume(TokenKind::Comma);
                    }

                    let ret = if self.consume(TokenKind::Arrow).is_some() {
                        Some(Box::new(self.parse_type()?))
                    } else {
                        None
                    };

                    TypeKind::Fn { params, ret }
                }
                TokenKind::DynKw => {
                    self.advance();
                    TypeKind::Dyn(self.parse_trait_bounds()?)
                }
                TokenKind::Not => {
                    self.advance();
                    TypeKind::Never
                }
                TokenKind::Underscore => {
                    self.advance();
                    TypeKind::Infer
                }
                TokenKind::SelfKw => {
                    if self.check_peek(TokenKind::ColonColon) {
                        TypeKind::Path(self.parse_path()?)
                    } else {
                        self.advance();
                        TypeKind::SelfType
                    }
                }
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    TypeKind::Path(path)
                }
                _ => {
                    return Err(crate::error::CompileError::ParseError(
                        crate::error::ParseError::expected_type(),
                    ));
                }
            },
            None => {
                return Err(crate::error::CompileError::ParseError(
                    crate::error::ParseError::UnexpectedEof,
                ));
            }
        };

        Ok(kind)
    }
}
