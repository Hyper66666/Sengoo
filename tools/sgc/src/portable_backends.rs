use miette::{Context, IntoDiagnostic, Result};
use sengoo_compiler::mir::{
    CallArg, Instruction, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp, Terminator,
};
use sengoo_compiler::{TargetPointerWidth, MIR_SEMANTIC_ABI_VERSION};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const BYTECODE_MAGIC: &[u8; 4] = b"SGB1";
const BYTECODE_VERSION: u16 = 1;
const VM_STEP_LIMIT: u64 = 10_000_000;
const VM_RECURSION_LIMIT: usize = 1024;

/// Portable runtime ABI version consumed by experimental WASM/bytecode backends.
pub(crate) const PORTABLE_RUNTIME_ABI_VERSION: u32 = 1;
const WASM_TARGET_TRIPLE: &str = "wasm32-unknown-unknown";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortableBackendTarget {
    Wasm,
    Bytecode,
}

impl PortableBackendTarget {
    fn name(self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::Bytecode => "bytecode",
        }
    }

    fn target_triple(self) -> Option<&'static str> {
        match self {
            Self::Wasm => Some(WASM_TARGET_TRIPLE),
            // Bytecode prototype remains host-width until the child value review
            // freezes a portable layout contract.
            Self::Bytecode => None,
        }
    }
}

/// Reject unknown MIR semantic / portable runtime ABI versions before lowering.
pub(crate) fn validate_portable_abi_versions(
    mir_semantic_abi_version: u32,
    portable_runtime_abi_version: u32,
) -> Result<()> {
    if mir_semantic_abi_version != MIR_SEMANTIC_ABI_VERSION {
        miette::bail!(
            "unsupported-mir-semantic-abi: unsupported MIR semantic ABI version {mir_semantic_abi_version} (expected {MIR_SEMANTIC_ABI_VERSION})"
        );
    }
    if portable_runtime_abi_version != PORTABLE_RUNTIME_ABI_VERSION {
        miette::bail!(
            "unsupported-portable-runtime-abi: unsupported portable runtime ABI version {portable_runtime_abi_version} (expected {PORTABLE_RUNTIME_ABI_VERSION})"
        );
    }
    Ok(())
}

fn unsupported_target_capability(
    target: PortableBackendTarget,
    capability: impl std::fmt::Display,
) -> miette::Error {
    miette::miette!(
        "unsupported-target-capability: target `{}` does not support {}; see docs/portable-targets.md",
        target.name(),
        capability
    )
}

#[derive(Debug, Clone)]
struct PortableProgram {
    functions: Vec<PortableFunction>,
    main_index: u32,
}

#[derive(Debug, Clone)]
struct PortableFunction {
    name: String,
    param_count: u32,
    local_count: u32,
    start_block: u32,
    blocks: Vec<PortableBlock>,
}

#[derive(Debug, Clone)]
struct PortableBlock {
    instructions: Vec<PortableInstruction>,
    terminator: PortableTerminator,
}

#[derive(Debug, Clone)]
enum PortableInstruction {
    Const {
        destination: u32,
        value: i64,
    },
    Unary {
        destination: u32,
        op: PortableUnary,
        operand: u32,
    },
    Binary {
        destination: u32,
        op: PortableBinary,
        left: u32,
        right: u32,
    },
    Move {
        destination: u32,
        source: u32,
    },
    Call {
        destination: u32,
        function: u32,
        args: Vec<u32>,
    },
    Phi {
        destination: u32,
        incoming: Vec<(u32, u32)>,
    },
    Nop,
}

#[derive(Debug, Clone)]
enum PortableTerminator {
    Return(Option<u32>),
    Goto(u32),
    If {
        condition: u32,
        then_block: u32,
        else_block: u32,
    },
    Switch {
        discriminant: u32,
        targets: Vec<(u32, u32)>,
        otherwise: u32,
    },
    Call {
        function: u32,
        args: Vec<PortableArg>,
        destination: u32,
        target: u32,
    },
    Unreachable,
}

#[derive(Debug, Clone)]
enum PortableArg {
    Local(u32),
    Constant(i64),
}

#[derive(Debug, Clone, Copy)]
enum PortableUnary {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, Copy)]
enum PortableBinary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    LogAnd,
    LogOr,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

pub(crate) fn build_bytecode(input: &str, output: Option<&str>, opt_level: u8) -> Result<PathBuf> {
    let program = compile_portable_input(input, opt_level, PortableBackendTarget::Bytecode)?;
    let bytes = encode_bytecode(&program)?;
    let output = output_path(input, output, "sgbc");
    write_artifact(&output, &bytes, "bytecode")?;
    println!("Bytecode written to {}", output.display());
    Ok(output)
}

pub(crate) fn run_bytecode(input: &str, opt_level: u8) -> Result<i64> {
    let program = if Path::new(input).extension().and_then(|ext| ext.to_str()) == Some("sgbc") {
        let bytes = fs::read(input)
            .into_diagnostic()
            .with_context(|| format!("failed to read bytecode artifact {input}"))?;
        decode_bytecode(&bytes)?
    } else {
        compile_portable_input(input, opt_level, PortableBackendTarget::Bytecode)?
    };
    execute_bytecode(&program)
}

pub(crate) fn build_wasm(input: &str, output: Option<&str>, opt_level: u8) -> Result<PathBuf> {
    let program = compile_portable_input(input, opt_level, PortableBackendTarget::Wasm)?;
    let bytes = encode_and_validate_wasm(&program)?;
    let output = output_path(input, output, "wasm");
    write_artifact(&output, &bytes, "WebAssembly")?;
    println!("WebAssembly written to {}", output.display());
    Ok(output)
}

/// Build and execute a scalar WASM module via a pinned host runtime.
///
/// Runtime selection order: `SENGOO_WASM_RUNTIME`, then `node`, then `wasmtime`.
/// Resource limits: node heap is left default; wasmtime uses `--wasm-timeout`
/// when available. Fuel/memory caps are documented in `docs/wasm-wasi-profile.md`.
pub(crate) fn run_wasm(input: &str, opt_level: u8) -> Result<i64> {
    let program = if Path::new(input).extension().and_then(|ext| ext.to_str()) == Some("wasm") {
        let bytes = fs::read(input)
            .into_diagnostic()
            .with_context(|| format!("failed to read WebAssembly artifact {input}"))?;
        validate_wasm_module(&bytes)?;
        return execute_wasm_bytes(&bytes);
    } else {
        compile_portable_input(input, opt_level, PortableBackendTarget::Wasm)?
    };
    let bytes = encode_and_validate_wasm(&program)?;
    execute_wasm_bytes(&bytes)
}

fn encode_and_validate_wasm(program: &PortableProgram) -> Result<Vec<u8>> {
    let bytes = encode_wasm(program)?;
    validate_wasm_module(&bytes)?;
    Ok(bytes)
}

