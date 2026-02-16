//! Token 类型定义
//!
//! 定义 Sengoo 词法分析器的 Token 类型和位置信息。

use logos::Logos;
use std::fmt;

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
    #[token("_", priority = 10)]
    Underscore,

    // ===== 字面量 =====

    // 标识符
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1)]
    Ident,

    // 整数
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    #[regex(r"0x[0-9a-fA-F]+", |lex| i64::from_str_radix(&lex.slice()[2..], 16).ok())]
    Int(Option<i64>),

    // 浮点数
    #[regex(r"[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Float(Option<f64>),

    // 字符串
    #[regex(r#""[^"\\]*(?:\\.[^"\\]*)*""#, |lex| Some(unescape_string(&lex.slice()[1..lex.slice().len()-1])))]
    String(Option<String>),

    // 原始字符串
    #[regex(r#"r"[^"\\]*(?:\\.[^"\\]*)*""#, |lex| Some(lex.slice()[2..lex.slice().len()-1].to_string()))]
    RawString(Option<String>),

    // 字节串
    #[regex(r#"b"[^"\\]*(?:\\.[^"\\]*)*""#, |lex| Some(lex.slice()[2..lex.slice().len()-1].as_bytes().to_vec()))]
    Bytes(Option<Vec<u8>>),

    // 字符
    #[regex(r"'[^'\\]*(?:\\.[^'\\]*)*'", |lex| Some(parse_char(&lex.slice()[1..lex.slice().len()-1])))]
    Char(Option<char>),
}

/// 关键字枚举（Python 风格）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Def, // def 函数名() -> type （Python 风格函数定义）
    Fn,  // fn(A, B) -> C （函数类型语法）
    Class,
    Struct,
    Enum,
    Impl,
    Trait,
    Type,
    Const,
    Static,
    Let,
    If,
    Elif, // Python 风格的 else if
    Else,
    Match,
    Case,
    Default,
    For,
    While,
    Loop,
    Break,
    Continue,
    Return,
    Yield,
    Await,
    Async,
    Parallel,
    Import,
    From,
    As,
    Export,
    Try,
    Except, // Python 风格的 catch
    Finally,
    Raise, // Python 风格的 throw
    Throw,
    Pub,
    Priv,
    Where,
    SelfKw,
    SelfLower,
    True,
    False,
    None, // Python 风格的 null
    In,
    Is,
    NotIn,
    IsNot,
    Pass, // Python 的空语句
    Not,  // Python 风格的 !
    And,  // Python 风格的 &&
    Or,   // Python 风格的 ||
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Keyword::Def => "def",
            Keyword::Fn => "fn",
            Keyword::Class => "class",
            Keyword::Struct => "struct",
            Keyword::Enum => "enum",
            Keyword::Impl => "impl",
            Keyword::Trait => "trait",
            Keyword::Type => "type",
            Keyword::Const => "const",
            Keyword::Static => "static",
            Keyword::Let => "let",
            Keyword::If => "if",
            Keyword::Elif => "elif",
            Keyword::Else => "else",
            Keyword::Match => "match",
            Keyword::Case => "case",
            Keyword::Default => "default",
            Keyword::For => "for",
            Keyword::While => "while",
            Keyword::Loop => "loop",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Return => "return",
            Keyword::Yield => "yield",
            Keyword::Await => "await",
            Keyword::Async => "async",
            Keyword::Parallel => "parallel",
            Keyword::Import => "import",
            Keyword::From => "from",
            Keyword::As => "as",
            Keyword::Export => "export",
            Keyword::Try => "try",
            Keyword::Except => "except",
            Keyword::Finally => "finally",
            Keyword::Raise => "raise",
            Keyword::Throw => "throw",
            Keyword::Pub => "pub",
            Keyword::Priv => "priv",
            Keyword::Where => "where",
            Keyword::SelfKw => "Self",
            Keyword::SelfLower => "self",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::None => "none",
            Keyword::In => "in",
            Keyword::Is => "is",
            Keyword::NotIn => "not in",
            Keyword::IsNot => "is not",
            Keyword::Pass => "pass",
            Keyword::Not => "not",
            Keyword::And => "and",
            Keyword::Or => "or",
        };
        write!(f, "{}", s)
    }
}

