//! 类型检查器 (Type Checker)
//!
//! 对 AST 进行类型分析和检查

mod check;
mod env;
mod infer;
pub mod r#trait;
pub mod ty;

pub use check::TypeChecker;
pub use env::TypeEnv;
pub use infer::TypeInfer;
pub use r#trait::{
    type_key, FunctionTy, ImplInfo, ImplRegistry, MethodSig, TraitInfo, TraitRegistry,
};
pub use ty::{Ty, TyKind, TypeckError};

/// 类型检查结果（公开）
pub type TypeckResult<T> = crate::Result<T>;

use crate::ast::Program;
use crate::error::CompileError;
use crate::Result;

/// 对整个程序进行类型检查
pub fn typeck(program: &Program) -> Result<Program> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)?;
    Ok(program.clone())
}