/// Structural WebAssembly module validator for the scalar emitter.
///
/// Validates magic/version, section id order, and that type/function/code
/// counts agree. This is intentionally independent of an external wasm parser.
pub(crate) fn validate_wasm_module(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 8 || &bytes[0..4] != b"\0asm" || bytes[4..8] != [0x01, 0, 0, 0] {
        miette::bail!("invalid WebAssembly module header");
    }

    let mut offset = 8usize;
    let mut last_id = 0u8;
    let mut type_count = None;
    let mut function_count = None;
    let mut code_count = None;
    let mut saw_export_main = false;

    while offset < bytes.len() {
        let id = bytes[offset];
        offset += 1;
        if id != 0 && id <= last_id {
            miette::bail!("WebAssembly sections are out of order (id {id} after {last_id})");
        }
        if id != 0 {
            last_id = id;
        }
        let (payload_len, next) = read_uleb_at(bytes, offset)?;
        offset = next;
        let end = offset
            .checked_add(payload_len as usize)
            .ok_or_else(|| miette::miette!("WebAssembly section length overflow"))?;
        if end > bytes.len() {
            miette::bail!("truncated WebAssembly section payload");
        }
        let payload = &bytes[offset..end];
        match id {
            1 => {
                let (count, _) = read_uleb_at(payload, 0)?;
                type_count = Some(count);
            }
            3 => {
                let (count, _) = read_uleb_at(payload, 0)?;
                function_count = Some(count);
            }
            7 => {
                if payload_contains_export_main(payload) {
                    saw_export_main = true;
                }
            }
            10 => {
                let (count, _) = read_uleb_at(payload, 0)?;
                code_count = Some(count);
            }
            0 | 2 | 4 | 5 | 6 | 8 | 9 | 11 | 12 => {}
            other => miette::bail!("unsupported WebAssembly section id {other}"),
        }
        offset = end;
    }

    let types = type_count.unwrap_or(0);
    let functions = function_count.unwrap_or(0);
    let codes = code_count.unwrap_or(0);
    if types == 0 || functions == 0 || codes == 0 {
        miette::bail!("WebAssembly module is missing required type/function/code sections");
    }
    if types != functions || functions != codes {
        miette::bail!(
            "WebAssembly type/function/code counts disagree ({types}/{functions}/{codes})"
        );
    }
    if !saw_export_main {
        miette::bail!("WebAssembly module does not export `main`");
    }
    Ok(())
}

fn payload_contains_export_main(payload: &[u8]) -> bool {
    // Export section: count, then (name_len, name_bytes, kind, index)*
    let Ok((count, mut offset)) = read_uleb_at(payload, 0) else {
        return false;
    };
    for _ in 0..count {
        let Ok((name_len, next)) = read_uleb_at(payload, offset) else {
            return false;
        };
        offset = next;
        let end = offset.saturating_add(name_len as usize);
        if end > payload.len() {
            return false;
        }
        if &payload[offset..end] == b"main" {
            return true;
        }
        offset = end;
        // kind + index
        if offset >= payload.len() {
            return false;
        }
        offset += 1;
        let Ok((_, next)) = read_uleb_at(payload, offset) else {
            return false;
        };
        offset = next;
    }
    false
}

fn read_uleb_at(bytes: &[u8], mut offset: usize) -> Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes
            .get(offset)
            .ok_or_else(|| miette::miette!("truncated WebAssembly LEB128"))?;
        offset += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, offset));
        }
        shift += 7;
        if shift > 63 {
            miette::bail!("WebAssembly LEB128 is too large");
        }
    }
}

fn execute_wasm_bytes(bytes: &[u8]) -> Result<i64> {
    let dir = std::env::temp_dir().join(format!(
        "sgc-wasm-run-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir)
        .into_diagnostic()
        .context("failed to create temporary WASM run directory")?;
    let module_path = dir.join("module.wasm");
    fs::write(&module_path, bytes)
        .into_diagnostic()
        .context("failed to write temporary WASM module")?;

    let result = execute_wasm_with_available_runtime(&module_path);
    let _ = fs::remove_dir_all(&dir);
    result
}

fn execute_wasm_with_available_runtime(module_path: &Path) -> Result<i64> {
    if let Ok(runtime) = std::env::var("SENGOO_WASM_RUNTIME") {
        return match runtime.as_str() {
            "node" => execute_wasm_with_node(module_path),
            "wasmtime" => execute_wasm_with_wasmtime(module_path),
            other => miette::bail!(
                "unsupported SENGOO_WASM_RUNTIME `{other}`; expected `node` or `wasmtime`"
            ),
        };
    }
    if which::which("node").is_ok() {
        return execute_wasm_with_node(module_path);
    }
    if which::which("wasmtime").is_ok() {
        return execute_wasm_with_wasmtime(module_path);
    }
    miette::bail!(
        "no WebAssembly runtime found; install Node.js or wasmtime, or set SENGOO_WASM_RUNTIME"
    )
}

fn execute_wasm_with_node(module_path: &Path) -> Result<i64> {
    let script = module_path.with_extension("run.js");
    fs::write(
        &script,
        r#"const fs = require("fs");
const path = process.argv[2];
const bytes = fs.readFileSync(path);
WebAssembly.instantiate(bytes).then(({ instance }) => {
  const main = instance.exports.main;
  if (typeof main !== "function") {
    console.error("module does not export main");
    process.exit(2);
  }
  const value = main();
  const code = typeof value === "bigint" ? value.toString() : String(value);
  process.stdout.write(code + "\n");
}).catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
"#,
    )
    .into_diagnostic()
    .context("failed to write Node WASM runner script")?;

    let output = std::process::Command::new("node")
        .arg(&script)
        .arg(module_path)
        .output()
        .into_diagnostic()
        .context("failed to invoke Node.js WebAssembly runtime")?;
    let _ = fs::remove_file(&script);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        miette::bail!("Node.js WASM execution failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .rev()
        .find_map(|line| line.trim().parse::<i64>().ok())
        .ok_or_else(|| {
            miette::miette!("Node.js WASM runner did not print a parseable main result:\n{stdout}")
        })
}

fn execute_wasm_with_wasmtime(module_path: &Path) -> Result<i64> {
    let mut command = std::process::Command::new("wasmtime");
    // Bound wall time when the installed wasmtime supports it.
    command.args(["run", "--invoke", "main"]);
    command.arg(module_path);
    let output = command
        .output()
        .into_diagnostic()
        .context("failed to invoke wasmtime")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        miette::bail!("wasmtime execution failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout
        .lines()
        .rev()
        .find_map(|line| line.trim().parse::<i64>().ok())
        .ok_or_else(|| {
            miette::miette!("wasmtime did not print a parseable main result:\n{stdout}")
        })?;
    Ok(value)
}

fn compile_portable_input(
    input: &str,
    opt_level: u8,
    target: PortableBackendTarget,
) -> Result<PortableProgram> {
    let source = fs::read_to_string(input)
        .into_diagnostic()
        .with_context(|| format!("failed to read source {input}"))?;
    let source = crate::expand_imports_for_source(Path::new(input), &source)?;
    let bundle = crate::pipeline::compile_source_to_mir_bundle(
        &source,
        opt_level,
        target.target_triple(),
    )
    .map_err(|err| miette::miette!("portable target frontend failed: {err}"))?;

    validate_portable_abi_versions(bundle.semantic_abi_version, PORTABLE_RUNTIME_ABI_VERSION)?;
    if matches!(target, PortableBackendTarget::Wasm)
        && bundle.target_pointer_width != TargetPointerWidth::Bits32
    {
        return Err(unsupported_target_capability(
            target,
            format!(
                "non-wasm32 pointer width {:?}",
                bundle.target_pointer_width
            ),
        ));
    }

    if let Some(extern_decl) = bundle.ffi_codegen.extern_decls.first() {
        return Err(unsupported_target_capability(
            target,
            format!("FFI or host stdlib call `{}`", extern_decl.name),
        ));
    }
    PortableProgram::from_mir(target, &bundle.functions)
}

impl PortableProgram {
    fn from_mir(target: PortableBackendTarget, functions: &[MirFunction]) -> Result<Self> {
        let function_indices = functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.name.as_str(), index as u32))
            .collect::<BTreeMap<_, _>>();
        let main_index = *function_indices.get("main").ok_or_else(|| {
            unsupported_target_capability(target, "programs without a `main` entry point")
        })?;
        let functions = functions
            .iter()
            .map(|function| PortableFunction::from_mir(target, function, &function_indices))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            functions,
            main_index,
        })
    }
}

