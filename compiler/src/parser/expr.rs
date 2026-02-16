//! 表达式解析

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::{Keyword, TokenKind};
use crate::Result;
use miette::SourceSpan;

use super::Parser;

/// 运算符优先级
const PREC_ASSIGN: u8 = 1;
const PREC_OR: u8 = 2;
const PREC_AND: u8 = 3;
const PREC_COMPARE: u8 = 4;
const PREC_BIT_OR: u8 = 5;
const PREC_BIT_XOR: u8 = 6;
const PREC_BIT_AND: u8 = 7;
const PREC_SHIFT: u8 = 8;
const PREC_ADD: u8 = 9;
const PREC_MUL: u8 = 10;
const PREC_UNARY: u8 = 11;
const PREC_CALL: u8 = 12;
const PREC_PRIMARY: u8 = 13;

impl<'source> Parser<'source> {
    /// 解析表达式
    pub(super) fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_expr_prec(0)
    }

    /// 解析简单表达式（不包含结构体/函数调用等后缀操作）
    /// 用于 for 循环迭代器等场景，避免将 `{` 误解析为结构体字面量
    fn parse_simple_expr(&mut self) -> Result<Expr> {
        self.parse_simple_expr_prec(0)
    }

    /// 解析表达式（不包含逗号运算符）
    pub(super) fn parse_expr_no_struct(&mut self) -> Result<Expr> {
        self.parse_simple_expr()
    }

    /// 使用优先级解析简单表达式（不含结构体/调用后缀）
    fn parse_simple_expr_prec(&mut self, precedence: u8) -> Result<Expr> {
        let mut left = self.parse_simple_prefix()?;

        loop {
            let token = self.current().cloned();
            let next_prec = match &token {
                Some(t) => self.get_infix_precedence(&t.kind),
                None => 0,
            };

            if next_prec <= precedence {
                break;
            }

            left = self.parse_infix(left, next_prec)?;
        }

        Ok(left)
    }

    /// 解析简单前缀表达式（不含结构体/调用）
    fn parse_simple_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // 字面量
                TokenKind::Int(Some(n)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Int(*n))
                }
                TokenKind::Float(Some(f)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Float(*f))
                }
                TokenKind::String(Some(s)) => {
                    self.advance();
                    ExprKind::Literal(Literal::String(s.clone()))
                }
                TokenKind::Char(Some(c)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Char(*c))
                }
                TokenKind::TrueKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(true))
                }
                TokenKind::FalseKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(false))
                }
                TokenKind::NullKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Null)
                }

                // 一元运算符
                TokenKind::Not => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Minus => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitAnd => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Ref,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Star => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Deref,
                        operand: Box::new(operand),
                    }
                }

                // 括号表达式
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_simple_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }

                // 数组 `[a, b, c]`
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }

                // 块 `{ ... }`
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }

                // 标识符或路径 - 不解析结构体/调用后缀
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    ExprKind::Path(path)
                }

                // self 关键字（用于 impl 块方法）
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    let name = self.extract_ident(span);
                    self.advance();
                    // 将 self 处理为标识符表达式
                    ExprKind::Ident(Ident::new(name, span))
                }

                _ => {
                    let span = source_span(token.span);
                    return Err(CompileError::ParseError(
                        ParseError::unexpected_token_in_expression(),
                    ));
                }
            },

            None => {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 使用优先级解析表达式
    fn parse_expr_prec(&mut self, precedence: u8) -> Result<Expr> {
        let mut left = self.parse_prefix()?;

        loop {
            let token = self.current().cloned();
            let next_prec = match &token {
                Some(t) => self.get_infix_precedence(&t.kind),
                None => 0,
            };

            if next_prec <= precedence {
                break;
            }

            left = self.parse_infix(left, next_prec)?;
        }

        Ok(left)
    }

    /// 解析前缀表达式
    fn parse_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
                // 字面量
                TokenKind::Int(Some(n)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Int(*n))
                }
                TokenKind::Float(Some(f)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Float(*f))
                }
                TokenKind::String(Some(s)) => {
                    self.advance();
                    ExprKind::Literal(Literal::String(s.clone()))
                }
                TokenKind::Char(Some(c)) => {
                    self.advance();
                    ExprKind::Literal(Literal::Char(*c))
                }
                TokenKind::TrueKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(true))
                }
                TokenKind::FalseKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Bool(false))
                }
                TokenKind::NullKw => {
                    self.advance();
                    ExprKind::Literal(Literal::Null)
                }

                // 一元运算符
                TokenKind::Minus => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Not => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitNot => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::BitNot,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::BitAnd => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Ref,
                        operand: Box::new(operand),
                    }
                }
                TokenKind::Star => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Unary {
                        op: UnOp::Deref,
                        operand: Box::new(operand),
                    }
                }

                // 括号
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }

                // 数组 `[a, b, c]`
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }

                // 块 `{ ... }`
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }

                // if 表达式
                TokenKind::IfKw => {
                    return self.parse_if_expr();
                }

                // while 循环
                TokenKind::WhileKw => {
                    return self.parse_while_expr();
                }

                // for 循环
                TokenKind::ForKw => {
                    return self.parse_for_expr();
                }

                // loop 循环
                TokenKind::LoopKw => {
                    return self.parse_loop_expr();
                }

                // match 表达式
                TokenKind::MatchKw => {
                    return self.parse_match_expr();
                }

                // Lambda 闭包 `|args| expr`
                TokenKind::BitOr => {
                    return self.parse_lambda_expr();
                }

                // return
                TokenKind::ReturnKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Return(value.map(Box::new))
                }

                // break
                TokenKind::BreakKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Break(value.map(Box::new))
                }

                // continue
                TokenKind::ContinueKw => {
                    self.advance();
                    ExprKind::Continue
                }

                // yield
                TokenKind::YieldKw => {
                    self.advance();
                    let value = if self.check_expr() {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    ExprKind::Yield(value.map(Box::new))
                }

                // async 块
                TokenKind::AsyncKw => {
                    return self.parse_async_block();
                }

                // parallel 块
                TokenKind::ParallelKw => {
                    return self.parse_parallel_block();
                }

                // 标识符或路径
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            TokenKind::LBrace => {
                                // 在条件上下文中（if/while），{ 是块体开始，不是结构体字面量
                                if !self.in_condition_context {
                                    return self.parse_struct_expr(path);
                                }
                            }
                            TokenKind::LParen => {
                                return self.parse_call_or_tuple_expr(path);
                            }
                            _ => {}
                        }
                    }
                    ExprKind::Path(path)
                }

                // self 关键字（用于 impl 块方法）
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    let name = self.extract_ident(span);
                    self.advance();
                    // 将 self 处理为标识符表达式
                    ExprKind::Ident(Ident::new(name, span))
                }

                _ => {
                    let span = source_span(token.span);
                    return Err(CompileError::ParseError(
                        ParseError::unexpected_token_in_expression(),
                    ));
                }
            },

            None => {
                return Err(CompileError::ParseError(ParseError::UnexpectedEof));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 解析中缀表达式
    fn parse_infix(&mut self, left: Expr, precedence: u8) -> Result<Expr> {
        let lo = left.span.lo;
        let token = self.advance().unwrap();

        let kind = match &token.kind {
            // 二元运算符
            TokenKind::Plus => ExprKind::Binary {
                op: BinOp::Add,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Minus => ExprKind::Binary {
                op: BinOp::Sub,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Star => ExprKind::Binary {
                op: BinOp::Mul,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Slash => ExprKind::Binary {
                op: BinOp::Div,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Percent => ExprKind::Binary {
                op: BinOp::Mod,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitAnd => ExprKind::Binary {
                op: BinOp::BitAnd,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitOr => ExprKind::Binary {
                op: BinOp::BitOr,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::BitXor => ExprKind::Binary {
                op: BinOp::BitXor,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Shl => ExprKind::Binary {
                op: BinOp::Shl,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Shr => ExprKind::Binary {
                op: BinOp::Shr,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::And => ExprKind::Binary {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Or => ExprKind::Binary {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Eq => ExprKind::Binary {
                op: BinOp::Eq,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::NotEq => ExprKind::Binary {
                op: BinOp::NotEq,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Lt => ExprKind::Binary {
                op: BinOp::Lt,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Le => ExprKind::Binary {
                op: BinOp::Le,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Gt => ExprKind::Binary {
                op: BinOp::Gt,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },
            TokenKind::Ge => ExprKind::Binary {
                op: BinOp::Ge,
                left: Box::new(left),
                right: Box::new(self.parse_expr_prec(precedence)?),
            },

            // 赋值
            TokenKind::Assign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::Assign {
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::AddAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::AddAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::SubAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::SubAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::MulAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::MulAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::DivAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::DivAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }
            TokenKind::ModAssign => {
                let value = self.parse_expr_prec(PREC_ASSIGN - 1)?;
                ExprKind::AssignOp {
                    op: AssignOp::ModAssign,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }

            // 范围
            TokenKind::DotDot => {
                let end = if self.check_range_end() {
                    Some(self.parse_expr_prec(PREC_OR)?)
                } else {
                    None
                };
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: end.map(Box::new),
                    inclusive: false,
                }
            }
            TokenKind::DotDot => {
                let end = if self.check_range_end() {
                    Some(self.parse_expr_prec(PREC_OR)?)
                } else {
                    None
                };
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: end.map(Box::new),
                    inclusive: true,
                }
            }

            // 字段访问 `obj.field` 或方法调用 `obj.method(args)`
            TokenKind::Dot => {
                let field = self.expect_ident()?;

                // 检查是否是方法调用 `obj.method(...)`
                if self.check(TokenKind::LParen) {
                    let mut args = Vec::new();
                    self.advance(); // 消耗 (
                    while !self.is_eof() {
                        if self.consume(TokenKind::RParen).is_some() {
                            break;
                        }
                        args.push(self.parse_expr()?);
                        self.consume(TokenKind::Comma);
                    }

                    ExprKind::MethodCall {
                        receiver: Box::new(left),
                        method: field,
                        args,
                    }
                } else {
                    // 字段访问
                    ExprKind::Field {
                        base: Box::new(left),
                        field,
                    }
                }
            }

            // 索引 `arr[index]`
            TokenKind::LBracket => {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                ExprKind::Index {
                    base: Box::new(left),
                    index: Box::new(index),
                }
            }

            // 方法调用 `obj.method(...)`
            TokenKind::DotDot => {
                // 实际上这是范围，但已经被处理了
                return Err(CompileError::ParseError(
                    ParseError::unexpected_range_in_infix(),
                ));
            }

            // 调用（实际上前缀处理了）
            _ => {
                return Err(CompileError::ParseError(
                    ParseError::unexpected_token_in_infix(),
                ));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 解析数组表达式
    fn parse_array_expr(&mut self) -> Result<Expr> {
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

    /// 解析块表达式
    fn parse_block_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let block = self.parse_block()?;
        Ok(Expr::new(ExprKind::Block(block), self.span_at(lo)))
    }

    /// 解析 if 表达式
    fn parse_if_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::IfKw)?;

        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let cond = self.parse_expr()?;
        self.in_condition_context = prev;
        let then_branch = self.parse_block()?;

        let else_branch = if self.consume(TokenKind::ElseKw).is_some() {
            if self.check(TokenKind::IfKw) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let lo = self.current_span().lo;
                let block = self.parse_block()?;
                Some(Box::new(Expr::new(
                    ExprKind::Block(block),
                    self.span_at(lo),
                )))
            }
        } else {
            None
        };

        Ok(Expr::new(
            ExprKind::If {
                cond: Box::new(cond),
                then_branch,
                else_branch,
            },
            self.span_at(lo),
        ))
    }

    /// 解析 while 循环
    fn parse_while_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::WhileKw)?;

        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let cond = self.parse_expr()?;
        self.in_condition_context = prev;
        let body = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::While {
                cond: Box::new(cond),
                body,
            },
            self.span_at(lo),
        ))
    }

    /// 解析 for 循环
    fn parse_for_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::ForKw)?;

        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::InKw)?;
        let iter = self.parse_simple_expr()?;
        let body = self.parse_block()?;

        Ok(Expr::new(
            ExprKind::For {
                pattern,
                iter: Box::new(iter),
                body,
            },
            self.span_at(lo),
        ))
    }

    /// 解析 loop 循环
    fn parse_loop_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LoopKw)?;

        let body = self.parse_block()?;

        Ok(Expr::new(ExprKind::Loop(body), self.span_at(lo)))
    }

    /// 解析 match 表达式
    fn parse_match_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::MatchKw)?;

        // In `match <scrutinee> { ... }`, the following `{` always starts
        // arm blocks and must not be parsed as a struct literal.
        let prev = self.in_condition_context;
        self.in_condition_context = true;
        let scrutinee = self.parse_expr()?;
        self.in_condition_context = prev;
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // 模式
            let mut patterns = vec![self.parse_pattern()?];

            // `A | B` 模式
            while self.consume(TokenKind::BitOr).is_some() {
                patterns.push(self.parse_pattern()?);
            }

            // 可选的守卫
            let guard = if self.consume(TokenKind::IfKw).is_some() {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };

            self.expect(TokenKind::FatArrow)?;

            // 表达式
            let body = self.parse_expr()?;

            self.consume(TokenKind::Comma);

            let mut arm = MatchArm::new(patterns, body, self.current_span());
            if let Some(guard) = guard {
                arm = arm.with_guard(*guard);
            }
            arms.push(arm);
        }

        Ok(Expr::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            self.span_at(lo),
        ))
    }

    /// 解析 Lambda 闭包表达式 `|args| body`
    fn parse_lambda_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;

        // 解析参数列表 `|arg1, arg2, ...|`
        let mut params = Vec::new();
        self.expect(TokenKind::BitOr)?;

        while !self.is_eof() {
            if self.consume(TokenKind::BitOr).is_some() {
                break;
            }

            let name = self.expect_ident()?;
            params.push(name);

            self.consume(TokenKind::Comma);
        }

        // 解析闭包体（可以是表达式或块）
        let body = self.parse_expr()?;

        Ok(Expr::new(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            self.span_at(lo),
        ))
    }

    /// 解析 async 块
    fn parse_async_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::AsyncKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::AsyncBlock(block), self.span_at(lo)))
    }

    /// 解析 parallel 块
    fn parse_parallel_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::ParallelKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::ParallelBlock(block), self.span_at(lo)))
    }

    /// 解析调用或元组表达式
    fn parse_call_or_tuple_expr(&mut self, path: Path) -> Result<Expr> {
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

        // 如果只有一个元素且后面有 `.`，则可能是方法调用
        if args.len() == 1 && self.consume(TokenKind::Dot).is_some() {
            let method = self.expect_ident()?;
            return Ok(Expr::new(
                ExprKind::MethodCall {
                    receiver: Box::new(Expr::new(ExprKind::Path(path.clone()), path.span)),
                    method,
                    args: vec![args.into_iter().next().unwrap()],
                },
                self.span_at(lo),
            ));
        }

        Ok(Expr::new(
            ExprKind::Call {
                func: Box::new(Expr::new(ExprKind::Path(path.clone()), path.span)),
                args,
            },
            self.span_at(lo),
        ))
    }

    /// 解析结构体表达式
    fn parse_struct_expr(&mut self, path: Path) -> Result<Expr> {
        let lo = path.span.lo;
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut base = None;

        while !self.is_eof() {
            if self.consume(TokenKind::RBrace).is_some() {
                break;
            }

            // `..base` 形式
            if self.consume(TokenKind::DotDot).is_some() {
                base = Some(Box::new(self.parse_expr()?));
                self.consume(TokenKind::RBrace);
                break;
            }

            let (name, name_span) = if let Some(token) = self.current() {
                match &token.kind {
                    TokenKind::Ident => {
                        let span = token.span;
                        let name = self.extract_ident(span);
                        self.advance();
                        (FieldName::Ident(Ident::new(name, span)), span)
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
                // 简写形式 `{ x }` 等价于 `{ x: x }`
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

    /// 获取中缀运算符的优先级
    fn get_infix_precedence(&self, kind: &TokenKind) -> u8 {
        match kind {
            TokenKind::Assign
            | TokenKind::AddAssign
            | TokenKind::SubAssign
            | TokenKind::MulAssign
            | TokenKind::DivAssign
            | TokenKind::ModAssign => PREC_ASSIGN,

            TokenKind::Or => PREC_OR,
            TokenKind::And => PREC_AND,

            TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::Le
            | TokenKind::Gt
            | TokenKind::Ge => PREC_COMPARE,

            TokenKind::BitOr => PREC_BIT_OR,
            TokenKind::BitXor => PREC_BIT_XOR,
            TokenKind::BitAnd => PREC_BIT_AND,
            TokenKind::Shl | TokenKind::Shr => PREC_SHIFT,

            TokenKind::Plus | TokenKind::Minus => PREC_ADD,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => PREC_MUL,

            TokenKind::Dot | TokenKind::LBracket | TokenKind::LParen => PREC_CALL,

            TokenKind::DotDot => PREC_OR,

            _ => 0,
        }
    }

    /// 检查是否是表达式开始
    fn check_expr(&self) -> bool {
        if let Some(token) = self.current() {
            matches!(
                &token.kind,
                TokenKind::Int(_)
                    | TokenKind::Float(_)
                    | TokenKind::String(_)
                    | TokenKind::Char(_)
                    | TokenKind::TrueKw
                    | TokenKind::FalseKw
                    | TokenKind::NullKw
                    | TokenKind::Ident
                    | TokenKind::LParen
                    | TokenKind::LBrace
                    | TokenKind::LBracket
                    | TokenKind::IfKw
                    | TokenKind::WhileKw
                    | TokenKind::ForKw
                    | TokenKind::LoopKw
                    | TokenKind::MatchKw
                    | TokenKind::ReturnKw
                    | TokenKind::BreakKw
                    | TokenKind::ContinueKw
                    | TokenKind::YieldKw
                    | TokenKind::AsyncKw
                    | TokenKind::ParallelKw
                    | TokenKind::Minus
                    | TokenKind::Not
                    | TokenKind::BitNot
                    | TokenKind::And
                    | TokenKind::Star
            )
        } else {
            false
        }
    }

    /// 检查是否是范围结束
    fn check_range_end(&self) -> bool {
        self.check_expr()
    }
}

/// 将 AST Span 转换为 miette SourceSpan
fn source_span(span: crate::lexer::Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
