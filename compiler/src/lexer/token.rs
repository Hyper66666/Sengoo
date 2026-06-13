//! Token 类型定义
//!
//! 定义 Sengoo 词法分析器的 Token 类型和位置信息。

mod keyword;

use logos::Logos;
use std::fmt;

pub use keyword::Keyword;

/// 符号类型（用于 interned 字符串）
pub type Symbol = String;

/// 位置信息
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 起始字节偏移
    pub lo: u32,
    /// 结束字节偏移
    pub hi: u32,
}

impl Span {
    /// 创建一个新的位置
    pub fn new(lo: u32, hi: u32) -> Self {
        Self { lo, hi }
    }

    /// 创建一个空位置
    pub fn dummy() -> Self {
        Self { lo: 0, hi: 0 }
    }

    /// 合并两个位置
    pub fn merge(&self, other: Span) -> Span {
        Span {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    /// 获取长度
    pub fn len(&self) -> u32 {
        self.hi - self.lo
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.lo >= self.hi
    }
}

impl From<(u32, u32)> for Span {
    fn from((lo, hi): (u32, u32)) -> Self {
        Self { lo, hi }
    }
}

impl From<(usize, usize)> for Span {
    fn from((lo, hi): (usize, usize)) -> Self {
        Self {
            lo: lo as u32,
            hi: hi as u32,
        }
    }
}

/// Token 类型
/// 使用 Logos derive 实现词法分析
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"//[^\n]*")] // 单行注释
#[logos(skip r"/\*([^*]|\*[^/])*\*/")] // 多行注释
#[logos(skip r"[ \t\r\f]+")] // 空白字符
#[logos(skip r"\n")] // 换行符
pub enum TokenKind {
    // ===== 关键字（Python 风格）=====
    #[token("def")]
    DefKw, // 函数定义（Python 风格）
    #[token("fn")]
    FnKw, // 函数类型（用于类型表达式，如 fn(A, B) -> C）
    #[token("class")]
    ClassKw,
    #[token("struct")]
    StructKw,
    #[token("enum")]
    EnumKw,
    #[token("impl")]
    ImplKw,
    #[token("trait")]
    TraitKw,
    #[token("type")]
    TypeKw,
    #[token("const")]
    ConstKw,
    #[token("static")]
    StaticKw,
    #[token("let")]
    LetKw,

    #[token("if")]
    IfKw,
    #[token("elif")]
    ElifKw, // Python 风格的 else if
    #[token("else")]
    ElseKw,
    #[token("match")]
    MatchKw,
    #[token("case")]
    CaseKw,
    #[token("default")]
    DefaultKw,
    #[token("for")]
    ForKw,
    #[token("while")]
    WhileKw,
    #[token("loop")]
    LoopKw,
    #[token("break")]
    BreakKw,
    #[token("continue")]
    ContinueKw,

    #[token("return")]
    ReturnKw,
    #[token("yield")]
    YieldKw,
    #[token("await")]
    AwaitKw,

    #[token("async")]
    AsyncKw,
    #[token("parallel")]
    ParallelKw,

    #[token("import")]
    ImportKw,
    #[token("from")]
    FromKw,
    #[token("as")]
    AsKw,
    #[token("export")]
    ExportKw,
    #[token("extern")]
    ExternKw,
    #[token("unsafe")]
    UnsafeKw,

    #[token("try")]
    TryKw,
    #[token("except")]
    ExceptKw, // Python 风格的 catch
    #[token("finally")]
    FinallyKw,
    #[token("raise")]
    RaiseKw, // Python 风格的 throw
    #[token("throw")]
    ThrowKw,

    #[token("pub")]
    PubKw,
    #[token("priv")]
    PrivKw,
    #[token("mut")]
    MutKw,

    #[token("where")]
    WhereKw,
    #[token("requires")]
    RequiresKw,
    #[token("ensures")]
    EnsuresKw,
    #[token("Self")]
    SelfKw,
    #[token("self")]
    SelfLowerKw,

