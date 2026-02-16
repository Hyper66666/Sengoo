//! Compiler error types.

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Compiler result type.
pub type Result<T> = std::result::Result<T, CompileError>;

/// Top-level compiler error.
#[derive(Debug, Diagnostic, Error)]
pub enum CompileError {
    #[error("lex error: {0}")]
    #[diagnostic(code(lexer::error), help("check invalid characters or malformed literals"))]
    LexError(#[from] LexError),

    #[error("parse error: {0}")]
    #[diagnostic(code(parser::error))]
    ParseError(#[from] ParseError),

    #[error("type error: {0}")]
    #[diagnostic(code(typeck::error))]
    TypeError(#[from] TypeError),

    #[error("type check error: {0}")]
    #[diagnostic(code(typeck::error))]
    TypeckError(#[from] crate::typeck::TypeckError),

    #[error("io error: {0}")]
    #[diagnostic(code(io::error))]
    IoError(#[from] std::io::Error),

    #[error("HIR lowering error: {0}")]
    #[diagnostic(code(hir::lower_error))]
    HirLower(String),

    #[error("MIR lowering error: {0}")]
    #[diagnostic(code(mir::lower_error))]
    MirLower(String),

    #[error("codegen error: {0}")]
    #[diagnostic(code(codegen::error))]
    Codegen(String),
}

/// Lexer errors.
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum LexError {
    #[error("illegal character: {0}")]
    #[diagnostic(code(lexer::illegal_char), help("remove or replace this character"))]
    IllegalChar(char),

    #[error("unclosed string literal")]
    #[diagnostic(code(lexer::unclosed_string), help("add closing quote"))]
    UnclosedString,

    #[error("unclosed bytes literal")]
    #[diagnostic(code(lexer::unclosed_bytes), help("add closing quote"))]
    UnclosedBytes,

    #[error("unclosed block comment")]
    #[diagnostic(code(lexer::unclosed_comment), help("add */ to close comment"))]
    UnclosedComment,

    #[error("invalid numeric literal: {0}")]
    #[diagnostic(code(lexer::invalid_number))]
    InvalidNumber(String),

    #[error("invalid escape sequence: \\{0}")]
    #[diagnostic(code(lexer::invalid_escape))]
    InvalidEscape(char),
}

/// Parser errors.
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum ParseError {
    #[error("expected {expected}, found {found}")]
    #[diagnostic(code(parser::unexpected_token))]
    UnexpectedToken {
        expected: String,
        found: String,
        #[label("here")]
        span: SourceSpan,
    },

    #[error("unclosed block")]
    #[diagnostic(code(parser::unclosed_block), help("add }} to close the block"))]
    UnclosedBlock(#[label("block starts here")] SourceSpan),

    #[error("unclosed parenthesis")]
    #[diagnostic(code(parser::unclosed_paren), help("add ) to close parenthesis"))]
    UnclosedParen(#[label("paren starts here")] SourceSpan),

    #[error("invalid pattern: {0}")]
    #[diagnostic(code(parser::invalid_pattern))]
    InvalidPattern(String),

    #[error("invalid struct field: expected identifier or string key, found {found}")]
    #[diagnostic(
        code(parser::invalid_struct_field),
        help("use `name: expr`, `\"name\": expr`, or shorthand `name`")
    )]
    InvalidStructField {
        found: String,
        #[label("invalid field")]
        span: SourceSpan,
    },

    #[error("struct field shorthand supports identifiers only")]
    #[diagnostic(
        code(parser::invalid_struct_field_shorthand),
        help("rewrite as explicit `field: value`")
    )]
    InvalidStructFieldShorthand {
        #[label("shorthand not allowed here")]
        span: SourceSpan,
    },

    #[error("duplicate parameter name: {0}")]
    #[diagnostic(code(parser::duplicate_param), help("use a different parameter name"))]
    DuplicateParam(String),

    #[error("unexpected end of input")]
    #[diagnostic(code(parser::unexpected_eof))]
    UnexpectedEof,
}

impl ParseError {
    fn invalid_pattern(message: &str) -> Self {
        Self::InvalidPattern(message.to_string())
    }

    pub fn expected_declaration() -> Self {
        Self::invalid_pattern("expected declaration")
    }

    pub fn expected_trait_item() -> Self {
        Self::invalid_pattern("expected trait item")
    }

    pub fn expected_trait_path_in_impl() -> Self {
        Self::invalid_pattern("expected trait path in impl declaration")
    }

    pub fn invalid_class_header_form() -> Self {
        Self::invalid_pattern("invalid class header: expected `class Child: Parent { ... }`")
    }

    pub fn class_header_trait_list_not_supported() -> Self {
        Self::invalid_pattern("class header trait list is not supported; use `impl Trait for Type`")
    }

    pub fn unexpected_token_in_expression() -> Self {
        Self::invalid_pattern("unexpected token in expression")
    }

    pub fn unexpected_token_in_pattern() -> Self {
        Self::invalid_pattern("unexpected token in pattern")
    }

    pub fn expected_identifier() -> Self {
        Self::invalid_pattern("expected identifier")
    }

    pub fn expected_array_length() -> Self {
        Self::invalid_pattern("expected array length")
    }

    pub fn expected_type() -> Self {
        Self::invalid_pattern("expected type")
    }

    pub fn unexpected_range_in_infix() -> Self {
        Self::invalid_pattern("unexpected `..` in infix position")
    }

    pub fn unexpected_token_in_infix() -> Self {
        Self::invalid_pattern("unexpected token in infix position")
    }
}

/// User-facing type errors.
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum TypeError {
    #[error("type mismatch: expected {expected}, found {found}")]
    #[diagnostic(code(typeck::mismatch), help("add an explicit conversion if needed"))]
    Mismatch {
        expected: String,
        found: String,
        #[label("here")]
        span: SourceSpan,
    },

    #[error("undefined variable: {name}")]
    #[diagnostic(code(typeck::undefined_var), help("check the variable name"))]
    UndefinedVar {
        name: String,
        #[label("undefined variable")]
        _span: SourceSpan,
    },

    #[error("undefined type: {0}")]
    #[diagnostic(code(typeck::undefined_type))]
    UndefinedType(String),

    #[error("undefined method: {0}")]
    #[diagnostic(code(typeck::undefined_method), help("check method name and receiver type"))]
    UndefinedMethod(String),

    #[error("argument count mismatch: expected {expected}, found {found}")]
    #[diagnostic(code(typeck::arg_count))]
    ArgCountMismatch { expected: usize, found: usize },

    #[error("trait not implemented: {trait_name}")]
    #[diagnostic(code(typeck::trait_not_implemented))]
    TraitNotImplemented { trait_name: String },
}

impl CompileError {
    /// Build lexer error at span.
    pub fn lex(_span: SourceSpan, err: LexError) -> Self {
        CompileError::LexError(err)
    }

    /// Build parser error at span.
    pub fn parse(_span: SourceSpan, err: ParseError) -> Self {
        CompileError::ParseError(err)
    }

    /// Build type error at span.
    pub fn typeck(_span: SourceSpan, err: TypeError) -> Self {
        CompileError::TypeError(err)
    }
}