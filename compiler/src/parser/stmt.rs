//! 璇彞瑙ｆ瀽

use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

impl<'source> Parser<'source> {
    /// 瑙ｆ瀽鍧?
    pub(super) fn parse_block(&mut self) -> Result<Block> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LBrace)?;

        let mut stmts = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            stmts.push(self.parse_stmt()?);
        }

        Ok(Block::new(stmts, self.span_at(lo)))
    }

    /// 瑙ｆ瀽璇彞
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // let 缁戝畾
                TokenKind::LetKw => {
                    self.advance();
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
                    }
                }

                // const 缁戝畾
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

                // 琛ㄨ揪寮忚鍙?
                _ => {
                    let expr = self.parse_expr()?;
                    let has_semi = self.consume(TokenKind::Semicolon).is_some();

                    if has_semi {
                        // 浠ュ垎鍙风粨灏剧殑璇彞涓嶄骇鐢熷€?
                        StmtKind::Expr(Box::new(expr))
                    } else {
                        // 娌℃湁 鍒嗗彿锛岃〃杈惧紡浜х敓鍊?
                        StmtKind::Expr(Box::new(expr))
                    }
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

    /// 瑙ｆ瀽绫诲瀷
    pub(super) fn parse_type(&mut self) -> Result<Type> {
        let lo = self.current_span().lo;
        let kind = self.parse_type_primary()?;

        // Parse and consume generic arguments syntax `Type<...>`.
        // Current type AST does not retain generic args, so this is syntax-only.
        if self.consume(TokenKind::Lt).is_some() {
            let mut depth = 1usize;
            while depth > 0 {
                let token = self.advance().ok_or(crate::error::CompileError::ParseError(
                    crate::error::ParseError::UnexpectedEof,
                ))?;
                match token.kind {
                    TokenKind::Lt => depth += 1,
                    TokenKind::Gt => depth -= 1,
                    TokenKind::Shr => {
                        if depth >= 2 {
                            depth -= 2;
                        } else {
                            depth = 0;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(Type::new(kind, self.span_at(lo)))
    }

    fn parse_type_primary(&mut self) -> Result<TypeKind> {
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // 鎷彿绫诲瀷 `(A, B, C)`
                TokenKind::LParen => {
                    self.advance();
                    let mut types = Vec::new();

                    while !self.is_eof() {
                        if self.consume(TokenKind::RParen).is_some() {
                            break;
                        }

                        types.push(self.parse_type()?);

                        self.consume(TokenKind::Comma);
                    }

                    if types.len() == 1 {
                        // `(Type)` 鍙槸鎷彿绫诲瀷
                        TypeKind::Tuple(types)
                    } else {
                        TypeKind::Tuple(types)
                    }
                }

                // 鏁扮粍绫诲瀷 `[Type; N]` 鍜屽垏鐗囩被鍨?`[Type]`
                TokenKind::LBracket => {
                    self.advance();
                    let elem = self.parse_type()?;

                    if self.consume(TokenKind::Semicolon).is_some() {
                        // 鏁扮粍绫诲瀷
                        let len = if let Some(token) = self.current() {
                            match &token.kind {
                                TokenKind::Int(Some(n)) if *n >= 0 => {
                                    let n = *n as u64;
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
                        // 鍒囩墖绫诲瀷
                        self.expect(TokenKind::RBracket)?;
                        TypeKind::Slice(Box::new(elem))
                    }
                }

                // 鎸囬拡绫诲瀷 `*mut Type` 鎴?`*const Type`
                TokenKind::Star => {
                    self.advance();
                    let is_mut = if self.consume(TokenKind::MutKw).is_some() {
                        true
                    } else if self.consume(TokenKind::ConstKw).is_some() {
                        false
                    } else {
                        false
                    };
                    let base = self.parse_type()?;
                    TypeKind::Ptr {
                        base: Box::new(base),
                        is_mut,
                    }
                }

                // 寮曠敤绫诲瀷 `&mut Type` 鎴?`&Type`
                TokenKind::And => {
                    self.advance();
                    let is_mut = self.consume(TokenKind::MutKw).is_some();
                    let base = self.parse_type()?;
                    TypeKind::Ref {
                        base: Box::new(base),
                        is_mut,
                    }
                }

                // 鍑芥暟绫诲瀷 (Python 椋庢牸浣跨敤 def, 浣嗙被鍨嬭〃绀轰粛浣跨敤 fn)
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

                // Never 绫诲瀷 `!`
                TokenKind::Not => {
                    self.advance();
                    TypeKind::Never
                }

                // Infer 绫诲瀷 `_`
                TokenKind::Underscore => {
                    self.advance();
                    TypeKind::Infer
                }

                // 绠€鍗曡矾寰勭被鍨?
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