    #[token("true")]
    TrueKw,
    #[token("false")]
    FalseKw,
    #[token("none")]
    NoneKw, // Python 风格的 null
    #[token("null")]
    NullKw, // 兼容 null

    #[token("in")]
    InKw,
    #[token("is")]
    IsKw,
    #[token("not")]
    NotKw, // Python 风格的 !
    #[token("and")]
    AndKw, // Python 风格的 &&
    #[token("or")]
    OrKw, // Python 风格的 ||
    #[token("pass")]
    PassKw, // Python 的空语句

    // ===== 运算符 - 算术 =====
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    // ===== 运算符 - 位运算 =====
    #[token("&")]
    BitAnd,
    #[token("|")]
    BitOr,
    #[token("^")]
    BitXor,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("~")]
    BitNot,

    // ===== 运算符 - 逻辑 =====
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("!")]
    Not,

    // ===== 运算符 - 比较 =====
    #[token("==")]
    Eq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,

    // ===== 运算符 - 赋值 =====
    #[token("=")]
    Assign,
    #[token("+=")]
    AddAssign,
    #[token("-=")]
    SubAssign,
    #[token("*=")]
    MulAssign,
    #[token("/=")]
    DivAssign,
    #[token("%=")]
    ModAssign,
    #[token("&=")]
    BitAndAssign,
    #[token("|=")]
    BitOrAssign,
    #[token("^=")]
    BitXorAssign,
    #[token("<<=")]
    ShlAssign,
    #[token(">>=")]
    ShrAssign,

    // ===== 分隔符 =====
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token("::")]
    ColonColon,
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("...")]
    DotDotDot,

    // ===== 箭头 =====
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    // ===== 其他符号 =====
    #[token("@")]
    At,
    #[token("?")]
    Question,
    #[token("$")]
    Dollar,
    #[token("#")]
    Hash,
    #[token("_", priority = 10)]
    Underscore,

    // ===== 字面量 =====

    // 标识符
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1)]
    Ident,

    // 整数
    #[regex(r"0b[01][01_]*(i8|i16|i32|i64|isize|u8|u16|u32|u64|usize)?", |lex| parse_int_literal(lex.slice()))]
    #[regex(r"0o[0-7][0-7_]*(i8|i16|i32|i64|isize|u8|u16|u32|u64|usize)?", |lex| parse_int_literal(lex.slice()))]
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*(i8|i16|i32|i64|isize|u8|u16|u32|u64|usize)?", |lex| parse_int_literal(lex.slice()))]
    #[regex(r"[0-9][0-9_]*(i8|i16|i32|i64|isize|u8|u16|u32|u64|usize)?", |lex| parse_int_literal(lex.slice()))]
    Int(Option<i64>),

    // 浮点数
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)?(f32|f64)?", |lex| parse_float_literal(lex.slice()))]
    Float(Option<f64>),

    // 字符串
    #[token("\"\"\"", lex_multiline_string)]
    #[regex(r#""[^"\\]*(?:\\.[^"\\]*)*""#, |lex| Some(unescape_string(&lex.slice()[1..lex.slice().len()-1])))]
    String(Option<String>),

    // 原始字符串
    #[regex(r#"r"[^"\\]*(?:\\.[^"\\]*)*""#, |lex| Some(lex.slice()[2..lex.slice().len()-1].to_string()))]
    RawString(Option<String>),

    // 字节串
    #[regex(r#"b"[^"\\]*(?:\\.[^"\\]*)*""#, |lex| { let slice = lex.slice(); Some(slice.as_bytes()[2..slice.len()-1].to_vec()) })]
    Bytes(Option<Vec<u8>>),

    // 字符
    #[regex(r"'[^'\\]*(?:\\.[^'\\]*)*'", |lex| Some(parse_char(&lex.slice()[1..lex.slice().len()-1])))]
    Char(Option<char>),
}