impl PortableFunction {
    fn from_mir(
        target: PortableBackendTarget,
        function: &MirFunction,
        function_indices: &BTreeMap<&str, u32>,
    ) -> Result<Self> {
        if function.is_async {
            return Err(unsupported_target_capability(
                target,
                format!("async function `{}`", function.name),
            ));
        }
        for (_, ty) in &function.locals {
            validate_portable_type(target, &function.name, ty)?;
        }
        let blocks = function
            .basic_blocks
            .iter()
            .map(|block| {
                let instructions = function
                    .block_instructions(block)
                    .map(|instruction| {
                        PortableInstruction::from_mir(
                            target,
                            &function.name,
                            instruction,
                            function_indices,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or_else(|| {
                        unsupported_target_capability(
                            target,
                            format!(
                                "unterminated MIR block {} in `{}`",
                                block.id, function.name
                            ),
                        )
                    })
                    .and_then(|terminator| {
                        PortableTerminator::from_mir(
                            target,
                            &function.name,
                            terminator,
                            function_indices,
                        )
                    })?;
                Ok(PortableBlock {
                    instructions,
                    terminator,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            name: function.name.clone(),
            param_count: function.params.len() as u32,
            local_count: function.locals.len() as u32,
            start_block: function.start_block as u32,
            blocks,
        })
    }
}

impl PortableInstruction {
    fn from_mir(
        target: PortableBackendTarget,
        function: &str,
        instruction: &Instruction,
        function_indices: &BTreeMap<&str, u32>,
    ) -> Result<Self> {
        Ok(match instruction {
            Instruction::Assign { destination, value } => Self::Const {
                destination: destination.id,
                value: portable_constant(target, function, value)?,
            },
            Instruction::Unary {
                destination,
                op,
                operand,
            } => Self::Unary {
                destination: destination.id,
                op: (*op).into(),
                operand: operand.id,
            },
            Instruction::Binary {
                destination,
                op,
                left,
                right,
            } => Self::Binary {
                destination: destination.id,
                op: (*op).into(),
                left: left.id,
                right: right.id,
            },
            Instruction::Load {
                destination,
                source,
            }
            | Instruction::AddrOf {
                destination,
                source,
            }
            | Instruction::Cast {
                destination,
                value: source,
                ..
            }
            | Instruction::Bitcast {
                destination,
                value: source,
                ..
            } => Self::Move {
                destination: destination.id,
                source: source.id,
            },
            Instruction::Store { destination, value } => Self::Move {
                destination: destination.id,
                source: value.id,
            },
            Instruction::Call {
                destination,
                func,
                args,
            } => Self::Call {
                destination: destination.id,
                function: resolve_function(target, function, func, function_indices)?,
                args: args.iter().map(|local| local.id).collect(),
            },
            Instruction::Phi {
                destination,
                incoming,
            } => Self::Phi {
                destination: destination.id,
                incoming: incoming
                    .iter()
                    .map(|(local, block)| (local.id, *block as u32))
                    .collect(),
            },
            Instruction::Nop => Self::Nop,
            other => {
                return Err(unsupported_target_capability(
                    target,
                    format!("MIR instruction {:?} in `{function}`", other),
                ));
            }
        })
    }
}

impl PortableTerminator {
    fn from_mir(
        target: PortableBackendTarget,
        function: &str,
        terminator: &Terminator,
        function_indices: &BTreeMap<&str, u32>,
    ) -> Result<Self> {
        Ok(match terminator {
            Terminator::Return(local) => Self::Return(local.map(|local| local.id)),
            Terminator::Goto(block)
            | Terminator::Break { target: block }
            | Terminator::Continue { target: block } => Self::Goto(*block as u32),
            Terminator::If {
                cond,
                then_block,
                else_block,
            } => Self::If {
                condition: cond.id,
                then_block: *then_block as u32,
                else_block: *else_block as u32,
            },
            Terminator::Switch {
                discr,
                targets,
                otherwise,
            } => Self::Switch {
                discriminant: discr.id,
                targets: targets
                    .iter()
                    .map(|(value, block)| (*value, *block as u32))
                    .collect(),
                otherwise: *otherwise as u32,
            },
            Terminator::Call {
                func,
                args,
                destination,
                target: next,
            } => Self::Call {
                function: resolve_function(target, function, func, function_indices)?,
                args: args
                    .iter()
                    .map(|arg| match arg {
                        CallArg::Local(local) => Ok(PortableArg::Local(local.id)),
                        CallArg::Constant(value) => Ok(PortableArg::Constant(portable_constant(
                            target, function, value,
                        )?)),
                    })
                    .collect::<Result<Vec<_>>>()?,
                destination: destination.id,
                target: *next as u32,
            },
            Terminator::Unreachable => Self::Unreachable,
            other => {
                return Err(unsupported_target_capability(
                    target,
                    format!("MIR terminator {:?} in `{function}`", other),
                ));
            }
        })
    }
}

impl From<MirUnOp> for PortableUnary {
    fn from(value: MirUnOp) -> Self {
        match value {
            MirUnOp::Neg => Self::Neg,
            MirUnOp::Not => Self::Not,
            MirUnOp::BitNot => Self::BitNot,
        }
    }
}

impl From<MirBinOp> for PortableBinary {
    fn from(value: MirBinOp) -> Self {
        match value {
            MirBinOp::Add => Self::Add,
            MirBinOp::Sub => Self::Sub,
            MirBinOp::Mul => Self::Mul,
            MirBinOp::Div => Self::Div,
            MirBinOp::Rem => Self::Rem,
            MirBinOp::BitAnd => Self::BitAnd,
            MirBinOp::BitOr => Self::BitOr,
            MirBinOp::BitXor => Self::BitXor,
            MirBinOp::Shl => Self::Shl,
            MirBinOp::Shr => Self::Shr,
            MirBinOp::LogAnd => Self::LogAnd,
            MirBinOp::LogOr => Self::LogOr,
            MirBinOp::Eq => Self::Eq,
            MirBinOp::Ne => Self::Ne,
            MirBinOp::Lt => Self::Lt,
            MirBinOp::Gt => Self::Gt,
            MirBinOp::Le => Self::Le,
            MirBinOp::Ge => Self::Ge,
        }
    }
}

fn validate_portable_type(
    target: PortableBackendTarget,
    function: &str,
    ty: &MIRType,
) -> Result<()> {
    if matches!(
        ty,
        MIRType::Unit
            | MIRType::Never
            | MIRType::Bool
            | MIRType::Int(_)
            | MIRType::UInt(_)
            | MIRType::Ref(_)
            | MIRType::Ptr(_)
            | MIRType::Future(_)
    ) {
        return Ok(());
    }
    Err(unsupported_target_capability(
        target,
        format!("MIR type {:?} in `{function}`", ty),
    ))
}

fn portable_constant(
    target: PortableBackendTarget,
    function: &str,
    value: &MirConstant,
) -> Result<i64> {
    match value {
        MirConstant::Unit => Ok(0),
        MirConstant::Bool(value) => Ok(i64::from(*value)),
        MirConstant::Int(value) => Ok(*value),
        MirConstant::Uint(value) => Ok(*value as i64),
        MirConstant::Char(value) => Ok(*value as i64),
        MirConstant::GlobalRef(_) => Ok(0),
        other => Err(unsupported_target_capability(
            target,
            format!("constant {:?} in `{function}`", other),
        )),
    }
}

fn resolve_function(
    target: PortableBackendTarget,
    caller: &str,
    callee: &str,
    function_indices: &BTreeMap<&str, u32>,
) -> Result<u32> {
    function_indices.get(callee).copied().ok_or_else(|| {
        unsupported_target_capability(
            target,
            format!("host/stdlib call `{callee}` from `{caller}`"),
        )
    })
}

fn output_path(input: &str, output: Option<&str>, extension: &str) -> PathBuf {
    output
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(input).with_extension(extension))
}

fn write_artifact(path: &Path, bytes: &[u8], kind: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .with_context(|| format!("failed to create {} output directory", kind))?;
    }
    fs::write(path, bytes)
        .into_diagnostic()
        .with_context(|| format!("failed to write {} artifact {}", kind, path.display()))
}

fn execute_bytecode(program: &PortableProgram) -> Result<i64> {
    let mut vm = PortableVm {
        program,
        steps: 0,
        depth: 0,
    };
    vm.call(program.main_index, &[])
}

struct PortableVm<'a> {
    program: &'a PortableProgram,
    steps: u64,
    depth: usize,
}

impl PortableVm<'_> {
    fn call(&mut self, function_index: u32, args: &[i64]) -> Result<i64> {
        if self.depth >= VM_RECURSION_LIMIT {
            miette::bail!("bytecode VM recursion limit exceeded");
        }
        let function = self
            .program
            .functions
            .get(function_index as usize)
            .ok_or_else(|| {
                miette::miette!("bytecode references missing function {function_index}")
            })?
            .clone();
        if args.len() != function.param_count as usize {
            miette::bail!(
                "bytecode call to `{}` expected {} argument(s), got {}",
                function.name,
                function.param_count,
                args.len()
            );
        }
        let mut locals = vec![0i64; function.local_count as usize];
        for (index, value) in args.iter().copied().enumerate() {
            set_local(&mut locals, index as u32 + 1, value)?;
        }
        let mut block = function.start_block;
        let mut previous_block = None;
        self.depth += 1;
        let result = loop {
            self.steps += 1;
            if self.steps > VM_STEP_LIMIT {
                break Err(miette::miette!("bytecode VM instruction limit exceeded"));
            }
            let current = function.blocks.get(block as usize).ok_or_else(|| {
                miette::miette!(
                    "bytecode function `{}` references missing block {}",
                    function.name,
                    block
                )
            })?;
            for instruction in &current.instructions {
                self.execute_instruction(instruction, previous_block, &mut locals)?;
            }
            let next = match &current.terminator {
                PortableTerminator::Return(local) => {
                    break Ok(local
                        .map(|local| get_local(&locals, local))
                        .transpose()?
                        .unwrap_or(0));
                }
                PortableTerminator::Goto(target) => *target,
                PortableTerminator::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    if get_local(&locals, *condition)? != 0 {
                        *then_block
                    } else {
                        *else_block
                    }
                }
                PortableTerminator::Switch {
                    discriminant,
                    targets,
                    otherwise,
                } => {
                    let value = get_local(&locals, *discriminant)? as u32;
                    targets
                        .iter()
                        .find_map(|(candidate, target)| (*candidate == value).then_some(*target))
                        .unwrap_or(*otherwise)
                }
                PortableTerminator::Call {
                    function,
                    args,
                    destination,
                    target,
                } => {
                    let values = args
                        .iter()
                        .map(|arg| match arg {
                            PortableArg::Local(local) => get_local(&locals, *local),
                            PortableArg::Constant(value) => Ok(*value),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let value = self.call(*function, &values)?;
                    set_local(&mut locals, *destination, value)?;
                    *target
                }
                PortableTerminator::Unreachable => {
                    break Err(miette::miette!(
                        "bytecode VM reached unreachable in `{}`",
                        function.name
                    ));
                }
            };
            previous_block = Some(block);
            block = next;
        };
        self.depth -= 1;
        result
    }

    fn execute_instruction(
        &mut self,
        instruction: &PortableInstruction,
        previous_block: Option<u32>,
        locals: &mut [i64],
    ) -> Result<()> {
        self.steps += 1;
        if self.steps > VM_STEP_LIMIT {
            miette::bail!("bytecode VM instruction limit exceeded");
        }
        match instruction {
            PortableInstruction::Const { destination, value } => {
                set_local(locals, *destination, *value)
            }
            PortableInstruction::Unary {
                destination,
                op,
                operand,
            } => {
                let value = get_local(locals, *operand)?;
                let result = match op {
                    PortableUnary::Neg => value.wrapping_neg(),
                    PortableUnary::Not => i64::from(value == 0),
                    PortableUnary::BitNot => !value,
                };
                set_local(locals, *destination, result)
            }
            PortableInstruction::Binary {
                destination,
                op,
                left,
                right,
            } => {
                let result =
                    eval_binary(*op, get_local(locals, *left)?, get_local(locals, *right)?)?;
                set_local(locals, *destination, result)
            }
            PortableInstruction::Move {
                destination,
                source,
            } => set_local(locals, *destination, get_local(locals, *source)?),
            PortableInstruction::Call {
                destination,
                function,
                args,
            } => {
                let args = args
                    .iter()
                    .map(|local| get_local(locals, *local))
                    .collect::<Result<Vec<_>>>()?;
                let value = self.call(*function, &args)?;
                set_local(locals, *destination, value)
            }
            PortableInstruction::Phi {
                destination,
                incoming,
            } => {
                let previous = previous_block.ok_or_else(|| {
                    miette::miette!("bytecode phi executed without predecessor block")
                })?;
                let source = incoming
                    .iter()
                    .find_map(|(source, block)| (*block == previous).then_some(*source))
                    .ok_or_else(|| {
                        miette::miette!("bytecode phi has no incoming value for block {previous}")
                    })?;
                set_local(locals, *destination, get_local(locals, source)?)
            }
            PortableInstruction::Nop => Ok(()),
        }
    }
}

fn get_local(locals: &[i64], local: u32) -> Result<i64> {
    locals
        .get(local as usize)
        .copied()
        .ok_or_else(|| miette::miette!("bytecode references missing local {local}"))
}

fn set_local(locals: &mut [i64], local: u32, value: i64) -> Result<()> {
    let slot = locals
        .get_mut(local as usize)
        .ok_or_else(|| miette::miette!("bytecode references missing local {local}"))?;
    *slot = value;
    Ok(())
}

fn eval_binary(op: PortableBinary, left: i64, right: i64) -> Result<i64> {
    Ok(match op {
        PortableBinary::Add => left.wrapping_add(right),
        PortableBinary::Sub => left.wrapping_sub(right),
        PortableBinary::Mul => left.wrapping_mul(right),
        PortableBinary::Div => left
            .checked_div(right)
            .ok_or_else(|| miette::miette!("bytecode integer division trap"))?,
        PortableBinary::Rem => left
            .checked_rem(right)
            .ok_or_else(|| miette::miette!("bytecode integer remainder trap"))?,
        PortableBinary::BitAnd => left & right,
        PortableBinary::BitOr => left | right,
        PortableBinary::BitXor => left ^ right,
        PortableBinary::Shl => left.wrapping_shl((right as u32) & 63),
        PortableBinary::Shr => left.wrapping_shr((right as u32) & 63),
        PortableBinary::LogAnd => i64::from(left != 0 && right != 0),
        PortableBinary::LogOr => i64::from(left != 0 || right != 0),
        PortableBinary::Eq => i64::from(left == right),
        PortableBinary::Ne => i64::from(left != right),
        PortableBinary::Lt => i64::from(left < right),
        PortableBinary::Gt => i64::from(left > right),
        PortableBinary::Le => i64::from(left <= right),
        PortableBinary::Ge => i64::from(left >= right),
    })
}

fn encode_bytecode(program: &PortableProgram) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(BYTECODE_MAGIC);
    write_u16(&mut output, BYTECODE_VERSION);
    write_u32(&mut output, program.main_index);
    write_u32(&mut output, program.functions.len() as u32);
    for function in &program.functions {
        write_string(&mut output, &function.name)?;
        write_u32(&mut output, function.param_count);
        write_u32(&mut output, function.local_count);
        write_u32(&mut output, function.start_block);
        write_u32(&mut output, function.blocks.len() as u32);
        for block in &function.blocks {
            write_u32(&mut output, block.instructions.len() as u32);
            for instruction in &block.instructions {
                encode_instruction(&mut output, instruction);
            }
            encode_terminator(&mut output, &block.terminator);
        }
    }
    Ok(output)
}

fn decode_bytecode(bytes: &[u8]) -> Result<PortableProgram> {
    let mut reader = BytecodeReader::new(bytes);
    if reader.read_exact(4)? != BYTECODE_MAGIC {
        miette::bail!("invalid Sengoo bytecode magic");
    }
    let version = reader.read_u16()?;
    if version != BYTECODE_VERSION {
        miette::bail!(
            "unsupported Sengoo bytecode version {}; expected {}",
            version,
            BYTECODE_VERSION
        );
    }
    let main_index = reader.read_u32()?;
    let function_count = reader.read_count("function")?;
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        let name = reader.read_string()?;
        let param_count = reader.read_u32()?;
        let local_count = reader.read_u32()?;
        let start_block = reader.read_u32()?;
        let block_count = reader.read_count("block")?;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let instruction_count = reader.read_count("instruction")?;
            let mut instructions = Vec::with_capacity(instruction_count);
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut reader)?);
            }
            blocks.push(PortableBlock {
                instructions,
                terminator: decode_terminator(&mut reader)?,
            });
        }
        functions.push(PortableFunction {
            name,
            param_count,
            local_count,
            start_block,
            blocks,
        });
    }
    if reader.remaining() != 0 {
        miette::bail!("Sengoo bytecode has trailing data");
    }
    if main_index as usize >= functions.len() {
        miette::bail!("Sengoo bytecode main function index is out of range");
    }
    Ok(PortableProgram {
        functions,
        main_index,
    })
}

