//! 语句解析

use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

impl<'source> Parser<'source> {
    /// 解析块
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

    /// 解析语句
    pub(super) fn parse_stmt(&mut self) -> Result<Stmt> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // let 绑定
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

                // const 绑定
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

                // 表达式语句
                _ => {
                    let expr = self.parse_expr()?;
                    let has_semi = self.consume(TokenKind::Semicolon).is_some();

                    if has_semi {
                        // 以分号结尾的语句不产生值
                        StmtKind::Expr(Box::new(expr))
                    } else {
                        // 没有 分号，表达式产生值
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

    /// 解析类型
    pub(super) fn parse_type(&mut self) -> Result<Type> {
        let lo = self.current_span().lo;
        let kind = self.parse_type_primary()?;

        // 处理泛型参数 `Vec<T>`
        // TODO: 实现泛型参数

        Ok(Type::new(kind, self.span_at(lo)))
    }

    fn parse_type_primary(&mut self) -> Result<TypeKind> {
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // 括号类型 `(A, B, C)`
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
                        // `(Type)` 只是括号类型
                        TypeKind::Tuple(types)
                    } else {
                        TypeKind::Tuple(types)
                    }
                }

                // 数组类型 `[Type; N]` 和切片类型 `[Type]`
                TokenKind::LBracket => {
                    self.advance();
                    let elem = self.parse_type()?;

                    if self.consume(TokenKind::Semicolon).is_some() {
                        // 数组类型
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
                        // 切片类型
                        self.expect(TokenKind::RBracket)?;
                        TypeKind::Slice(Box::new(elem))
                    }
                }

                // 指针类型 `*mut Type` 或 `*const Type`
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

                // 引用类型 `&mut Type` 或 `&Type`
                TokenKind::And => {
                    self.advance();
                    let is_mut = self.consume(TokenKind::MutKw).is_some();
                    let base = self.parse_type()?;
                    TypeKind::Ref {
                        base: Box::new(base),
                        is_mut,
                    }
                }

                // 函数类型 (Python 风格使用 def, 但类型表示仍使用 fn)
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

                // Never 类型 `!`
                TokenKind::Not => {
                    self.advance();
                    TypeKind::Never
                }

                // Infer 类型 `_`
                TokenKind::Underscore => {
                    self.advance();
                    TypeKind::Infer
                }

                // 简单路径类型
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
