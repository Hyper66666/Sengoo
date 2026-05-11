//! 递归下降 Parser 实现
//!
//! 将 Token 流解析为 AST。

mod decl;
mod derive_expander;
mod expr;
mod macro_expander;
mod pat;
mod stmt;

use crate::ast::Program;
use crate::error::{CompileError, ParseError};
use crate::lexer::{Lexer, Span, Token, TokenKind};
use crate::symbol::SymbolInterner;
use crate::Result;
use miette::SourceSpan;

/// Parser 结构。
pub struct Parser<'source> {
    /// Token 列表。
    tokens: Vec<Token>,
    /// 当前游标位置。
    pos: usize,
    /// 原始源码。
    source: &'source str,
    /// 解析过程中收集的错误。
    errors: Vec<ParseError>,
    /// 是否处于条件上下文，用于限制 if/while 条件里的某些语法。
    in_condition_context: bool,
    /// 解析类型参数时拆分嵌套泛型 `>>` 产生的虚拟 `>` Token 数量。
    pending_type_arg_gt: usize,
    interner: SymbolInterner,
}

impl<'source> Parser<'source> {
    /// 从源码创建解析器。
    pub fn new(source: &'source str) -> Self {
        let tokens = Lexer::tokenize(source);
        Self {
            tokens,
            pos: 0,
            source,
            errors: Vec::new(),
            in_condition_context: false,
            pending_type_arg_gt: 0,
            interner: SymbolInterner::default(),
        }
    }

    /// 解析源码并返回 AST。
    pub fn parse(source: &str) -> Result<Program> {
        let expanded_source = macro_expander::expand_declarative_macros(source)?;
        let expanded_source = derive_expander::expand_derive_macros(&expanded_source)?;
        let mut parser = Parser::new(&expanded_source);
        let program = parser.parse_program()?;
        if !parser.errors.is_empty() {
            return Err(CompileError::ParseError(parser.errors.remove(0)));
        }
        Ok(program)
    }

    /// 解析整个程序。
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut decls = Vec::new();

        while !self.is_eof() {
            match self.parse_decl() {
                Ok(decl) => decls.push(decl),
                Err(CompileError::ParseError(e)) => {
                    self.errors
                        .push(e.with_span(source_span(self.current_span())));
                    self.recover_to_decl();
                }
                Err(_) => break,
            }
        }

        // 若已累计解析错误，优先返回首个错误。
        if !self.errors.is_empty() {
            return Err(CompileError::ParseError(self.errors.remove(0)));
        }

        Ok(Program { decls })
    }

    /// 是否已有解析错误。
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 当前 Token。
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// 向前查看第 n 个 Token。
    fn peek(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// 消费当前 Token。
    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// 检查当前 Token 是否匹配。
    fn check(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 检查下一个 Token 是否匹配。
    fn check_peek(&self, kind: TokenKind) -> bool {
        self.peek(1).map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 若匹配则消费当前 Token。
    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    /// 期望当前 Token 为指定类型，否则报错。
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

    /// 是否到达 Token 流末尾。
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 当前 Token 的 span。
    fn current_span(&self) -> Span {
        self.current().map(|t| t.span).unwrap_or_else(|| {
            self.tokens
                .last()
                .map(|t| Span::new(t.span.hi, t.span.hi))
                .unwrap_or_else(|| Span::new(0, 0))
        })
    }

    /// 以给定起点和当前位置构造 span。
    /// 常用于为已消费前缀补全结束位置。
    fn span_at(&self, lo: u32) -> Span {
        Span::new(lo, self.current_span().hi)
    }

    /// 发生错误后恢复到下一个声明起点。
    fn recover_to_decl(&mut self) {
        // 跳过 token，直到遇到可能的新声明边界。
        while !self.is_eof() {
            let kind = self.current().map(|t| &t.kind);
            match kind {
                Some(TokenKind::DefKw)
                | Some(TokenKind::ExternKw)
                | Some(TokenKind::StructKw)
                | Some(TokenKind::EnumKw)
                | Some(TokenKind::ClassKw)
                | Some(TokenKind::TraitKw)
                | Some(TokenKind::ImplKw)
                | Some(TokenKind::TypeKw)
                | Some(TokenKind::ConstKw)
                | Some(TokenKind::StaticKw)
                | Some(TokenKind::ImportKw)
                | Some(TokenKind::UnsafeKw)
                | Some(TokenKind::Hash) => break,
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

    /// 根据 span 提取源码切片。
    pub fn source_slice(&self, span: Span) -> &'source str {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        if hi <= self.source.len() {
            &self.source[lo..hi]
        } else {
            ""
        }
    }

    /// 根据 span 提取标识符字符串。
    pub fn extract_ident(&self, span: Span) -> String {
        self.source_slice(span).to_string()
    }

    pub(super) fn intern_ident(&mut self, span: Span) -> crate::ast::Ident {
        let name = self.extract_ident(span);
        let symbol = self.interner.intern(&name);
        crate::ast::Ident::with_symbol(name, symbol, span)
    }
}

/// 将 AST 的 Span 转换为 miette::SourceSpan。
fn source_span(span: Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