impl Keyword {
    /// 从字符串解析关键字
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "def" => Some(Keyword::Def),
            "fn" => Some(Keyword::Fn), // 函数类型
            "class" => Some(Keyword::Class),
            "struct" => Some(Keyword::Struct),
            "enum" => Some(Keyword::Enum),
            "impl" => Some(Keyword::Impl),
            "trait" => Some(Keyword::Trait),
            "type" => Some(Keyword::Type),
            "const" => Some(Keyword::Const),
            "static" => Some(Keyword::Static),
            "let" => Some(Keyword::Let),
            "if" => Some(Keyword::If),
            "elif" => Some(Keyword::Elif),
            "else" => Some(Keyword::Else),
            "match" => Some(Keyword::Match),
            "case" => Some(Keyword::Case),
            "default" => Some(Keyword::Default),
            "for" => Some(Keyword::For),
            "while" => Some(Keyword::While),
            "loop" => Some(Keyword::Loop),
            "break" => Some(Keyword::Break),
            "continue" => Some(Keyword::Continue),
            "return" => Some(Keyword::Return),
            "yield" => Some(Keyword::Yield),
            "await" => Some(Keyword::Await),
            "async" => Some(Keyword::Async),
            "parallel" => Some(Keyword::Parallel),
            "import" => Some(Keyword::Import),
            "from" => Some(Keyword::From),
            "as" => Some(Keyword::As),
            "export" => Some(Keyword::Export),
            "try" => Some(Keyword::Try),
            "except" => Some(Keyword::Except),
            "finally" => Some(Keyword::Finally),
            "raise" => Some(Keyword::Raise),
            "throw" => Some(Keyword::Throw),
            "pub" => Some(Keyword::Pub),
            "priv" => Some(Keyword::Priv),
            "where" => Some(Keyword::Where),
            "Self" => Some(Keyword::SelfKw),
            "self" => Some(Keyword::SelfLower),
            "true" => Some(Keyword::True),
            "false" => Some(Keyword::False),
            "none" => Some(Keyword::None),
            "null" => Some(Keyword::None), // 兼容
            "in" => Some(Keyword::In),
            "is" => Some(Keyword::Is),
            "pass" => Some(Keyword::Pass),
            "not" => Some(Keyword::Not),
            "and" => Some(Keyword::And),
            "or" => Some(Keyword::Or),
            _ => None,
        }
    }
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
                | TokenKind::TryKw
                | TokenKind::ExceptKw
                | TokenKind::FinallyKw
                | TokenKind::RaiseKw
                | TokenKind::ThrowKw
                | TokenKind::PubKw
                | TokenKind::PrivKw
                | TokenKind::WhereKw
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
        match (self, kw) {
            (TokenKind::DefKw, Keyword::Def) => true,
            (TokenKind::FnKw, Keyword::Fn) => true,
            (TokenKind::ClassKw, Keyword::Class) => true,
            (TokenKind::StructKw, Keyword::Struct) => true,
            (TokenKind::EnumKw, Keyword::Enum) => true,
            (TokenKind::ImplKw, Keyword::Impl) => true,
            (TokenKind::TraitKw, Keyword::Trait) => true,
            (TokenKind::TypeKw, Keyword::Type) => true,
            (TokenKind::ConstKw, Keyword::Const) => true,
            (TokenKind::StaticKw, Keyword::Static) => true,
            (TokenKind::LetKw, Keyword::Let) => true,
            (TokenKind::IfKw, Keyword::If) => true,
            (TokenKind::ElifKw, Keyword::Elif) => true,
            (TokenKind::ElseKw, Keyword::Else) => true,
            (TokenKind::MatchKw, Keyword::Match) => true,
            (TokenKind::CaseKw, Keyword::Case) => true,
            (TokenKind::DefaultKw, Keyword::Default) => true,
            (TokenKind::ForKw, Keyword::For) => true,
            (TokenKind::WhileKw, Keyword::While) => true,
            (TokenKind::LoopKw, Keyword::Loop) => true,
            (TokenKind::BreakKw, Keyword::Break) => true,
            (TokenKind::ContinueKw, Keyword::Continue) => true,
            (TokenKind::ReturnKw, Keyword::Return) => true,
            (TokenKind::YieldKw, Keyword::Yield) => true,
            (TokenKind::AwaitKw, Keyword::Await) => true,
            (TokenKind::AsyncKw, Keyword::Async) => true,
            (TokenKind::ParallelKw, Keyword::Parallel) => true,
            (TokenKind::ImportKw, Keyword::Import) => true,
            (TokenKind::FromKw, Keyword::From) => true,
            (TokenKind::AsKw, Keyword::As) => true,
            (TokenKind::ExportKw, Keyword::Export) => true,
            (TokenKind::TryKw, Keyword::Try) => true,
            (TokenKind::ExceptKw, Keyword::Except) => true,
            (TokenKind::FinallyKw, Keyword::Finally) => true,
            (TokenKind::RaiseKw, Keyword::Raise) => true,
            (TokenKind::ThrowKw, Keyword::Throw) => true,
            (TokenKind::PubKw, Keyword::Pub) => true,
            (TokenKind::PrivKw, Keyword::Priv) => true,
            (TokenKind::WhereKw, Keyword::Where) => true,
            (TokenKind::SelfKw, Keyword::SelfKw) => true,
            (TokenKind::SelfLowerKw, Keyword::SelfLower) => true,
            (TokenKind::TrueKw, Keyword::True) => true,
            (TokenKind::FalseKw, Keyword::False) => true,
            (TokenKind::NoneKw, Keyword::None) => true,
            (TokenKind::InKw, Keyword::In) => true,
            (TokenKind::IsKw, Keyword::Is) => true,
            (TokenKind::PassKw, Keyword::Pass) => true,
            _ => false,
        }
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
            TokenKind::TryKw => Some(Keyword::Try),
            TokenKind::ExceptKw => Some(Keyword::Except),
            TokenKind::FinallyKw => Some(Keyword::Finally),
            TokenKind::RaiseKw => Some(Keyword::Raise),
            TokenKind::ThrowKw => Some(Keyword::Throw),
            TokenKind::PubKw => Some(Keyword::Pub),
            TokenKind::PrivKw => Some(Keyword::Priv),
            TokenKind::WhereKw => Some(Keyword::Where),
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
        assert_eq!(Keyword::from_str("fn"), Some(Keyword::Fn));
        assert_eq!(Keyword::from_str("let"), Some(Keyword::Let));
        assert_eq!(Keyword::from_str("if"), Some(Keyword::If));
        assert_eq!(Keyword::from_str("unknown"), None);
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
