//! 语法分析器 (Parser)
//!
//! 将 Token 流转换为 AST。

mod decl;
mod expr;
mod pat;
mod stmt;

use crate::ast::{Decl, Program};
use crate::error::{CompileError, ParseError};
use crate::lexer::{Lexer, Span, Token, TokenKind};
use crate::Result;
use miette::SourceSpan;

/// 语法分析器
pub struct Parser<'source> {
    /// Token 流
    tokens: Vec<Token>,
    /// 当前位置
    pos: usize,
    /// 源代码
    source: &'source str,
    /// 错误列表（用于错误恢复）
    errors: Vec<ParseError>,
    /// 是否在条件表达式上下文中（if/while 条件），禁止解析结构体字面量
    in_condition_context: bool,
}

impl<'source> Parser<'source> {
    /// 创建一个新的语法分析器
    pub fn new(source: &'source str) -> Self {
        let tokens = Lexer::tokenize(source);
        Self {
            tokens,
            pos: 0,
            source,
            errors: Vec::new(),
            in_condition_context: false,
        }
    }

    /// 解析源代码，返回程序
    pub fn parse(source: &'source str) -> Result<Program> {
        let mut parser = Self::new(source);
        let program = parser.parse_program()?;
        if !parser.errors.is_empty() {
            return Err(CompileError::ParseError(parser.errors.remove(0)));
        }
        Ok(program)
    }

    /// 解析程序
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut decls = Vec::new();

        while !self.is_eof() {
            match self.parse_decl() {
                Ok(decl) => decls.push(decl),
                Err(CompileError::ParseError(e)) => {
                    self.errors.push(e);
                    self.recover_to_decl();
                }
                Err(_) => break,
            }
        }

        // 如果有解析错误，返回第一个错误
        if !self.errors.is_empty() {
            return Err(CompileError::ParseError(self.errors.remove(0)));
        }

        Ok(Program { decls })
    }

    /// 检查是否有解析错误
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 获取当前 token
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// 获取下一个 token
    fn peek(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// 消耗当前 token
    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// 检查当前 token 是否是指定类型
    fn check(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 检查下一个 token 是否是指定类型
    fn check_peek(&self, kind: TokenKind) -> bool {
        self.peek(1).map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 如果当前 token 是指定类型，消耗它
    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    /// 消耗指定类型的 token，否则返回错误
    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        if let Some(token) = self.consume(kind.clone()) {
            Ok(token)
        } else {
            let found = self
                .current()
                .map(|t| format!("{:?}", t.kind))
                .unwrap_or_else(|| "EOF".to_string());
            let expected = format!("{:?}", kind);
            let span = self
                .current()
                .map(|t| source_span(t.span))
                .unwrap_or_else(|| SourceSpan::new(0_usize.into(), 0_usize));
            Err(CompileError::ParseError(ParseError::UnexpectedToken {
                expected,
                found,
                span,
            }))
        }
    }

    /// 检查是否到达文件末尾
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 获取当前 token 的 span
    fn current_span(&self) -> Span {
        self.current().map(|t| t.span).unwrap_or_else(|| {
            self.tokens
                .last()
                .map(|t| Span::new(t.span.hi, t.span.hi))
                .unwrap_or_else(|| Span::new(0, 0))
        })
    }

    /// 获取从起始位置到当前位置的 span
    fn span_from(&self, start: Span) -> Span {
        Span::new(start.lo, self.current_span().hi)
    }

    /// 从指定位置开始的 span
    fn span_at(&self, lo: u32) -> Span {
        Span::new(lo, self.current_span().hi)
    }

    /// 恢复到下一个声明的开始
    fn recover_to_decl(&mut self) {
        // 跳过 token 直到找到一个声明的开始
        while !self.is_eof() {
            let kind = self.current().map(|t| &t.kind);
            match kind {
                Some(TokenKind::DefKw)
                | Some(TokenKind::StructKw)
                | Some(TokenKind::EnumKw)
                | Some(TokenKind::ClassKw)
                | Some(TokenKind::TraitKw)
                | Some(TokenKind::ImplKw)
                | Some(TokenKind::TypeKw)
                | Some(TokenKind::ConstKw)
                | Some(TokenKind::StaticKw)
                | Some(TokenKind::ImportKw) => break,
                Some(TokenKind::Semicolon) => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// 获取源代码片段
    pub fn source_slice(&self, span: Span) -> &'source str {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        if hi <= self.source.len() {
            &self.source[lo..hi]
        } else {
            ""
        }
    }

    /// 从 token 的 span 提取标识符文本
    pub fn extract_ident(&self, span: Span) -> String {
        self.source_slice(span).to_string()
    }
}

/// 将 AST Span 转换为 miette SourceSpan
fn source_span(span: Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