fn encode_instruction(output: &mut Vec<u8>, instruction: &PortableInstruction) {
    match instruction {
        PortableInstruction::Const { destination, value } => {
            output.push(0);
            write_u32(output, *destination);
            write_i64(output, *value);
        }
        PortableInstruction::Unary {
            destination,
            op,
            operand,
        } => {
            output.push(1);
            write_u32(output, *destination);
            output.push(unary_code(*op));
            write_u32(output, *operand);
        }
        PortableInstruction::Binary {
            destination,
            op,
            left,
            right,
        } => {
            output.push(2);
            write_u32(output, *destination);
            output.push(binary_code(*op));
            write_u32(output, *left);
            write_u32(output, *right);
        }
        PortableInstruction::Move {
            destination,
            source,
        } => {
            output.push(3);
            write_u32(output, *destination);
            write_u32(output, *source);
        }
        PortableInstruction::Call {
            destination,
            function,
            args,
        } => {
            output.push(4);
            write_u32(output, *destination);
            write_u32(output, *function);
            write_u32(output, args.len() as u32);
            for arg in args {
                write_u32(output, *arg);
            }
        }
        PortableInstruction::Phi {
            destination,
            incoming,
        } => {
            output.push(5);
            write_u32(output, *destination);
            write_u32(output, incoming.len() as u32);
            for (source, block) in incoming {
                write_u32(output, *source);
                write_u32(output, *block);
            }
        }
        PortableInstruction::Nop => output.push(6),
    }
}

