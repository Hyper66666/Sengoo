//! 璇硶鍒嗘瀽鍣?(Parser)
//!
//! 灏?Token 娴佽浆鎹负 AST銆?

mod decl;
mod expr;
mod pat;
mod stmt;

use crate::ast::Program;
use crate::error::{CompileError, ParseError};
use crate::lexer::{Lexer, Span, Token, TokenKind};
use crate::symbol::SymbolInterner;
use crate::Result;
use miette::SourceSpan;

/// 璇硶鍒嗘瀽鍣?
pub struct Parser<'source> {
    /// Token 娴?
    tokens: Vec<Token>,
    /// 褰撳墠浣嶇疆
    pos: usize,
    /// 婧愪唬鐮?
    source: &'source str,
    /// 閿欒鍒楄〃锛堢敤浜庨敊璇仮澶嶏級
    errors: Vec<ParseError>,
    /// 鏄惁鍦ㄦ潯浠惰〃杈惧紡涓婁笅鏂囦腑锛坕f/while 鏉′欢锛夛紝绂佹瑙ｆ瀽缁撴瀯浣撳瓧闈㈤噺
    in_condition_context: bool,
    /// Number of virtual > tokens produced when splitting nested generic >> in type argument parsing.
    pending_type_arg_gt: usize,
    interner: SymbolInterner,
}

impl<'source> Parser<'source> {
    /// 鍒涘缓涓€涓柊鐨勮娉曞垎鏋愬櫒
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

    /// 瑙ｆ瀽婧愪唬鐮侊紝杩斿洖绋嬪簭
    pub fn parse(source: &'source str) -> Result<Program> {
        let mut parser = Self::new(source);
        let program = parser.parse_program()?;
        if !parser.errors.is_empty() {
            return Err(CompileError::ParseError(parser.errors.remove(0)));
        }
        Ok(program)
    }

    /// 瑙ｆ瀽绋嬪簭
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

        // 濡傛灉鏈夎В鏋愰敊璇紝杩斿洖绗竴涓敊璇?
        if !self.errors.is_empty() {
            return Err(CompileError::ParseError(self.errors.remove(0)));
        }

        Ok(Program { decls })
    }

    /// 妫€鏌ユ槸鍚︽湁瑙ｆ瀽閿欒
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 鑾峰彇褰撳墠 token
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// 鑾峰彇涓嬩竴涓?token
    fn peek(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// 娑堣€楀綋鍓?token
    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// 妫€鏌ュ綋鍓?token 鏄惁鏄寚瀹氱被鍨?
    fn check(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 妫€鏌ヤ笅涓€涓?token 鏄惁鏄寚瀹氱被鍨?
    fn check_peek(&self, kind: TokenKind) -> bool {
        self.peek(1).map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 濡傛灉褰撳墠 token 鏄寚瀹氱被鍨嬶紝娑堣€楀畠
    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    /// 娑堣€楁寚瀹氱被鍨嬬殑 token锛屽惁鍒欒繑鍥為敊璇?
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

    /// 妫€鏌ユ槸鍚﹀埌杈炬枃浠舵湯灏?
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 鑾峰彇褰撳墠 token 鐨?span
    fn current_span(&self) -> Span {
        self.current().map(|t| t.span).unwrap_or_else(|| {
            self.tokens
                .last()
                .map(|t| Span::new(t.span.hi, t.span.hi))
                .unwrap_or_else(|| Span::new(0, 0))
        })
    }

    /// 鑾峰彇浠庤捣濮嬩綅缃埌褰撳墠浣嶇疆鐨?span
    /// 浠庢寚瀹氫綅缃紑濮嬬殑 span
    fn span_at(&self, lo: u32) -> Span {
        Span::new(lo, self.current_span().hi)
    }

    /// 鎭㈠鍒颁笅涓€涓０鏄庣殑寮€濮?
    fn recover_to_decl(&mut self) {
        // 璺宠繃 token 鐩村埌鎵惧埌涓€涓０鏄庣殑寮€濮?
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

    /// 鑾峰彇婧愪唬鐮佺墖娈?
    pub fn source_slice(&self, span: Span) -> &'source str {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        if hi <= self.source.len() {
            &self.source[lo..hi]
        } else {
            ""
        }
    }

    /// 浠?token 鐨?span 鎻愬彇鏍囪瘑绗︽枃鏈?
    pub fn extract_ident(&self, span: Span) -> String {
        self.source_slice(span).to_string()
    }

    pub(super) fn intern_ident(&mut self, span: Span) -> crate::ast::Ident {
        let name = self.extract_ident(span);
        let symbol = self.interner.intern(&name);
        crate::ast::Ident::with_symbol(name, symbol, span)
    }
}

/// 灏?AST Span 杞崲涓?miette SourceSpan
fn source_span(span: Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
