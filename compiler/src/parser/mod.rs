//! 鐠囶厽纭堕崚鍡樼€介崳?(Parser)
//!
//! 鐏?Token 濞翠浇娴嗛幑顫礋 AST閵?

mod derive_expander;
mod decl;
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

/// 鐠囶厽纭堕崚鍡樼€介崳?
pub struct Parser<'source> {
    /// Token 濞?
    tokens: Vec<Token>,
    /// 瑜版挸澧犳担宥囩枂
    pos: usize,
    /// 濠ф劒鍞惍?
    source: &'source str,
    /// 闁挎瑨顕ら崚妤勩€冮敍鍫㈡暏娴滃酣鏁婄拠顖涗划婢跺稄绱?
    errors: Vec<ParseError>,
    /// 閺勵垰鎯侀崷銊︽蒋娴犳儼銆冩潏鎯х础娑撳﹣绗呴弬鍥﹁厬閿涘潟f/while 閺夆€叉閿涘绱濈粋浣诡剾鐟欙絾鐎界紒鎾寸€担鎾崇摟闂堛垽鍣?
    in_condition_context: bool,
    /// Number of virtual > tokens produced when splitting nested generic >> in type argument parsing.
    pending_type_arg_gt: usize,
    interner: SymbolInterner,
}

impl<'source> Parser<'source> {
    /// 閸掓稑缂撴稉鈧稉顏呮煀閻ㄥ嫯顕㈠▔鏇炲瀻閺嬫劕娅?
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

    /// 鐟欙絾鐎藉┃鎰敩閻緤绱濇潻鏂挎礀缁嬪绨?
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

    /// 鐟欙絾鐎界粙瀣碍
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut decls = Vec::new();

        while !self.is_eof() {
            match self.parse_decl() {
                Ok(decl) => decls.push(decl),
                Err(CompileError::ParseError(e)) => {
                    self.errors.push(e.with_span(source_span(self.current_span())));
                    self.recover_to_decl();
                }
                Err(_) => break,
            }
        }

        // 婵″倹鐏夐張澶幮掗弸鎰版晩鐠囶垽绱濇潻鏂挎礀缁楊兛绔存稉顏堟晩鐠?
        if !self.errors.is_empty() {
            return Err(CompileError::ParseError(self.errors.remove(0)));
        }

        Ok(Program { decls })
    }

    /// 濡偓閺屻儲妲搁崥锔芥箒鐟欙絾鐎介柨娆掝嚖
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 閼惧嘲褰囪ぐ鎾冲 token
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// 閼惧嘲褰囨稉瀣╃娑?token
    fn peek(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// 濞戝牐鈧缍嬮崜?token
    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// 濡偓閺屻儱缍嬮崜?token 閺勵垰鎯侀弰顖涘瘹鐎规氨琚崹?
    fn check(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 濡偓閺屻儰绗呮稉鈧稉?token 閺勵垰鎯侀弰顖涘瘹鐎规氨琚崹?
    fn check_peek(&self, kind: TokenKind) -> bool {
        self.peek(1).map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 婵″倹鐏夎ぐ鎾冲 token 閺勵垱瀵氱€规氨琚崹瀣剁礉濞戝牐鈧鐣?
    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    /// 濞戝牐鈧瀵氱€规氨琚崹瀣畱 token閿涘苯鎯侀崚娆掔箲閸ョ偤鏁婄拠?
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

    /// 濡偓閺屻儲妲搁崥锕€鍩屾潏鐐瀮娴犺埖婀亸?
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 閼惧嘲褰囪ぐ鎾冲 token 閻?span
    fn current_span(&self) -> Span {
        self.current().map(|t| t.span).unwrap_or_else(|| {
            self.tokens
                .last()
                .map(|t| Span::new(t.span.hi, t.span.hi))
                .unwrap_or_else(|| Span::new(0, 0))
        })
    }

    /// 閼惧嘲褰囨禒搴ゆ崳婵缍呯純顔煎煂瑜版挸澧犳担宥囩枂閻?span
    /// 娴犲孩瀵氱€规矮缍呯純顔肩磻婵娈?span
    fn span_at(&self, lo: u32) -> Span {
        Span::new(lo, self.current_span().hi)
    }

    /// 閹垹顦查崚棰佺瑓娑撯偓娑擃亜锛愰弰搴ｆ畱瀵偓婵?
    fn recover_to_decl(&mut self) {
        // 鐠哄疇绻?token 閻╂潙鍩岄幍鎯у煂娑撯偓娑擃亜锛愰弰搴ｆ畱瀵偓婵?
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

    /// 閼惧嘲褰囧┃鎰敩閻胶澧栧▓?
    pub fn source_slice(&self, span: Span) -> &'source str {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        if hi <= self.source.len() {
            &self.source[lo..hi]
        } else {
            ""
        }
    }

    /// 娴?token 閻?span 閹绘劕褰囬弽鍥槕缁楋附鏋冮張?
    pub fn extract_ident(&self, span: Span) -> String {
        self.source_slice(span).to_string()
    }

    pub(super) fn intern_ident(&mut self, span: Span) -> crate::ast::Ident {
        let name = self.extract_ident(span);
        let symbol = self.interner.intern(&name);
        crate::ast::Ident::with_symbol(name, symbol, span)
    }
}

/// 鐏?AST Span 鏉烆剚宕叉稉?miette SourceSpan
fn source_span(span: Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}




