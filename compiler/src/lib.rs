//! Sengoo compiler core library.

pub mod ast;
pub mod codegen;
pub mod error;
pub mod format_template;
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
pub use codegen::{
    jit::JITCodegen, Codegen, DebugInfoConfig, FfiCodegenConfig, IntegerOverflowMode,
};
pub use error::{CompileError, CompileWarning, Result};
pub use hir::{lower_ast, lower_ast_with_coverage};
pub use lexer::{Keyword, Lexer, LiteralKind, Span, Symbol, Token, TokenKind};
pub use mir::opt::MirOptLevel;
pub use mir::{
    lower_hir, lower_hir_with_options, AssertCallsiteContext, CoverageContext, MirLowerOptions,
};
pub use parser::Parser;
pub use symbol::{SymbolId, SymbolInterner};
pub use typeck::TypeChecker;

/// Compiler version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sengoo language version.
pub const LANGUAGE_VERSION: &str = "0.1.0";

/// MIR semantic ABI version used by compiler-produced MIR bundles.
pub const MIR_SEMANTIC_ABI_VERSION: u32 = 1;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPointerWidth {
    Bits32,
    Bits64,
}

impl TargetPointerWidth {
    pub const fn bits(self) -> u8 {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }

    pub fn from_target_triple(triple: &str) -> Option<Self> {
        let architecture = triple.split('-').next()?.to_ascii_lowercase();
        if architecture == "x86"
            || architecture.starts_with('i') && architecture.ends_with("86")
            || architecture.starts_with("arm")
            || architecture.starts_with("thumb")
            || architecture == "wasm32"
            || architecture == "riscv32"
            || architecture == "powerpc"
            || architecture.starts_with("mips") && !architecture.contains("64")
        {
            Some(Self::Bits32)
        } else if architecture == "x86_64"
            || architecture == "aarch64"
            || architecture == "wasm64"
            || architecture == "riscv64"
            || architecture == "powerpc64"
            || architecture == "powerpc64le"
            || architecture == "s390x"
            || architecture.contains("mips64")
        {
            Some(Self::Bits64)
        } else {
            None
        }
    }

    pub const fn host() -> Self {
        if usize::BITS == 32 {
            Self::Bits32
        } else {
            Self::Bits64
        }
    }
}

/// Target-aware MIR bundle produced by the compiler frontend.
#[derive(Debug, Clone)]
pub struct MirBundle {
    pub semantic_abi_version: u32,
    pub target_pointer_width: TargetPointerWidth,
    pub functions: Vec<mir::MirFunction>,
    pub ffi_codegen: FfiCodegenConfig,
}

pub fn collect_ffi_codegen_config(hir_module: &hir::Module) -> codegen::FfiCodegenConfig {
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

/// Collect FFI codegen metadata using target pointer-width type mapping.
pub fn collect_ffi_codegen_config_for_pointer_width(
    hir_module: &hir::Module,
    pointer_width: TargetPointerWidth,
) -> codegen::FfiCodegenConfig {
    crate::mir::type_mapping_helpers::with_target_pointer_width(pointer_width.bits(), || {
        collect_ffi_codegen_config(hir_module)
    })
}

/// Collect unique native library names from `#[link(name = "...")]` extern blocks.
pub fn collect_native_link_libraries(hir_module: &hir::Module) -> Vec<String> {
    let mut libraries = Vec::new();
    for item in &hir_module.items {
        let hir::HIRItem::ExternBlock(block) = item else {
            continue;
        };
        let Some(name) = block.link_name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        if libraries.iter().any(|existing| existing == name) {
            continue;
        }
        libraries.push(name.to_string());
    }
    libraries
}

fn target_pointer_width_for_triple(target_triple: &str) -> Result<TargetPointerWidth> {
    TargetPointerWidth::from_target_triple(target_triple).ok_or_else(|| {
        CompileError::Codegen(format!(
            "unsupported target architecture in triple `{target_triple}`"
        ))
    })
}

/// Compile Sengoo source to LLVM IR using explicit options.
pub fn compile_to_ir_with_options(source: &str, options: CompileOptions) -> Result<String> {
    compile_to_ir_inner(source, options, None, TargetPointerWidth::host())
}

/// Compile Sengoo source for an explicit target triple.
///
/// Pointer-sized integers are lowered according to the selected target without
/// requiring the generated program to run on the build host.
pub fn compile_to_ir_for_target(
    source: &str,
    options: CompileOptions,
    target_triple: &str,
) -> Result<String> {
    let pointer_width = target_pointer_width_for_triple(target_triple)?;
    compile_to_ir_inner(
        source,
        options,
        Some(target_triple.to_string()),
        pointer_width,
    )
}

fn compile_to_ir_inner(
    source: &str,
    options: CompileOptions,
    target_triple: Option<String>,
    pointer_width: TargetPointerWidth,
) -> Result<String> {
    let bundle = compile_to_mir_bundle_inner(source, options, pointer_width)?;
    let ffi_codegen = bundle.ffi_codegen;
    let mut mir_fns = bundle.functions;

    // 5. MIR optimization pipeline for the selected level.
    let pipeline = mir::opt::pipeline_for_level(options.mir_opt_level);
    pipeline.run(&mut mir_fns);

    // 6. Code generation (MIR -> LLVM IR).
    let overflow_mode = match options.mir_opt_level {
        MirOptLevel::O0 | MirOptLevel::O1 => IntegerOverflowMode::DebugChecked,
        MirOptLevel::O2 | MirOptLevel::O3 => IntegerOverflowMode::ReleaseWrapping,
    };
    let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
        ffi_codegen,
        target_triple,
        DebugInfoConfig::disabled(),
        overflow_mode,
    );
    codegen.codegen(&mir_fns).map_err(CompileError::Codegen)
}

