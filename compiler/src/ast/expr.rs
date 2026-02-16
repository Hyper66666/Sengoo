//! 表达式

use super::op::{AssignOp, BinOp, UnOp};
use super::param::Param;
use super::{Block, Ident, Literal, MatchArm, Node, Path, Span, Type};

/// 表达式
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// 创建字面量表达式
    pub fn literal(lit: Literal, span: Span) -> Self {
        Self::new(ExprKind::Literal(lit), span)
    }

    /// 创建标识符表达式
    pub fn ident(name: impl Into<String>, span: Span) -> Self {
        Self::new(ExprKind::Ident(Ident::new(name, span)), span)
    }

    /// 创建路径表达式
    pub fn path(path: Path) -> Self {
        let span = path.span();
        Self::new(ExprKind::Path(path), span)
    }

    /// 创建二元运算表达式
    pub fn binary(op: BinOp, left: Expr, right: Expr, span: Span) -> Self {
        Self::new(
            ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        )
    }

    /// 创建一元运算表达式
    pub fn unary(op: UnOp, operand: Expr, span: Span) -> Self {
        Self::new(
            ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            span,
        )
    }

    /// 创建函数调用表达式
    pub fn call(func: Expr, args: Vec<Expr>, span: Span) -> Self {
        Self::new(
            ExprKind::Call {
                func: Box::new(func),
                args,
            },
            span,
        )
    }

    /// 创建方法调用表达式
    pub fn method_call(receiver: Expr, method: Ident, args: Vec<Expr>, span: Span) -> Self {
        Self::new(
            ExprKind::MethodCall {
                receiver: Box::new(receiver),
                method,
                args,
            },
            span,
        )
    }

    /// 创建块表达式
    pub fn block(block: Block) -> Self {
        let span = block.span();
        Self::new(ExprKind::Block(block), span)
    }

    /// 创建 if 表达式
    pub fn if_expr(
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
        span: Span,
    ) -> Self {
        Self::new(
            ExprKind::If {
                cond: Box::new(cond),
                then_branch,
                else_branch,
            },
            span,
        )
    }

    /// 创建 while 循环
    pub fn while_loop(cond: Expr, body: Block, span: Span) -> Self {
        Self::new(
            ExprKind::While {
                cond: Box::new(cond),
                body,
            },
            span,
        )
    }

    /// 创建 for 循环
    pub fn for_loop(pattern: super::pattern::Pattern, iter: Expr, body: Block, span: Span) -> Self {
        Self::new(
            ExprKind::For {
                pattern,
                iter: Box::new(iter),
                body,
            },
            span,
        )
    }

    /// 创建 loop 循环
    pub fn loop_expr(body: Block, span: Span) -> Self {
        Self::new(ExprKind::Loop(body), span)
    }

    /// 创建 match 表达式
    pub fn match_expr(scrutinee: Expr, arms: Vec<MatchArm>, span: Span) -> Self {
        Self::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span,
        )
    }

    /// 创建 return 表达式
    pub fn return_expr(value: Option<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Return(value.map(Box::new)), span)
    }

    /// 创建 break 表达式
    pub fn break_expr(value: Option<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Break(value.map(Box::new)), span)
    }

    /// 创建 continue 表达式
    pub fn continue_expr(span: Span) -> Self {
        Self::new(ExprKind::Continue, span)
    }

    /// 创建 yield 表达式
    pub fn yield_expr(value: Option<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Yield(value.map(Box::new)), span)
    }

    /// 创建 await 表达式
    pub fn await_expr(base: Expr, span: Span) -> Self {
        Self::new(ExprKind::Await(Box::new(base)), span)
    }

    /// 创建 async 块
    pub fn async_block(block: Block, span: Span) -> Self {
        Self::new(ExprKind::AsyncBlock(block), span)
    }

    /// 创建 parallel 块
    pub fn parallel_block(block: Block, span: Span) -> Self {
        Self::new(ExprKind::ParallelBlock(block), span)
    }

    /// 创建索引表达式
    pub fn index(base: Expr, index: Expr, span: Span) -> Self {
        Self::new(
            ExprKind::Index {
                base: Box::new(base),
                index: Box::new(index),
            },
            span,
        )
    }

    /// 创建字段访问表达式
    pub fn field(base: Expr, field: Ident, span: Span) -> Self {
        Self::new(
            ExprKind::Field {
                base: Box::new(base),
                field,
            },
            span,
        )
    }

    /// 创建数组表达式
    pub fn array(elements: Vec<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Array(elements), span)
    }

    /// 创建元组表达式
    pub fn tuple(elements: Vec<Expr>, span: Span) -> Self {
        Self::new(ExprKind::Tuple(elements), span)
    }

    /// 创建结构体表达式
    pub fn struct_expr(
        path: Path,
        fields: Vec<super::FieldValue>,
        base: Option<Box<Expr>>,
        span: Span,
    ) -> Self {
        Self::new(ExprKind::Struct { path, fields, base }, span)
    }

    /// 创建赋值表达式
    pub fn assign(target: Expr, value: Expr, span: Span) -> Self {
        Self::new(
            ExprKind::Assign {
                target: Box::new(target),
                value: Box::new(value),
            },
            span,
        )
    }

    /// 创建复合赋值表达式
    pub fn assign_op(op: AssignOp, target: Expr, value: Expr, span: Span) -> Self {
        Self::new(
            ExprKind::AssignOp {
                op,
                target: Box::new(target),
                value: Box::new(value),
            },
            span,
        )
    }

    /// 创建范围表达式
    pub fn range(start: Option<Expr>, end: Option<Expr>, inclusive: bool, span: Span) -> Self {
        Self::new(
            ExprKind::Range {
                start: start.map(Box::new),
                end: end.map(Box::new),
                inclusive,
            },
            span,
        )
    }

    /// 创建 try 表达式
    pub fn try_expr(expr: Expr, span: Span) -> Self {
        Self::new(ExprKind::Try(Box::new(expr)), span)
    }

    /// 创建类型转换表达式
    pub fn cast(expr: Expr, ty: Type, span: Span) -> Self {
        Self::new(
            ExprKind::Cast {
                expr: Box::new(expr),
                ty,
            },
            span,
        )
    }

    /// 创建类型断言表达式 `expr is`
    pub fn is_expr(expr: Expr, ty: Type, span: Span) -> Self {
        Self::new(
            ExprKind::Is {
                expr: Box::new(expr),
                ty,
            },
            span,
        )
    }

    /// 创建括号表达式
    pub fn paren(expr: Expr, span: Span) -> Self {
        Self::new(ExprKind::Paren(Box::new(expr)), span)
    }

    /// 是否是字面量
    pub fn is_literal(&self) -> bool {
        matches!(self.kind, ExprKind::Literal(_))
    }

    /// 是否是标识符
    pub fn is_ident(&self) -> bool {
        matches!(self.kind, ExprKind::Ident(_))
    }

    /// 是否是路径
    pub fn is_path(&self) -> bool {
        matches!(self.kind, ExprKind::Path(_))
    }
}