/// 字面量类型
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralKind {
    Int(i64),
    Float(f64),
    String(String),
    RawString(String),
    Bytes(Vec<u8>),
    Char(char),
    Bool(bool),
}

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralKind::Int(n) => write!(f, "{}", n),
            LiteralKind::Float(n) => write!(f, "{}", n),
            LiteralKind::String(s) => write!(f, "\"{}\"", s),
            LiteralKind::RawString(s) => write!(f, "r\"{}\"", s),
            LiteralKind::Bytes(b) => write!(f, "b\"{}\"", String::from_utf8_lossy(b)),
            LiteralKind::Char(c) => write!(f, "'{}'", c),
            LiteralKind::Bool(b) => write!(f, "{}", b),
        }
    }
}

impl TokenKind {
    /// 检查是否为关键字
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::DefKw
                | TokenKind::FnKw
                | TokenKind::ClassKw
                | TokenKind::StructKw
                | TokenKind::EnumKw
                | TokenKind::ImplKw
                | TokenKind::TraitKw
                | TokenKind::TypeKw
                | TokenKind::ConstKw
                | TokenKind::StaticKw
                | TokenKind::LetKw
                | TokenKind::IfKw
                | TokenKind::ElseKw
                | TokenKind::MatchKw
                | TokenKind::CaseKw
                | TokenKind::DefaultKw
                | TokenKind::ForKw
                | TokenKind::WhileKw
                | TokenKind::LoopKw
                | TokenKind::BreakKw
                | TokenKind::ContinueKw
                | TokenKind::ReturnKw
                | TokenKind::YieldKw
                | TokenKind::AwaitKw
                | TokenKind::AsyncKw
                | TokenKind::ParallelKw
                | TokenKind::ImportKw
                | TokenKind::FromKw
                | TokenKind::AsKw
                | TokenKind::ExportKw
                | TokenKind::ExternKw
                | TokenKind::UnsafeKw
                | TokenKind::TryKw
                | TokenKind::ExceptKw
                | TokenKind::FinallyKw
                | TokenKind::RaiseKw
                | TokenKind::ThrowKw
                | TokenKind::PubKw
                | TokenKind::PrivKw
                | TokenKind::WhereKw
                | TokenKind::RequiresKw
                | TokenKind::EnsuresKw
                | TokenKind::SelfKw
                | TokenKind::SelfLowerKw
                | TokenKind::TrueKw
                | TokenKind::FalseKw
                | TokenKind::NullKw
                | TokenKind::NoneKw
                | TokenKind::InKw
                | TokenKind::IsKw
                | TokenKind::PassKw
                | TokenKind::NotKw
                | TokenKind::AndKw
                | TokenKind::OrKw
                | TokenKind::ElifKw
        )
    }

    /// 检查是否为特定关键字
    pub fn matches_keyword(&self, kw: Keyword) -> bool {
        matches!(
            (self, kw),
            (TokenKind::DefKw, Keyword::Def)
                | (TokenKind::FnKw, Keyword::Fn)
                | (TokenKind::ClassKw, Keyword::Class)
                | (TokenKind::StructKw, Keyword::Struct)
                | (TokenKind::EnumKw, Keyword::Enum)
                | (TokenKind::ImplKw, Keyword::Impl)
                | (TokenKind::TraitKw, Keyword::Trait)
                | (TokenKind::TypeKw, Keyword::Type)
                | (TokenKind::ConstKw, Keyword::Const)
                | (TokenKind::StaticKw, Keyword::Static)
                | (TokenKind::LetKw, Keyword::Let)
                | (TokenKind::IfKw, Keyword::If)
                | (TokenKind::ElifKw, Keyword::Elif)
                | (TokenKind::ElseKw, Keyword::Else)
                | (TokenKind::MatchKw, Keyword::Match)
                | (TokenKind::CaseKw, Keyword::Case)
                | (TokenKind::DefaultKw, Keyword::Default)
                | (TokenKind::ForKw, Keyword::For)
                | (TokenKind::WhileKw, Keyword::While)
                | (TokenKind::LoopKw, Keyword::Loop)
                | (TokenKind::BreakKw, Keyword::Break)
                | (TokenKind::ContinueKw, Keyword::Continue)
                | (TokenKind::ReturnKw, Keyword::Return)
                | (TokenKind::YieldKw, Keyword::Yield)
                | (TokenKind::AwaitKw, Keyword::Await)
                | (TokenKind::AsyncKw, Keyword::Async)
                | (TokenKind::ParallelKw, Keyword::Parallel)
                | (TokenKind::ImportKw, Keyword::Import)
                | (TokenKind::FromKw, Keyword::From)
                | (TokenKind::AsKw, Keyword::As)
                | (TokenKind::ExportKw, Keyword::Export)
                | (TokenKind::ExternKw, Keyword::Extern)
                | (TokenKind::UnsafeKw, Keyword::Unsafe)
                | (TokenKind::TryKw, Keyword::Try)
                | (TokenKind::ExceptKw, Keyword::Except)
                | (TokenKind::FinallyKw, Keyword::Finally)
                | (TokenKind::RaiseKw, Keyword::Raise)
                | (TokenKind::ThrowKw, Keyword::Throw)
                | (TokenKind::PubKw, Keyword::Pub)
                | (TokenKind::PrivKw, Keyword::Priv)
                | (TokenKind::WhereKw, Keyword::Where)
                | (TokenKind::RequiresKw, Keyword::Requires)
                | (TokenKind::EnsuresKw, Keyword::Ensures)
                | (TokenKind::SelfKw, Keyword::SelfKw)
                | (TokenKind::SelfLowerKw, Keyword::SelfLower)
                | (TokenKind::TrueKw, Keyword::True)
                | (TokenKind::FalseKw, Keyword::False)
                | (TokenKind::NoneKw, Keyword::None)
                | (TokenKind::InKw, Keyword::In)
                | (TokenKind::IsKw, Keyword::Is)
                | (TokenKind::PassKw, Keyword::Pass)
        )
    }

    /// 检查是否为标识符
    pub fn is_ident(&self) -> bool {
        matches!(self, TokenKind::Ident)
    }

    /// 检查是否为字面量
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            TokenKind::Int(_)
                | TokenKind::Float(_)
                | TokenKind::String(_)
                | TokenKind::RawString(_)
                | TokenKind::Bytes(_)
                | TokenKind::Char(_)
        )
    }

    /// 转换为字面量类型
    pub fn as_literal(&self) -> Option<LiteralKind> {
        match self {
            TokenKind::Int(Some(n)) => Some(LiteralKind::Int(*n)),
            TokenKind::Float(Some(n)) => Some(LiteralKind::Float(*n)),
            TokenKind::String(Some(s)) => Some(LiteralKind::String(s.clone())),
            TokenKind::RawString(Some(s)) => Some(LiteralKind::RawString(s.clone())),
            TokenKind::Bytes(Some(b)) => Some(LiteralKind::Bytes(b.clone())),
            TokenKind::Char(Some(c)) => Some(LiteralKind::Char(*c)),
            _ => None,
        }
    }

    /// 转换为关键字
    pub fn as_keyword(&self) -> Option<Keyword> {
        match self {
            TokenKind::DefKw => Some(Keyword::Def),
            TokenKind::FnKw => Some(Keyword::Fn), // 函数类型
            TokenKind::ClassKw => Some(Keyword::Class),
            TokenKind::StructKw => Some(Keyword::Struct),
            TokenKind::EnumKw => Some(Keyword::Enum),
            TokenKind::ImplKw => Some(Keyword::Impl),
            TokenKind::TraitKw => Some(Keyword::Trait),
            TokenKind::TypeKw => Some(Keyword::Type),
            TokenKind::ConstKw => Some(Keyword::Const),
            TokenKind::StaticKw => Some(Keyword::Static),
            TokenKind::LetKw => Some(Keyword::Let),
            TokenKind::IfKw => Some(Keyword::If),
            TokenKind::ElifKw => Some(Keyword::Elif),
            TokenKind::ElseKw => Some(Keyword::Else),
            TokenKind::MatchKw => Some(Keyword::Match),
            TokenKind::CaseKw => Some(Keyword::Case),
            TokenKind::DefaultKw => Some(Keyword::Default),
            TokenKind::ForKw => Some(Keyword::For),
            TokenKind::WhileKw => Some(Keyword::While),
            TokenKind::LoopKw => Some(Keyword::Loop),
            TokenKind::BreakKw => Some(Keyword::Break),
            TokenKind::ContinueKw => Some(Keyword::Continue),
            TokenKind::ReturnKw => Some(Keyword::Return),
            TokenKind::YieldKw => Some(Keyword::Yield),
            TokenKind::AwaitKw => Some(Keyword::Await),
            TokenKind::AsyncKw => Some(Keyword::Async),
            TokenKind::ParallelKw => Some(Keyword::Parallel),
            TokenKind::ImportKw => Some(Keyword::Import),
            TokenKind::FromKw => Some(Keyword::From),
            TokenKind::AsKw => Some(Keyword::As),
            TokenKind::ExportKw => Some(Keyword::Export),
            TokenKind::ExternKw => Some(Keyword::Extern),
            TokenKind::UnsafeKw => Some(Keyword::Unsafe),
            TokenKind::TryKw => Some(Keyword::Try),
            TokenKind::ExceptKw => Some(Keyword::Except),
            TokenKind::FinallyKw => Some(Keyword::Finally),
            TokenKind::RaiseKw => Some(Keyword::Raise),
            TokenKind::ThrowKw => Some(Keyword::Throw),
            TokenKind::PubKw => Some(Keyword::Pub),
            TokenKind::PrivKw => Some(Keyword::Priv),
            TokenKind::WhereKw => Some(Keyword::Where),
            TokenKind::RequiresKw => Some(Keyword::Requires),
            TokenKind::EnsuresKw => Some(Keyword::Ensures),
            TokenKind::SelfKw => Some(Keyword::SelfKw),
            TokenKind::SelfLowerKw => Some(Keyword::SelfLower),
            TokenKind::TrueKw => Some(Keyword::True),
            TokenKind::FalseKw => Some(Keyword::False),
            TokenKind::NoneKw => Some(Keyword::None), // Python 风格使用 None
            TokenKind::InKw => Some(Keyword::In),
            TokenKind::IsKw => Some(Keyword::Is),
            TokenKind::PassKw => Some(Keyword::Pass),
            TokenKind::NotKw => Some(Keyword::Not),
            TokenKind::AndKw => Some(Keyword::And),
            TokenKind::OrKw => Some(Keyword::Or),
            _ => None,
        }
    }
}

