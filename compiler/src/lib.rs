//! Sengoo compiler core library.

pub mod ast;
pub mod codegen;
pub mod error;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod runtime;
pub mod symbol;
pub mod typeck;

pub use ast::*;
pub use codegen::{jit::JITCodegen, Codegen};
pub use error::{CompileError, Result};
pub use hir::lower_ast;
pub use lexer::{Keyword, Lexer, LiteralKind, Span, Symbol, Token, TokenKind};
pub use mir::lower_hir;
pub use mir::opt::MirOptLevel;
pub use parser::Parser;
pub use symbol::{SymbolId, SymbolInterner};
pub use typeck::TypeChecker;

/// Compiler version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sengoo language version.
pub const LANGUAGE_VERSION: &str = "0.1.0";

/// Shared compile options for source->IR compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileOptions {
    pub mir_opt_level: MirOptLevel,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            mir_opt_level: MirOptLevel::O2,
        }
    }
}

/// Compile Sengoo source to LLVM IR using explicit options.
pub fn compile_to_ir_with_options(source: &str, options: CompileOptions) -> Result<String> {
    // 1. Parse source code.
    let program = Parser::parse(source)?;

    // 2. Type checking.
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    let type_env = checker.into_env();

    // 3. HIR lowering.
    let hir_module = lower_ast(&program, &type_env);

    // 4. MIR lowering.
    let mut mir_fns = lower_hir(&hir_module.items).map_err(CompileError::MirLower)?;
    drop(hir_module);
    drop(type_env);
    drop(program);

    // 5. MIR optimization pipeline for the selected level.
    let pipeline = mir::opt::pipeline_for_level(options.mir_opt_level);
    pipeline.run(&mut mir_fns);

    // 6. Code generation (MIR -> LLVM IR).
    let mut codegen = Codegen::new();
    codegen.codegen(&mir_fns).map_err(CompileError::Codegen)
}

/// Compile Sengoo source to LLVM IR using default options.
pub fn compile_to_ir(source: &str) -> Result<String> {
    compile_to_ir_with_options(source, CompileOptions::default())
}

/// Compile Sengoo source to MIR functions (without code generation).
pub fn compile_to_mir(source: &str) -> Result<Vec<mir::MirFunction>> {
    // 1. Parse source code.
    let program = Parser::parse(source)?;

    // 2. Type checking.
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    let type_env = checker.into_env();

    // 3. HIR lowering.
    let hir_module = lower_ast(&program, &type_env);

    // 4. MIR lowering.
    let mir_fns = lower_hir(&hir_module.items).map_err(CompileError::MirLower)?;
    drop(hir_module);
    drop(type_env);
    drop(program);
    Ok(mir_fns)
}

#[cfg(test)]
mod tests;
