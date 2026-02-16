//! HIR 块定义

use super::HIRStmt;

/// HIR 块
#[derive(Debug, Clone)]
pub struct HIRBody {
    pub stmts: Vec<HIRStmt>,
    pub expr: Option<Box<super::HIRExpr>>,
}

impl HIRBody {
    pub fn new() -> Self {
        Self {
            stmts: Vec::new(),
            expr: None,
        }
    }

    /// 创建一个空的块（单元类型）
    pub fn empty() -> Self {
        Self::new()
    }

    /// 创建只有语句的块
    pub fn with_stmts(stmts: Vec<HIRStmt>) -> Self {
        Self { stmts, expr: None }
    }

    /// 创建只有表达式的块
    pub fn with_expr(expr: super::HIRExpr) -> Self {
        Self {
            stmts: Vec::new(),
            expr: Some(Box::new(expr)),
        }
    }

    /// 添加语句
    pub fn add_stmt(&mut self, stmt: HIRStmt) {
        self.stmts.push(stmt);
    }

    /// 设置最终表达式
    pub fn set_expr(&mut self, expr: super::HIRExpr) {
        self.expr = Some(Box::new(expr));
    }

    /// 检查是否为空块
    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty() && self.expr.is_none()
    }
}

impl Default for HIRBody {
    fn default() -> Self {
        Self::new()
    }
}