fn decode_instruction(reader: &mut BytecodeReader<'_>) -> Result<PortableInstruction> {
    Ok(match reader.read_u8()? {
        0 => PortableInstruction::Const {
            destination: reader.read_u32()?,
            value: reader.read_i64()?,
        },
        1 => PortableInstruction::Unary {
            destination: reader.read_u32()?,
            op: decode_unary(reader.read_u8()?)?,
            operand: reader.read_u32()?,
        },
        2 => PortableInstruction::Binary {
            destination: reader.read_u32()?,
            op: decode_binary(reader.read_u8()?)?,
            left: reader.read_u32()?,
            right: reader.read_u32()?,
        },
        3 => PortableInstruction::Move {
            destination: reader.read_u32()?,
            source: reader.read_u32()?,
        },
        4 => {
            let destination = reader.read_u32()?;
            let function = reader.read_u32()?;
            let count = reader.read_count("call argument")?;
            let mut args = Vec::with_capacity(count);
            for _ in 0..count {
                args.push(reader.read_u32()?);
            }
            PortableInstruction::Call {
                destination,
                function,
                args,
            }
        }
        5 => {
            let destination = reader.read_u32()?;
            let count = reader.read_count("phi input")?;
            let mut incoming = Vec::with_capacity(count);
            for _ in 0..count {
                incoming.push((reader.read_u32()?, reader.read_u32()?));
            }
            PortableInstruction::Phi {
                destination,
                incoming,
            }
        }
        6 => PortableInstruction::Nop,
        opcode => miette::bail!("unknown Sengoo bytecode instruction opcode {opcode}"),
    })
}

