//! Sengoo compiler core library.

pub mod ast;
pub mod codegen;
pub mod error;
pub mod hir;
pub mod lexer;
mod method_resolution;
pub mod mir;
pub mod parser;
pub mod runtime;
pub mod symbol;
pub(crate) mod type_naming;
pub mod typeck;

pub use ast::*;
pub use codegen::{jit::JITCodegen, Codegen};
pub use error::{CompileError, Result};
pub use hir::lower_ast;
pub use lexer::{Keyword, Lexer, LiteralKind, Span, Symbol, Token, TokenKind};
pub use mir::opt::MirOptLevel;
pub use mir::{lower_hir, lower_hir_with_options, MirLowerOptions};
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
    pub runtime_contract_checks: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            mir_opt_level: MirOptLevel::O2,
            runtime_contract_checks: false,
        }
    }
}

fn collect_ffi_codegen_config(hir_module: &hir::Module) -> codegen::FfiCodegenConfig {
    let mut config = codegen::FfiCodegenConfig::default();

    for item in &hir_module.items {
        match item {
            hir::HIRItem::ExternBlock(block) => {
                for extern_item in &block.items {
                    if let hir::HIRExternItem::Function(func) = extern_item {
                        let params = func
                            .params
                            .iter()
                            .map(|p| p.ty.clone().into())
                            .collect::<Vec<mir::MIRType>>();
                        let ret = func.return_type.clone().into();
                        config.extern_decls.push(codegen::ExternDecl {
                            name: func.name.clone(),
                            abi: block.abi.clone(),
                            link_name: block.link_name.clone(),
                            params,
                            ret,
                        });
                    }
                }
            }
            hir::HIRItem::Function(func) => {
                if func.abi.as_deref() == Some("C")
                    && (func.no_mangle || func.export_name.is_some())
                {
                    let export_name = func
                        .export_name
                        .clone()
                        .unwrap_or_else(|| func.name.clone());
                    config.export_symbols.push(codegen::ExportSymbol {
                        internal_name: func.name.clone(),
                        export_name,
                    });
                }
            }
            _ => {}
        }
    }

    config
}

/// Compile Sengoo source to LLVM IR using explicit options.
pub fn compile_to_ir_with_options(source: &str, options: CompileOptions) -> Result<String> {
    // 1. Parse source code.
    let program = Parser::parse(source)?;

    // 2. Type checking.
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    let async_functions = checker.async_function_names().clone();
    let type_env = checker.into_env();

    // 3. HIR lowering.
    let hir_module = lower_ast(&program, &type_env);
    let ffi_codegen = collect_ffi_codegen_config(&hir_module);

    // 4. MIR lowering.
    let mut mir_fns = lower_hir_with_options(
        &hir_module.items,
        MirLowerOptions {
            runtime_contract_checks: options.runtime_contract_checks,
            lazy_generic_mono: true,
            async_functions: async_functions.clone(),
        },
    )
    .map_err(CompileError::MirLower)?;
    drop(hir_module);
    drop(type_env);
    drop(program);

    // 4b. Expand async functions into frame-backed helpers.
    if !async_functions.is_empty() {
        let async_helpers = mir::async_lowering::expand_async_functions(&mut mir_fns)?;
        mir_fns.extend(async_helpers);
    }

    // 5. MIR optimization pipeline for the selected level.
    let pipeline = mir::opt::pipeline_for_level(options.mir_opt_level);
    pipeline.run(&mut mir_fns);

    // 6. Code generation (MIR -> LLVM IR).
    let mut codegen = Codegen::with_ffi(ffi_codegen);
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
    let async_functions = checker.async_function_names().clone();
    let type_env = checker.into_env();

    // 3. HIR lowering.
    let hir_module = lower_ast(&program, &type_env);

    // 4. MIR lowering.
    let mut mir_fns = lower_hir_with_options(
        &hir_module.items,
        mir::MirLowerOptions {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: async_functions.clone(),
        },
    )
    .map_err(CompileError::MirLower)?;
    drop(hir_module);
    drop(type_env);
    drop(program);

    if !async_functions.is_empty() {
        let async_helpers = mir::async_lowering::expand_async_functions(&mut mir_fns)?;
        mir_fns.extend(async_helpers);
    }
    Ok(mir_fns)
}

#[cfg(test)]
mod tests;