/// Compile Sengoo source to LLVM IR using default options.
pub fn compile_to_ir(source: &str) -> Result<String> {
    compile_to_ir_with_options(source, CompileOptions::default())
}

/// Parse and type-check source, returning non-fatal diagnostics.
pub fn collect_compile_warnings(source: &str) -> Result<Vec<CompileWarning>> {
    let program = Parser::parse(source)?;
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    Ok(checker.warnings().to_vec())
}

/// Compile Sengoo source to MIR functions (without code generation).
pub fn compile_to_mir(source: &str) -> Result<Vec<mir::MirFunction>> {
    Ok(compile_to_mir_bundle(source)?.functions)
}

/// Compile Sengoo source to MIR functions (without code generation) using explicit options.
pub fn compile_to_mir_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<Vec<mir::MirFunction>> {
    Ok(compile_to_mir_bundle_with_options(source, options)?.functions)
}

/// Compile Sengoo source to a target-aware MIR bundle using default options.
pub fn compile_to_mir_bundle(source: &str) -> Result<MirBundle> {
    compile_to_mir_bundle_with_options(source, CompileOptions::default())
}

/// Compile Sengoo source to a target-aware MIR bundle using explicit options.
pub fn compile_to_mir_bundle_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<MirBundle> {
    compile_to_mir_bundle_inner(source, options, TargetPointerWidth::host())
}

/// Compile Sengoo source to a target-aware MIR bundle for an explicit target triple.
pub fn compile_to_mir_bundle_for_target(
    source: &str,
    options: CompileOptions,
    target_triple: &str,
) -> Result<MirBundle> {
    let pointer_width = target_pointer_width_for_triple(target_triple)?;
    compile_to_mir_bundle_inner(source, options, pointer_width)
}

fn compile_to_mir_bundle_inner(
    source: &str,
    options: CompileOptions,
    pointer_width: TargetPointerWidth,
) -> Result<MirBundle> {
    // 1. Parse source code.
    let program = Parser::parse_with_pointer_width(source, pointer_width.bits())?;

    // 2. Type checking.
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;
    let async_functions = checker.async_function_names().clone();
    let type_env = checker.into_env();

    // 3. HIR lowering.
    let hir_module = lower_ast(&program, &type_env);
    let ffi_codegen = collect_ffi_codegen_config_for_pointer_width(&hir_module, pointer_width);

    // 4. MIR lowering.
    let mut mir_fns = lower_hir_with_options(
        &hir_module.items,
        mir::MirLowerOptions::new(
            options.runtime_contract_checks,
            true,
            async_functions.clone(),
        )
        .with_target_pointer_width(pointer_width.bits()),
    )
    .map_err(CompileError::MirLower)?;
    drop(hir_module);
    drop(type_env);
    drop(program);

    if !async_functions.is_empty() {
        let async_helpers = mir::async_lowering::expand_async_functions(&mut mir_fns)?;
        mir_fns.extend(async_helpers);
    }
    Ok(MirBundle {
        semantic_abi_version: MIR_SEMANTIC_ABI_VERSION,
        target_pointer_width: pointer_width,
        functions: mir_fns,
        ffi_codegen,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_link_tests {
    use super::*;
    use crate::{lower_ast, Parser, TypeChecker};

    #[test]
    fn collect_native_link_libraries_dedupes_extern_blocks() {
        let source = r#"
            #[link(name = "sample")]
            extern "C" {
                fn foo();
            }

            #[link(name = "sample")]
            extern "C" {
                fn bar();
            }

            #[link(name = "other")]
            extern "C" {
                fn baz();
            }
        "#;
        let program = Parser::parse(source).unwrap();
        let mut checker = TypeChecker::new();
        checker.check_program(&program).unwrap();
        let type_env = checker.into_env();
        let hir_module = lower_ast(&program, &type_env);
        assert_eq!(
            collect_native_link_libraries(&hir_module),
            vec!["sample".to_string(), "other".to_string()]
        );
    }
}