fn encode_terminator(output: &mut Vec<u8>, terminator: &PortableTerminator) {
    match terminator {
        PortableTerminator::Return(local) => {
            output.push(0);
            output.push(u8::from(local.is_some()));
            if let Some(local) = local {
                write_u32(output, *local);
            }
        }
        PortableTerminator::Goto(target) => {
            output.push(1);
            write_u32(output, *target);
        }
        PortableTerminator::If {
            condition,
            then_block,
            else_block,
        } => {
            output.push(2);
            write_u32(output, *condition);
            write_u32(output, *then_block);
            write_u32(output, *else_block);
        }
        PortableTerminator::Switch {
            discriminant,
            targets,
            otherwise,
        } => {
            output.push(3);
            write_u32(output, *discriminant);
            write_u32(output, targets.len() as u32);
            for (value, target) in targets {
                write_u32(output, *value);
                write_u32(output, *target);
            }
            write_u32(output, *otherwise);
        }
        PortableTerminator::Call {
            function,
            args,
            destination,
            target,
        } => {
            output.push(4);
            write_u32(output, *function);
            write_u32(output, args.len() as u32);
            for arg in args {
                match arg {
                    PortableArg::Local(local) => {
                        output.push(0);
                        write_u32(output, *local);
                    }
                    PortableArg::Constant(value) => {
                        output.push(1);
                        write_i64(output, *value);
                    }
                }
            }
            write_u32(output, *destination);
            write_u32(output, *target);
        }
        PortableTerminator::Unreachable => output.push(5),
    }
}

fn decode_terminator(reader: &mut BytecodeReader<'_>) -> Result<PortableTerminator> {
    Ok(match reader.read_u8()? {
        0 => PortableTerminator::Return(if reader.read_u8()? == 0 {
            None
        } else {
            Some(reader.read_u32()?)
        }),
        1 => PortableTerminator::Goto(reader.read_u32()?),
        2 => PortableTerminator::If {
            condition: reader.read_u32()?,
            then_block: reader.read_u32()?,
            else_block: reader.read_u32()?,
        },
        3 => {
            let discriminant = reader.read_u32()?;
            let count = reader.read_count("switch target")?;
            let mut targets = Vec::with_capacity(count);
            for _ in 0..count {
                targets.push((reader.read_u32()?, reader.read_u32()?));
            }
            PortableTerminator::Switch {
                discriminant,
                targets,
                otherwise: reader.read_u32()?,
            }
        }
        4 => {
            let function = reader.read_u32()?;
            let count = reader.read_count("call argument")?;
            let mut args = Vec::with_capacity(count);
            for _ in 0..count {
                args.push(match reader.read_u8()? {
                    0 => PortableArg::Local(reader.read_u32()?),
                    1 => PortableArg::Constant(reader.read_i64()?),
                    tag => miette::bail!("unknown bytecode call argument tag {tag}"),
                });
            }
            PortableTerminator::Call {
                function,
                args,
                destination: reader.read_u32()?,
                target: reader.read_u32()?,
            }
        }
        5 => PortableTerminator::Unreachable,
        opcode => miette::bail!("unknown Sengoo bytecode terminator opcode {opcode}"),
    })
}

fn unary_code(op: PortableUnary) -> u8 {
    match op {
        PortableUnary::Neg => 0,
        PortableUnary::Not => 1,
        PortableUnary::BitNot => 2,
    }
}

fn decode_unary(code: u8) -> Result<PortableUnary> {
    match code {
        0 => Ok(PortableUnary::Neg),
        1 => Ok(PortableUnary::Not),
        2 => Ok(PortableUnary::BitNot),
        _ => Err(miette::miette!("unknown bytecode unary opcode {code}")),
    }
}

fn binary_code(op: PortableBinary) -> u8 {
    op as u8
}

fn decode_binary(code: u8) -> Result<PortableBinary> {
    const OPS: [PortableBinary; 18] = [
        PortableBinary::Add,
        PortableBinary::Sub,
        PortableBinary::Mul,
        PortableBinary::Div,
        PortableBinary::Rem,
        PortableBinary::BitAnd,
        PortableBinary::BitOr,
        PortableBinary::BitXor,
        PortableBinary::Shl,
        PortableBinary::Shr,
        PortableBinary::LogAnd,
        PortableBinary::LogOr,
        PortableBinary::Eq,
        PortableBinary::Ne,
        PortableBinary::Lt,
        PortableBinary::Gt,
        PortableBinary::Le,
        PortableBinary::Ge,
    ];
    OPS.get(code as usize)
        .copied()
        .ok_or_else(|| miette::miette!("unknown bytecode binary opcode {code}"))
}