/// Token
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// 创建一个新的 Token
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建一个位置标记的 Token
    pub fn with_span(kind: TokenKind, lo: u32, hi: u32) -> Self {
        Self {
            kind,
            span: Span { lo, hi },
        }
    }

    /// 获取 Token 的长度
    pub fn len(&self) -> u32 {
        self.span.len()
    }

    pub fn is_empty(&self) -> bool {
        self.span.is_empty()
    }

    /// 是否为 EOF（通过检查 span 是否为空来判断）
    pub fn is_eof(&self) -> bool {
        self.span.is_empty()
    }

    /// 是否为关键字
    pub fn is_keyword(&self, kw: Keyword) -> bool {
        self.kind.matches_keyword(kw)
    }

    /// 是否为标识符
    pub fn is_ident(&self) -> bool {
        self.kind.is_ident()
    }

    /// 是否为字面量
    pub fn is_literal(&self) -> bool {
        self.kind.is_literal()
    }
}

/// 处理字符串中的转义序列
fn lex_multiline_string(lex: &mut logos::Lexer<TokenKind>) -> Option<String> {
    let remainder = lex.remainder();
    let end = remainder.find("\"\"\"")?;
    let raw = &remainder[..end];
    lex.bump(end + 3);
    Some(strip_multiline_indent(raw))
}

