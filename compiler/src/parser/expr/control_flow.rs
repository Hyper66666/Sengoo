use crate::ast::*;
use crate::lexer::TokenKind;
use crate::Result;

use super::super::Parser;

impl<'source> Parser<'source> {
    /// 解析块表达式（花括号包含的语句序列）。
    pub(super) fn parse_block_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        let block = self.parse_block()?;
        Ok(Expr::new(ExprKind::Block(block), self.span_at(lo)))
    }

    /// 解析if/else条件表达式。
    pub(super) fn parse_if_expr(&mut self) -> Result<Expr> {
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

    /// 解析while条件循环表达式。
    pub(super) fn parse_while_expr(&mut self) -> Result<Expr> {
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

    /// 解析for-in迭代循环表达式。
    pub(super) fn parse_for_expr(&mut self) -> Result<Expr> {
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

    /// 解析loop无限循环表达式。
    pub(super) fn parse_loop_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::LoopKw)?;

        let body = self.parse_block()?;

        Ok(Expr::new(ExprKind::Loop(body), self.span_at(lo)))
    }

    /// 解析match模式匹配表达式。
    pub(super) fn parse_match_expr(&mut self) -> Result<Expr> {
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

            // 解析match arm中可能包含多个用|分隔的模式。
            let mut patterns = vec![self.parse_pattern()?];

            // 解析额外的模式（用|连接）。
            while self.consume(TokenKind::BitOr).is_some() {
                patterns.push(self.parse_pattern()?);
            }

            // 解析可选的if守卫条件。
            let guard = if self.consume(TokenKind::IfKw).is_some() {
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };

            self.expect(TokenKind::FatArrow)?;

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

    /// 解析lambda（闭包）表达式，形如|x, y| body。
    pub(super) fn parse_lambda_expr(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;

        // 解析lambda参数列表。
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

        let body = self.parse_expr()?;

        Ok(Expr::new(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
            },
            self.span_at(lo),
        ))
    }

    /// 解析async异步代码块表达式。
    pub(super) fn parse_async_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::AsyncKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::AsyncBlock(block), self.span_at(lo)))
    }

    /// 解析parallel并行代码块表达式。
    pub(super) fn parse_parallel_block(&mut self) -> Result<Expr> {
        let lo = self.current_span().lo;
        self.expect(TokenKind::ParallelKw)?;

        let block = self.parse_block()?;

        Ok(Expr::new(ExprKind::ParallelBlock(block), self.span_at(lo)))
    }
}