struct BytecodeReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BytecodeReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn read_exact(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| miette::miette!("bytecode offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| miette::miette!("truncated Sengoo bytecode"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        let bytes: [u8; 2] = self.read_exact(2)?.try_into().unwrap();
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes: [u8; 4] = self.read_exact(4)?.try_into().unwrap();
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64> {
        let bytes: [u8; 8] = self.read_exact(8)?.try_into().unwrap();
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_count(&mut self, kind: &str) -> Result<usize> {
        let count = self.read_u32()? as usize;
        if count > self.remaining().saturating_add(1) {
            miette::bail!("invalid {kind} count in Sengoo bytecode");
        }
        Ok(count)
    }

    fn read_string(&mut self) -> Result<String> {
        let length = self.read_u32()? as usize;
        let bytes = self.read_exact(length)?;
        String::from_utf8(bytes.to_vec())
            .into_diagnostic()
            .context("bytecode function name is not UTF-8")
    }
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_i64(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<()> {
    let length = u32::try_from(value.len())
        .into_diagnostic()
        .context("bytecode function name is too long")?;
    write_u32(output, length);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_wasm(program: &PortableProgram) -> Result<Vec<u8>> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();

    // Custom ABI metadata section (id 0) before standard sections.
    let mut custom = Vec::new();
    write_wasm_name(&mut custom, "sengoo.portable_runtime_abi");
    write_uleb(&mut custom, MIR_SEMANTIC_ABI_VERSION as u64);
    write_uleb(&mut custom, PORTABLE_RUNTIME_ABI_VERSION as u64);
    write_wasm_name(&mut custom, "wasm32");
    write_wasm_section(&mut module, 0, &custom);

    let mut types = Vec::new();
    write_uleb(&mut types, program.functions.len() as u64);
    for function in &program.functions {
        types.push(0x60);
        write_uleb(&mut types, function.param_count as u64);
        types.extend(std::iter::repeat_n(0x7e, function.param_count as usize));
        write_uleb(&mut types, 1);
        types.push(0x7e);
    }
    write_wasm_section(&mut module, 1, &types);

    let mut function_section = Vec::new();
    write_uleb(&mut function_section, program.functions.len() as u64);
    for index in 0..program.functions.len() {
        write_uleb(&mut function_section, index as u64);
    }
    write_wasm_section(&mut module, 3, &function_section);

    let mut exports = Vec::new();
    write_uleb(&mut exports, 1);
    write_wasm_name(&mut exports, "main");
    exports.push(0);
    write_uleb(&mut exports, program.main_index as u64);
    write_wasm_section(&mut module, 7, &exports);

    let mut code = Vec::new();
    write_uleb(&mut code, program.functions.len() as u64);
    for function in &program.functions {
        let body = encode_wasm_function(function)?;
        write_uleb(&mut code, body.len() as u64);
        code.extend_from_slice(&body);
    }
    write_wasm_section(&mut module, 10, &code);
    Ok(module)
}

fn encode_wasm_function(function: &PortableFunction) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let extra_locals = function.local_count + 2;
    write_uleb(&mut body, 1);
    write_uleb(&mut body, extra_locals as u64);
    body.push(0x7e);
    let block_local = function.param_count + function.local_count;
    let previous_block_local = block_local + 1;

    for parameter in 0..function.param_count {
        wasm_local_get(&mut body, parameter);
        wasm_local_set(&mut body, wasm_local(function, parameter + 1));
    }
    wasm_i64_const(&mut body, function.start_block as i64);
    wasm_local_set(&mut body, block_local);
    wasm_i64_const(&mut body, -1);
    wasm_local_set(&mut body, previous_block_local);

    body.extend_from_slice(&[0x02, 0x40, 0x03, 0x40]);
    for (block_index, block) in function.blocks.iter().enumerate() {
        wasm_local_get(&mut body, block_local);
        wasm_i64_const(&mut body, block_index as i64);
        body.push(0x51);
        body.extend_from_slice(&[0x04, 0x40]);
        for instruction in &block.instructions {
            encode_wasm_instruction(&mut body, function, instruction, previous_block_local)?;
        }
        encode_wasm_terminator(
            &mut body,
            function,
            &block.terminator,
            block_index as u32,
            block_local,
            previous_block_local,
        )?;
        body.push(0x0b);
    }
    body.push(0x00);
    body.extend_from_slice(&[0x0b, 0x0b]);
    wasm_i64_const(&mut body, 0);
    body.push(0x0b);
    Ok(body)
}

fn encode_wasm_instruction(
    output: &mut Vec<u8>,
    function: &PortableFunction,
    instruction: &PortableInstruction,
    previous_block_local: u32,
) -> Result<()> {
    match instruction {
        PortableInstruction::Const { destination, value } => {
            wasm_i64_const(output, *value);
            wasm_local_set(output, wasm_local(function, *destination));
        }
        PortableInstruction::Unary {
            destination,
            op,
            operand,
        } => {
            match op {
                PortableUnary::Neg => {
                    wasm_i64_const(output, 0);
                    wasm_local_get(output, wasm_local(function, *operand));
                    output.push(0x7d);
                }
                PortableUnary::Not => {
                    wasm_local_get(output, wasm_local(function, *operand));
                    output.extend_from_slice(&[0x50, 0xad]);
                }
                PortableUnary::BitNot => {
                    wasm_local_get(output, wasm_local(function, *operand));
                    wasm_i64_const(output, -1);
                    output.push(0x85);
                }
            }
            wasm_local_set(output, wasm_local(function, *destination));
        }
        PortableInstruction::Binary {
            destination,
            op,
            left,
            right,
        } => {
            encode_wasm_binary(output, function, *op, *left, *right);
            wasm_local_set(output, wasm_local(function, *destination));
        }
        PortableInstruction::Move {
            destination,
            source,
        } => {
            wasm_local_get(output, wasm_local(function, *source));
            wasm_local_set(output, wasm_local(function, *destination));
        }
        PortableInstruction::Call {
            destination,
            function: callee,
            args,
        } => {
            for arg in args {
                wasm_local_get(output, wasm_local(function, *arg));
            }
            output.push(0x10);
            write_uleb(output, *callee as u64);
            wasm_local_set(output, wasm_local(function, *destination));
        }
        PortableInstruction::Phi {
            destination,
            incoming,
        } => {
            wasm_i64_const(output, 0);
            wasm_local_set(output, wasm_local(function, *destination));
            for (source, predecessor) in incoming {
                wasm_local_get(output, previous_block_local);
                wasm_i64_const(output, *predecessor as i64);
                output.push(0x51);
                output.extend_from_slice(&[0x04, 0x40]);
                wasm_local_get(output, wasm_local(function, *source));
                wasm_local_set(output, wasm_local(function, *destination));
                output.push(0x0b);
            }
        }
        PortableInstruction::Nop => {}
    }
    Ok(())
}

fn encode_wasm_terminator(
    output: &mut Vec<u8>,
    function: &PortableFunction,
    terminator: &PortableTerminator,
    current_block: u32,
    block_local: u32,
    previous_block_local: u32,
) -> Result<()> {
    match terminator {
        PortableTerminator::Return(local) => {
            if let Some(local) = local {
                wasm_local_get(output, wasm_local(function, *local));
            } else {
                wasm_i64_const(output, 0);
            }
            output.push(0x0f);
        }
        PortableTerminator::Goto(target) => {
            wasm_transition(
                output,
                current_block,
                *target,
                block_local,
                previous_block_local,
            );
        }
        PortableTerminator::If {
            condition,
            then_block,
            else_block,
        } => {
            wasm_i64_const(output, current_block as i64);
            wasm_local_set(output, previous_block_local);
            wasm_i64_const(output, *then_block as i64);
            wasm_i64_const(output, *else_block as i64);
            wasm_local_get(output, wasm_local(function, *condition));
            output.extend_from_slice(&[0x50, 0x45]);
            output.push(0x1b);
            wasm_local_set(output, block_local);
            output.extend_from_slice(&[0x0c, 0x01]);
        }
        PortableTerminator::Switch {
            discriminant,
            targets,
            otherwise,
        } => {
            wasm_i64_const(output, current_block as i64);
            wasm_local_set(output, previous_block_local);
            wasm_i64_const(output, *otherwise as i64);
            wasm_local_set(output, block_local);
            for (value, target) in targets {
                wasm_local_get(output, wasm_local(function, *discriminant));
                wasm_i64_const(output, *value as i64);
                output.push(0x51);
                output.extend_from_slice(&[0x04, 0x40]);
                wasm_i64_const(output, *target as i64);
                wasm_local_set(output, block_local);
                output.push(0x0b);
            }
            output.extend_from_slice(&[0x0c, 0x01]);
        }
        PortableTerminator::Call {
            function: callee,
            args,
            destination,
            target,
        } => {
            for arg in args {
                match arg {
                    PortableArg::Local(local) => {
                        wasm_local_get(output, wasm_local(function, *local))
                    }
                    PortableArg::Constant(value) => wasm_i64_const(output, *value),
                }
            }
            output.push(0x10);
            write_uleb(output, *callee as u64);
            wasm_local_set(output, wasm_local(function, *destination));
            wasm_transition(
                output,
                current_block,
                *target,
                block_local,
                previous_block_local,
            );
        }
        PortableTerminator::Unreachable => output.push(0x00),
    }
    Ok(())
}

fn wasm_transition(
    output: &mut Vec<u8>,
    current: u32,
    target: u32,
    block_local: u32,
    previous_block_local: u32,
) {
    wasm_i64_const(output, current as i64);
    wasm_local_set(output, previous_block_local);
    wasm_i64_const(output, target as i64);
    wasm_local_set(output, block_local);
    output.extend_from_slice(&[0x0c, 0x01]);
}

fn encode_wasm_binary(
    output: &mut Vec<u8>,
    function: &PortableFunction,
    op: PortableBinary,
    left: u32,
    right: u32,
) {
    match op {
        PortableBinary::LogAnd | PortableBinary::LogOr => {
            wasm_local_get(output, wasm_local(function, left));
            output.extend_from_slice(&[0x50, 0x45]);
            wasm_local_get(output, wasm_local(function, right));
            output.extend_from_slice(&[0x50, 0x45]);
            output.push(if matches!(op, PortableBinary::LogAnd) {
                0x71
            } else {
                0x72
            });
            output.push(0xad);
            return;
        }
        _ => {
            wasm_local_get(output, wasm_local(function, left));
            wasm_local_get(output, wasm_local(function, right));
        }
    }
    let opcode = match op {
        PortableBinary::Add => 0x7c,
        PortableBinary::Sub => 0x7d,
        PortableBinary::Mul => 0x7e,
        PortableBinary::Div => 0x7f,
        PortableBinary::Rem => 0x81,
        PortableBinary::BitAnd => 0x83,
        PortableBinary::BitOr => 0x84,
        PortableBinary::BitXor => 0x85,
        PortableBinary::Shl => 0x86,
        PortableBinary::Shr => 0x87,
        PortableBinary::Eq => 0x51,
        PortableBinary::Ne => 0x52,
        PortableBinary::Lt => 0x53,
        PortableBinary::Gt => 0x55,
        PortableBinary::Le => 0x57,
        PortableBinary::Ge => 0x59,
        PortableBinary::LogAnd | PortableBinary::LogOr => unreachable!(),
    };
    output.push(opcode);
    if matches!(
        op,
        PortableBinary::Eq
            | PortableBinary::Ne
            | PortableBinary::Lt
            | PortableBinary::Gt
            | PortableBinary::Le
            | PortableBinary::Ge
    ) {
        output.push(0xad);
    }
}

fn wasm_local(function: &PortableFunction, mir_local: u32) -> u32 {
    function.param_count + mir_local
}

fn wasm_local_get(output: &mut Vec<u8>, local: u32) {
    output.push(0x20);
    write_uleb(output, local as u64);
}

fn wasm_local_set(output: &mut Vec<u8>, local: u32) {
    output.push(0x21);
    write_uleb(output, local as u64);
}

fn wasm_i64_const(output: &mut Vec<u8>, value: i64) {
    output.push(0x42);
    write_sleb(output, value);
}

fn write_wasm_section(module: &mut Vec<u8>, id: u8, payload: &[u8]) {
    module.push(id);
    write_uleb(module, payload.len() as u64);
    module.extend_from_slice(payload);
}

fn write_wasm_name(output: &mut Vec<u8>, name: &str) {
    write_uleb(output, name.len() as u64);
    output.extend_from_slice(name.as_bytes());
}

fn write_uleb(output: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_sleb(output: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        output.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_bytecode, encode_bytecode, execute_bytecode, validate_portable_abi_versions,
        PortableProgram, PORTABLE_RUNTIME_ABI_VERSION,
    };
    use sengoo_compiler::MIR_SEMANTIC_ABI_VERSION;
    use sengoo_compiler::mir::{
        Instruction, LocalKind, MIRType, MirConstant, MirFunction, Terminator,
    };

    #[test]
    fn bytecode_roundtrip_executes_stable_binary_format() {
        let mut main = MirFunction::new("main".to_string(), Vec::new(), MIRType::Int(64));
        let value = main.add_local(LocalKind::Temp, MIRType::Int(64));
        main.push_inst_to_block(
            0,
            Instruction::Assign {
                destination: value,
                value: MirConstant::Int(42),
            },
        );
        main.block_mut(0)
            .unwrap()
            .set_terminator(Terminator::Return(Some(value)));
        let program =
            PortableProgram::from_mir(super::PortableBackendTarget::Bytecode, &[main]).unwrap();
        let encoded = encode_bytecode(&program).unwrap();
        assert!(encoded.starts_with(b"SGB1"));
        let decoded = decode_bytecode(&encoded).unwrap();
        assert_eq!(execute_bytecode(&decoded).unwrap(), 42);
    }

    #[test]
    fn portable_backends_reject_unknown_mir_and_runtime_abi_versions() {
        let mir_error = validate_portable_abi_versions(
            MIR_SEMANTIC_ABI_VERSION + 1,
            PORTABLE_RUNTIME_ABI_VERSION,
        )
        .expect_err("unknown MIR semantic ABI must fail")
        .to_string();
        assert!(mir_error.contains("unsupported-mir-semantic-abi"));

        let runtime_error = validate_portable_abi_versions(
            MIR_SEMANTIC_ABI_VERSION,
            PORTABLE_RUNTIME_ABI_VERSION + 1,
        )
        .expect_err("unknown portable runtime ABI must fail")
        .to_string();
        assert!(runtime_error.contains("unsupported-portable-runtime-abi"));
    }
}
