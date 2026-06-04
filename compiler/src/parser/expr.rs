//! 表达式解析器，负责将词法token序列解析为表达式AST节点。
mod aggregates;
mod control_flow;

use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;

use super::Parser;

/// 运算符优先级常量定义，用于Pratt解析算法的优先级比较。
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
const PREC_POSTFIX: u8 = 13;

impl<'source> Parser<'source> {
    pub(super) fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_expr_prec(0)
    }

    fn parse_simple_expr(&mut self) -> Result<Expr> {
        self.parse_simple_expr_prec(0)
    }

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

    fn parse_simple_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
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
                TokenKind::AwaitKw => {
                    self.advance();
                    let operand = self.parse_simple_expr_prec(PREC_UNARY)?;
                    ExprKind::Await(Box::new(operand))
                }
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_simple_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    ExprKind::Path(path)
                }
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    self.advance();
                    ExprKind::Ident(self.intern_ident(span))
                }
                _ => {
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

    /// 使用Pratt解析算法解析指定最低优先级的表达式。
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

    /// 解析前缀表达式（字面量、标识符、一元运算符等）。
    fn parse_prefix(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let token = self.current().cloned();

        let kind = match token {
            Some(token) => match &token.kind {
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

                // 解析一元负号表达式。
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
                TokenKind::AwaitKw => {
                    self.advance();
                    let operand = self.parse_expr_prec(PREC_UNARY)?;
                    ExprKind::Await(Box::new(operand))
                }

                // 解析括号表达式或元组表达式。
                TokenKind::LParen => {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    return Ok(Expr::new(ExprKind::Paren(Box::new(expr)), self.span_at(lo)));
                }

                // 解析数组字面量表达式。
                TokenKind::LBracket => {
                    return self.parse_array_expr();
                }

                // 解析返回值表达式（return后的值）。
                TokenKind::LBrace => {
                    return self.parse_block_expr();
                }

                TokenKind::IfKw => {
                    return self.parse_if_expr();
                }

                // 解析while循环表达式。
                TokenKind::WhileKw => {
                    return self.parse_while_expr();
                }

                // 解析for循环表达式。
                TokenKind::ForKw => {
                    return self.parse_for_expr();
                }

                // 解析loop循环表达式。
                TokenKind::LoopKw => {
                    return self.parse_loop_expr();
                }

                TokenKind::MatchKw => {
                    return self.parse_match_expr();
                }

                TokenKind::TryKw => {
                    return self.parse_try_block_expr();
                }

                // 解析lambda表达式（闭包）。
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

                TokenKind::AsyncKw => {
                    return self.parse_async_block();
                }

                TokenKind::ParallelKw => {
                    return self.parse_parallel_block();
                }

                // 解析标识符：变量引用、函数调用或结构体构造。
                TokenKind::Ident => {
                    let path = self.parse_path()?;
                    if let Some(token) = self.current() {
                        match &token.kind {
                            TokenKind::LBrace => {
                                // 检查是否为结构体初始化表达式。
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

                // 解析self关键字表达式。
                TokenKind::SelfLowerKw => {
                    let span = token.span;
                    self.advance();
                    ExprKind::Ident(self.intern_ident(span))
                }

                _ => {
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

    /// 解析中缀表达式（二元运算符、成员访问、函数调用、索引等）。
    fn parse_infix(&mut self, left: Expr, precedence: u8) -> Result<Expr> {
        let lo = left.span.lo;
        let token = self.advance().unwrap();

        let kind = match &token.kind {
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

            // 解析范围运算符（.. 或 ..=）中缀表达式。
            // 解析范围结束表达式（若存在）。
            TokenKind::DotDot => {
                let inclusive = self.consume(TokenKind::Eq).is_some();
                let end = if self.check_range_end() {
                    Some(self.parse_expr_prec(PREC_OR)?)
                } else {
                    None
                };
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: end.map(Box::new),
                    inclusive,
                }
            }

            // 解析成员访问或方法调用（点运算符）。
            TokenKind::Dot => {
                let field = self.expect_ident()?;

                // 检查是否为方法调用（后跟括号）。
                if self.check(TokenKind::LParen) {
                    self.expect(TokenKind::LParen)?;
                    let mut args = Vec::new();
                    // 解析方法调用的参数列表。
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
                    // 否则解析为字段访问表达式。
                    ExprKind::Field {
                        base: Box::new(left),
                        field,
                    }
                }
            }

            // 解析索引表达式（方括号）。
            TokenKind::LBracket => {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                ExprKind::Index {
                    base: Box::new(left),
                    index: Box::new(index),
                }
            }

            TokenKind::Question => ExprKind::Try(Box::new(left)),

            // 未知中缀运算符，返回原表达式。
            _ => {
                return Err(CompileError::ParseError(
                    ParseError::unexpected_token_in_infix(),
                ));
            }
        };

        Ok(Expr::new(kind, self.span_at(lo)))
    }

    /// 获取中缀运算符的优先级，用于Pratt解析。
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

            TokenKind::Question => PREC_POSTFIX,

            TokenKind::DotDot => PREC_OR,

            _ => 0,
        }
    }

    /// 检查当前token是否可以作为表达式的开始。
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
                    | TokenKind::AwaitKw
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

    /// 检查当前token是否可以作为范围结束表达式的开始。
    fn check_range_end(&self) -> bool {
        self.check_expr()
    }
}
