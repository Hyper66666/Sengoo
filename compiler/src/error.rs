//! 编译器错误类型定义
//!
//! 使用 `thiserror` 定义错误，使用 `miette` 提供友好的错误信息。

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// 编译器结果类型
pub type Result<T> = std::result::Result<T, CompileError>;

/// 编译器错误
#[derive(Debug, Diagnostic, Error)]
pub enum CompileError {
    /// 词法错误
    #[error("词法错误: {0}")]
    #[diagnostic(code(lexer::error), help("检查源代码中的非法字符或格式"))]
    LexError(#[from] LexError),

    /// 语法错误
    #[error("语法错误: {0}")]
    #[diagnostic(code(parser::error))]
    ParseError(#[from] ParseError),

    /// 类型错误
    #[error("类型错误: {0}")]
    #[diagnostic(code(typeck::error))]
    TypeError(#[from] TypeError),

    /// 类型检查错误
    #[error("类型检查错误: {0}")]
    #[diagnostic(code(typeck::error))]
    TypeckError(#[from] crate::typeck::TypeckError),

    /// IO 错误
    #[error("IO 错误: {0}")]
    #[diagnostic(code(io::error))]
    IoError(#[from] std::io::Error),

    /// HIR 降低错误
    #[error("HIR 降低错误: {0}")]
    #[diagnostic(code(hir::lower_error))]
    HirLower(String),

    /// MIR 降低错误
    #[error("MIR 降低错误: {0}")]
    #[diagnostic(code(mir::lower_error))]
    MirLower(String),

    /// 代码生成错误
    #[error("代码生成错误: {0}")]
    #[diagnostic(code(codegen::error))]
    Codegen(String),
}

/// 词法错误
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum LexError {
    #[error("非法字符: {0}")]
    #[diagnostic(code(lexer::illegal_char), help("移除或替换此字符"))]
    IllegalChar(char),

    #[error("未闭合的字符串")]
    #[diagnostic(code(lexer::unclosed_string), help("添加闭合的引号"))]
    UnclosedString,

    #[error("未闭合的字节串")]
    #[diagnostic(code(lexer::unclosed_bytes), help("添加闭合的引号"))]
    UnclosedBytes,

    #[error("未闭合的多行注释")]
    #[diagnostic(code(lexer::unclosed_comment), help("添加 */ 来闭合注释"))]
    UnclosedComment,

    #[error("无效的数字字面量: {0}")]
    #[diagnostic(code(lexer::invalid_number))]
    InvalidNumber(String),

    #[error("无效的转义序列: \\{0}")]
    #[diagnostic(code(lexer::invalid_escape))]
    InvalidEscape(char),
}

/// 语法错误
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum ParseError {
    #[error("期望 {expected}, 找到 {found}")]
    #[diagnostic(code(parser::unexpected_token))]
    UnexpectedToken {
        expected: String,
        found: String,
        #[label("此处")]
        span: SourceSpan,
    },

    #[error("未闭合的块")]
    #[diagnostic(code(parser::unclosed_block), help(r#"添加 }} 来闭合块"#))]
    UnclosedBlock(#[label("块开始于此")] SourceSpan),

    #[error("未闭合的括号")]
    #[diagnostic(code(parser::unclosed_paren), help(r#"添加 ) 来闭合括号"#))]
    UnclosedParen(#[label("括号开始于此")] SourceSpan),

    #[error("无效的模式: {0}")]
    #[diagnostic(code(parser::invalid_pattern))]
    InvalidPattern(String),

    #[error("结构体字段名无效: 期望标识符或字符串字段名，实际为 {found}")]
    #[diagnostic(
        code(parser::invalid_struct_field),
        help("字段应写成 `name: expr`、`\"name\": expr` 或简写 `name`")
    )]
    InvalidStructField {
        found: String,
        #[label("无效字段名")]
        span: SourceSpan,
    },

    #[error("结构体字段简写仅支持标识符")]
    #[diagnostic(
        code(parser::invalid_struct_field_shorthand),
        help("将字段改为显式写法，例如 `field: value`")
    )]
    InvalidStructFieldShorthand {
        #[label("此处不能使用字段简写")]
        span: SourceSpan,
    },

    #[error("重复的参数名: {0}")]
    #[diagnostic(code(parser::duplicate_param), help("使用不同的名称"))]
    DuplicateParam(String),

    #[error("意外的表达式结尾")]
    #[diagnostic(code(parser::unexpected_eof))]
    UnexpectedEof,
}

impl ParseError {
    fn invalid_pattern(message: &str) -> Self {
        Self::InvalidPattern(message.to_string())
    }

    pub fn expected_declaration() -> Self {
        Self::invalid_pattern("expected declaration / 需要声明")
    }

    pub fn expected_trait_item() -> Self {
        Self::invalid_pattern("expected trait item / 需要 trait 成员")
    }

    pub fn expected_trait_path_in_impl() -> Self {
        Self::invalid_pattern("expected trait path in impl declaration / impl 声明需要 trait 路径")
    }

    pub fn unexpected_token_in_expression() -> Self {
        Self::invalid_pattern("unexpected token in expression / 表达式中出现非法 token")
    }

    pub fn unexpected_token_in_pattern() -> Self {
        Self::invalid_pattern("unexpected token in pattern / 模式中出现非法 token")
    }

    pub fn expected_identifier() -> Self {
        Self::invalid_pattern("expected identifier / 需要标识符")
    }

    pub fn expected_array_length() -> Self {
        Self::invalid_pattern("expected array length / 需要数组长度")
    }

    pub fn expected_type() -> Self {
        Self::invalid_pattern("expected type / 需要类型")
    }

    pub fn unexpected_range_in_infix() -> Self {
        Self::invalid_pattern("unexpected `..` in infix position / 中缀位置不允许 `..`")
    }

    pub fn unexpected_token_in_infix() -> Self {
        Self::invalid_pattern("unexpected token in infix position / 中缀位置出现非法 token")
    }
}

/// 类型错误
#[derive(Debug, Clone, Diagnostic, Error)]
pub enum TypeError {
    #[error("类型不匹配: 期望 {expected}, 找到 {found}")]
    #[diagnostic(code(typeck::mismatch), help("尝试使用显式类型转换"))]
    Mismatch {
        expected: String,
        found: String,
        #[label("此处")]
        span: SourceSpan,
    },

    #[error("未定义的变量: {name}")]
    #[diagnostic(code(typeck::undefined_var), help("检查变量名是否正确拼写"))]
    UndefinedVar {
        name: String,
        #[label("未定义的变量")]
        _span: SourceSpan,
    },

    #[error("未定义的类型: {0}")]
    #[diagnostic(code(typeck::undefined_type))]
    UndefinedType(String),

    #[error("未定义的方法: {0}")]
    #[diagnostic(code(typeck::undefined_method), help("检查方法名是否正确"))]
    UndefinedMethod(String),

    #[error("参数数量不匹配: 期望 {expected} 个, 找到 {found} 个")]
    #[diagnostic(code(typeck::arg_count))]
    ArgCountMismatch { expected: usize, found: usize },

    #[error("特征未实现: {trait_name}")]
    #[diagnostic(code(typeck::trait_not_implemented))]
    TraitNotImplemented { trait_name: String },
}

impl CompileError {
    /// 创建一个位置标记的词法错误
    pub fn lex(_span: SourceSpan, err: LexError) -> Self {
        // 注意：当前 LexError 不包含位置，未来可以扩展
        CompileError::LexError(err)
    }

    /// 创建一个位置标记的语法错误
    pub fn parse(_span: SourceSpan, err: ParseError) -> Self {
        // ParseError 已经包含 span
        CompileError::ParseError(err)
    }

    /// 创建一个位置标记的类型错误
    pub fn typeck(_span: SourceSpan, err: TypeError) -> Self {
        // TypeError 已经包含 span
        CompileError::TypeError(err)
    }
}