fn strip_multiline_indent(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.lines().collect();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|c| *c == ' ' || *c == '\t').count())
        .min()
        .unwrap_or(0);
    lines
        .into_iter()
        .map(|line| {
            let byte_idx = line
                .char_indices()
                .nth(indent)
                .map(|(idx, _)| idx)
                .unwrap_or_else(|| line.len());
            line[byte_idx..].to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_float_literal(slice: &str) -> Option<f64> {
    let digits = slice
        .strip_suffix("f32")
        .or_else(|| slice.strip_suffix("f64"))
        .unwrap_or(slice);
    digits.replace('_', "").parse::<f64>().ok()
}

fn parse_int_literal(slice: &str) -> Option<i64> {
    const SUFFIXES: &[&str] = &[
        "isize", "usize", "i64", "i32", "i16", "i8", "u64", "u32", "u16", "u8",
    ];
    let digits = SUFFIXES
        .iter()
        .find_map(|suffix| slice.strip_suffix(suffix))
        .unwrap_or(slice);
    let normalized = digits.replace('_', "");
    if let Some(rest) = normalized.strip_prefix("0b") {
        i64::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = normalized.strip_prefix("0o") {
        i64::from_str_radix(rest, 8).ok()
    } else if let Some(rest) = normalized.strip_prefix("0x") {
        i64::from_str_radix(rest, 16).ok()
    } else {
        normalized.parse::<i64>().ok()
    }
}

fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('0') => result.push('\0'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('x') => {
                    let h1 = chars.next().unwrap_or('0');
                    let h2 = chars.next().unwrap_or('0');
                    let code = u8::from_str_radix(&format!("{}{}", h1, h2), 16).unwrap_or(0);
                    result.push(code as char);
                }
                Some('u') => {
                    if chars.next() == Some('{') {
                        let mut code_str = String::new();
                        while let Some(&c) = chars.peek() {
                            if c == '}' {
                                chars.next();
                                break;
                            }
                            code_str.push(c);
                            chars.next();
                        }
                        if let Ok(code) = u32::from_str_radix(&code_str, 16) {
                            if let Some(c) = char::from_u32(code) {
                                result.push(c);
                            }
                        }
                    }
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// 解析字符字面量
fn parse_char(s: &str) -> char {
    if s.starts_with('\\') {
        match s.chars().nth(1) {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('r') => '\r',
            Some('0') => '\0',
            Some('\\') => '\\',
            Some('\'') => '\'',
            Some('"') => '"',
            _ => s.chars().next().unwrap_or('\0'),
        }
    } else {
        s.chars().next().unwrap_or('\0')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span() {
        let span = Span::new(0, 10);
        assert_eq!(span.lo, 0);
        assert_eq!(span.hi, 10);
        assert_eq!(span.len(), 10);
        assert!(!span.is_empty());
    }

    #[test]
    fn test_span_merge() {
        let a = Span::new(0, 5);
        let b = Span::new(10, 15);
        let merged = a.merge(b);
        assert_eq!(merged.lo, 0);
        assert_eq!(merged.hi, 15);
    }

    #[test]
    fn test_keyword_from_str() {
        assert_eq!(Keyword::lookup("fn"), Some(Keyword::Fn));
        assert_eq!(Keyword::lookup("let"), Some(Keyword::Let));
        assert_eq!(Keyword::lookup("if"), Some(Keyword::If));
        assert_eq!(Keyword::lookup("unknown"), None);
    }

    #[test]
    fn test_keyword_display() {
        assert_eq!(Keyword::Fn.to_string(), "fn");
        assert_eq!(Keyword::Let.to_string(), "let");
        assert_eq!(Keyword::Async.to_string(), "async");
    }

    #[test]
    fn test_unescape_string() {
        assert_eq!(unescape_string(r"hello\nworld"), "hello\nworld");
        assert_eq!(unescape_string(r"hello\tworld"), "hello\tworld");
        assert_eq!(unescape_string(r"hello\\world"), "hello\\world");
    }

    #[test]
    fn test_parse_char() {
        assert_eq!(parse_char(r"\n"), '\n');
        assert_eq!(parse_char(r"\t"), '\t');
        assert_eq!(parse_char("a"), 'a');
    }
}
