//! 类型检查器 (Type Checker)
//!
//! 对 AST 进行类型分析和检查

mod borrow;
mod check;
mod env;
mod ffi;
mod infer;
pub mod interner;
pub mod r#trait;
pub mod ty;

pub use borrow::{BorrowChecker, BorrowError};
pub use check::TypeChecker;
pub use env::TypeEnv;
pub use infer::TypeInfer;
pub use interner::{InternedTyId, InternedTyKind, TyInterner};
pub use r#trait::{
    type_key, FunctionTy, ImplInfo, ImplRegistry, MethodSig, TraitInfo, TraitRegistry,
};
pub use ty::{Ty, TyKind, TypeckError};

/// 类型检查结果（公开）
pub type TypeckResult<T> = crate::Result<T>;

use crate::ast::Program;
use crate::Result;

/// 对整个程序进行类型检查
pub fn typeck(program: &Program) -> Result<Program> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)?;
    borrow_check(program, checker.env())?;
    Ok(program.clone())
}

/// Run lightweight borrow checking after semantic type checking.
///
/// This pass is intentionally conservative and may reject some patterns that a
/// full NLL-style checker could accept in the future.
pub fn borrow_check(program: &Program, env: &TypeEnv) -> Result<()> {
    let mut checker = BorrowChecker::new(env.clone());
    checker
        .check_program(program)
        .map_err(|errs| crate::error::CompileError::TypeckError(format_borrow_errors(&errs)))
}

fn format_borrow_errors(errors: &[BorrowError]) -> TypeckError {
    if errors.is_empty() {
        return TypeckError::Other("borrow check failed with unknown error".to_string());
    }

    let mut lines = Vec::with_capacity(errors.len());
    for err in errors {
        match err {
            BorrowError::MultipleMutableBorrows {
                var,
                first_span,
                second_span,
            } => lines.push(format!(
                "multiple mutable borrows of `{}` (first {:?}, second {:?})",
                var, first_span, second_span
            )),
            BorrowError::MutableWithOtherBorrows {
                var,
                mutable_span,
                other_span,
            } => lines.push(format!(
                "mutable borrow conflicts with other borrow for `{}` (mutable {:?}, other {:?})",
                var, mutable_span, other_span
            )),
            BorrowError::CannotMoveBorrowed {
                var,
                borrow_span,
                move_span,
            } => lines.push(format!(
                "cannot move borrowed value `{}` (borrow {:?}, move {:?})",
                var, borrow_span, move_span
            )),
        }
    }

    TypeckError::Other(format!("borrow check failed:\n- {}", lines.join("\n- ")))
}