impl Node for Expr {
    fn span(&self) -> Span {
        self.span
    }
}

/// 表达式类型
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// 字面量
    Literal(Literal),

    /// 标识符
    Ident(Ident),

    /// 路径
    Path(Path),

    /// 二元运算 `a + b`
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// 一元运算 `-a` 或 `!a`
    Unary { op: UnOp, operand: Box<Expr> },

    /// 函数调用 `f(x, y)`
    Call { func: Box<Expr>, args: Vec<Expr> },

    /// 方法调用 `obj.method(x, y)`
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },

    /// 块表达式 `{ stmts; expr }`
    Block(Block),

    /// If 表达式
    If {
        cond: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
    },

    /// While 循环
    While { cond: Box<Expr>, body: Block },

    /// For 循环
    For {
        pattern: super::pattern::Pattern,
        iter: Box<Expr>,
        body: Block,
    },

    /// 无限循环
    Loop(Block),

    /// Match 表达式
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Return `return expr`
    Return(Option<Box<Expr>>),

    /// Break `break expr`
    Break(Option<Box<Expr>>),

    /// Continue
    Continue,

    /// Yield `yield expr`
    Yield(Option<Box<Expr>>),

    /// Await `await expr`
    Await(Box<Expr>),

    /// Async 块 `async { ... }`
    AsyncBlock(Block),

    /// Parallel 块 `parallel { ... }`
    ParallelBlock(Block),

    /// 索引 `arr[index]`
    Index { base: Box<Expr>, index: Box<Expr> },

    /// 字段访问 `obj.field`
    Field { base: Box<Expr>, field: Ident },

    /// 数组 `[a, b, c]`
    Array(Vec<Expr>),

    /// 元组 `(a, b, c)`
    Tuple(Vec<Expr>),

    /// 结构体表达式 `Point { x, y }` 或 `Point { x: 1, y: 2 }`
    Struct {
        path: Path,
        fields: Vec<super::FieldValue>,
        base: Option<Box<Expr>>,
    },

    /// 赋值 `target = value`
    Assign { target: Box<Expr>, value: Box<Expr> },

    /// 复合赋值 `target += value`
    AssignOp {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },

    /// 范围 `start..end` 或 `start..=end`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },

    /// Lambda 闭包 `|args| body`
    Lambda { params: Vec<Ident>, body: Box<Expr> },

    /// Try 表达式 `expr?`
    Try(Box<Expr>),

    /// 类型转换 `expr as Type`
    Cast { expr: Box<Expr>, ty: Type },

    /// 类型断言 `expr is Type`
    Is { expr: Box<Expr>, ty: Type },

    /// 括号 `(expr)`
    Paren(Box<Expr>),
}
