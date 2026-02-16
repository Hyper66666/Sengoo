//! Sengoo CLI compiler (`sgc`).

use clap::{Parser as ClapParser, Subcommand, ValueEnum};
use miette::{IntoDiagnostic, Result};
use sengoo_compiler::mir::{
    Instruction as MirInstruction, MirFunction, Terminator as MirTerminator,
};
use sengoo_compiler::{
    lower_ast, lower_hir, ClassMember, Codegen, Decl, DeclKind, Expr, ExprKind, Function, Import,
    ImportKind, MirOptLevel, Param, Parser, Path as AstPath, SelfParam, Span, Stmt, StmtKind,
    TraitBound, TraitItem, Type, TypeChecker, TypeKind, VariantField, Visibility,
};
use sengoo_runtime::{
    ReflectionRuntime, ReflectionSymbolMetadata as RuntimeReflectionSymbolMetadata,
};
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::DefaultHasher, BTreeMap, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI8, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RunEngine {
    Auto,
    Native,
    Lli,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReflectionMode {
    Auto,
    On,
    Off,
}

impl Default for ReflectionMode {
    fn default() -> Self {
        Self::Auto
    }
}

const BUILD_GRAPH_SCHEMA_VERSION: u32 = 4;
const DAEMON_PROTOCOL_VERSION: u32 = 1;
const REFLECTION_SCHEMA_VERSION: u32 = 1;
const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:48765";
const DAEMON_CONNECT_TIMEOUT: Duration = Duration::from_millis(1200);
const LINKER_UNKNOWN: i8 = -1;
const LINKER_UNAVAILABLE: i8 = 0;
const LINKER_AVAILABLE: i8 = 1;

static LLD_AVAILABILITY: AtomicI8 = AtomicI8::new(LINKER_UNKNOWN);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ReflectionCliOptions {
    mode: ReflectionMode,
    enabled: bool,
    modules: Vec<String>,
    symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionSymbolMetadata {
    symbol: String,
    signature: String,
    #[serde(default)]
    native_symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionModuleMetadata {
    module_id: String,
    #[serde(default)]
    symbols: Vec<ReflectionSymbolMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReflectionMetadata {
    #[serde(default = "default_reflection_schema_version")]
    schema_version: u32,
    compiler_version: String,
    #[serde(default)]
    compatible_compiler_versions: Vec<String>,
    root_module: String,
    #[serde(default)]
    modules: Vec<ReflectionModuleMetadata>,
}

fn default_reflection_schema_version() -> u32 {
    REFLECTION_SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonRequest {
    protocol_version: u32,
    client_version: String,
    command: DaemonCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonCommand {
    Build {
        input: String,
        output: Option<String>,
        opt_level: u8,
        emit_llvm: bool,
        force_rebuild: bool,
        reflect: ReflectionMode,
        reflect_module: Vec<String>,
        reflect_symbol: Vec<String>,
    },
    Run {
        input: String,
        opt_level: u8,
        engine: RunEngine,
        force_rebuild: bool,
        args: Vec<String>,
        reflect: ReflectionMode,
        reflect_module: Vec<String>,
        reflect_symbol: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonResponse {
    protocol_version: u32,
    server_version: String,
    ok: bool,
    recoverable: bool,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonDispatchOutcome {
    Handled,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModuleFingerprint {
    path: String,
    #[serde(default)]
    interface_hash: u64,
    /// Implementation hash (normalized source, comments stripped).
    hash: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct ModuleInvalidationStats {
    #[allow(dead_code)]
    total_modules: u32,
    reused_modules: u32,
    rebuilt_modules: u32,
    interface_changed_modules: u32,
    implementation_only_changed_modules: u32,
}

#[derive(Debug, Clone)]
struct ModuleSourceInfo {
    source: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendProbeMode {
    FastNoVerify,
    VerifyChangedAndDependents,
    VerifyAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontendModuleCacheEntryV4 {
    module_id: String,
    source_hash: u64,
    parse_hash: u64,
    interface_hash: u64,
    body_hash: u64,
    hir_hash: u64,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    symbols: Vec<FunctionFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontendSessionStoreV4 {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    compiler_version: String,
    root_module: String,
    #[serde(default)]
    modules: Vec<FrontendModuleCacheEntryV4>,
}

#[derive(Debug, Clone)]
struct ModuleGraphSnapshot {
    module_fingerprints: Vec<ModuleFingerprint>,
    module_function_fingerprints: BTreeMap<String, Vec<FunctionFingerprint>>,
    dependency_edges: BTreeMap<String, Vec<String>>,
    reflection_import_modules: Vec<String>,
    diagnostics: Vec<String>,
    frontend_session_store: FrontendSessionStoreV4,
    reused_modules: Vec<String>,
    rebuilt_modules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunCacheMetadata {
    source_hash: u64,
    #[serde(default)]
    root_interface_hash: u64,
    #[serde(default)]
    root_implementation_hash: u64,
    #[serde(default)]
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
    llvm_ir_path: String,
    executable_path: Option<String>,
    #[serde(default)]
    llvm_ir_hash: u64,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    build_graph_v2: Option<BuildGraphV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunCacheKey {
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildCacheMetadata {
    #[serde(default = "default_build_cache_schema_version")]
    cache_schema_version: u32,
    source_hash: u64,
    #[serde(default)]
    root_interface_hash: u64,
    #[serde(default)]
    root_implementation_hash: u64,
    #[serde(default)]
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    llvm_ir_path: String,
    output_path: String,
    #[serde(default)]
    llvm_ir_hash: u64,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    build_graph_v2: Option<BuildGraphV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildCacheKey {
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FunctionFingerprint {
    symbol: String,
    #[serde(default)]
    abi_hash: u64,
    #[serde(default)]
    body_hash: u64,
    #[serde(default)]
    calls: Vec<String>,
    #[serde(default)]
    module_imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSignatureInfo {
    symbol: String,
    signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BuildGraphNodeV2 {
    module_path: String,
    #[serde(default)]
    interface_hash: u64,
    implementation_hash: u64,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    object_path: Option<String>,
    #[serde(default)]
    functions: Vec<FunctionFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BuildGraphV2 {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    root_module: String,
    #[serde(default)]
    nodes: Vec<BuildGraphNodeV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EditClass {
    Noop,
    ImplOnly,
    InterfaceChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncrementalLinkMode {
    Auto,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkerMode {
    Auto,
    Lld,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CachedNativeRecoveryPlan {
    RelinkFromObject,
    RebuildObjectFromCachedIr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BuildWorksetPlan {
    ReusePreviousArtifacts,
    RebuildImpactedRoot,
    FullRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleChangeKind {
    ImplOnly,
    Interface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionChangeKind {
    ImplOnly,
    Interface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditImpact {
    class: EditClass,
    changed_modules: Vec<String>,
    impacted_modules: Vec<String>,
    changed_functions: Vec<String>,
    impacted_functions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CodegenWorksetManifest {
    #[serde(default = "default_build_graph_schema_version")]
    schema_version: u32,
    root_module: String,
    plan: BuildWorksetPlan,
    #[serde(default)]
    edit_class: Option<EditClass>,
    #[serde(default)]
    changed_modules: Vec<String>,
    #[serde(default)]
    impacted_modules: Vec<String>,
    #[serde(default)]
    changed_symbols: Vec<String>,
    #[serde(default)]
    impacted_symbols: Vec<String>,
    #[serde(default)]
    rebuild_modules: Vec<String>,
    #[serde(default)]
    reuse_modules: Vec<String>,
    #[serde(default)]
    rebuild_symbols: Vec<String>,
    #[serde(default)]
    reuse_symbols: Vec<String>,
}

fn default_build_graph_schema_version() -> u32 {
    BUILD_GRAPH_SCHEMA_VERSION
}

fn default_build_cache_schema_version() -> u32 {
    1
}

#[derive(Debug, Serialize)]
struct BenchCaseResult {
    name: String,
    iterations: u32,
    warmup: u32,
    sample_ms: Vec<f64>,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    phases: Option<BTreeMap<String, f64>>,
    total_ms: Option<f64>,
    before_ms: Option<f64>,
    after_ms: Option<f64>,
    cache_reused_modules: Option<u32>,
}

#[derive(Debug, Serialize)]
struct BenchReport {
    schema_version: u32,
    kind: String,
    suite: String,
    generated_at_unix_ms: u128,
    cases: Vec<BenchCaseResult>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaselineTargets {
    runtime_median_improvement_pct: Option<f64>,
    full_compile_reduction_pct: Option<f64>,
    incremental_compile_reduction_pct: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaselineCase {
    p50_ms: Option<f64>,
    total_ms: Option<f64>,
    before_ms: Option<f64>,
    after_ms: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaseline {
    #[allow(dead_code)]
    schema_version: Option<u32>,
    #[allow(dead_code)]
    updated_at: Option<String>,
    #[serde(default)]
    targets: BenchBaselineTargets,
    #[serde(default)]
    cases: BTreeMap<String, BenchBaselineCase>,
}

/// Sengoo command-line compiler.
#[derive(ClapParser, Debug)]
#[command(name = "sgc")]
#[command(author = "Sengoo Team")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Sengoo language compiler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compile a Sengoo source file.
    Build {
        /// Input source file.
        input: String,

        /// Output file path.
        #[arg(short, long)]
        output: Option<String>,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Emit LLVM IR instead of a native executable.
        #[arg(long)]
        emit_llvm: bool,

        /// Ignore cached build artifacts and rebuild.
        #[arg(long)]
        force_rebuild: bool,

        /// Try dispatching request to local sgc daemon first.
        #[arg(long)]
        daemon: bool,

        /// Daemon address (default: 127.0.0.1:48765).
        #[arg(long)]
        daemon_addr: Option<String>,

        /// Reflection mode (`auto` by default; `--reflect` is shorthand for `--reflect=on`).
        #[arg(
            long,
            value_enum,
            default_value_t = ReflectionMode::Auto,
            num_args = 0..=1,
            default_missing_value = "on"
        )]
        reflect: ReflectionMode,

        /// Restrict reflection to selected module paths (repeatable).
        #[arg(long = "reflect-module")]
        reflect_module: Vec<String>,

        /// Restrict reflection to selected symbols (repeatable).
        #[arg(long = "reflect-symbol")]
        reflect_symbol: Vec<String>,
    },

    /// Run a Sengoo source file.
    Run {
        /// Input source file.
        input: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Runtime engine policy.
        #[arg(long, value_enum, default_value_t = RunEngine::Auto)]
        engine: RunEngine,

        /// Ignore cached run artifacts and rebuild.
        #[arg(long)]
        force_rebuild: bool,

        /// Try dispatching request to local sgc daemon first.
        #[arg(long)]
        daemon: bool,

        /// Daemon address (default: 127.0.0.1:48765).
        #[arg(long)]
        daemon_addr: Option<String>,

        /// Reflection mode (`auto` by default; `--reflect` is shorthand for `--reflect=on`).
        #[arg(
            long,
            value_enum,
            default_value_t = ReflectionMode::Auto,
            num_args = 0..=1,
            default_missing_value = "on"
        )]
        reflect: ReflectionMode,

        /// Restrict reflection to selected module paths (repeatable).
        #[arg(long = "reflect-module")]
        reflect_module: Vec<String>,

        /// Restrict reflection to selected symbols (repeatable).
        #[arg(long = "reflect-symbol")]
        reflect_symbol: Vec<String>,

        /// Arguments passed to program (reserved).
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Type-check/compile without generating final output.
    Check {
        /// Input source file.
        input: String,
    },

    /// Start REPL.
    Repl,

    /// Dump AST.
    DumpAst {
        /// Input source file.
        input: String,
    },

    /// Start persistent compiler daemon.
    Daemon {
        /// Daemon bind/listen address.
        #[arg(long, default_value = DEFAULT_DAEMON_ADDR)]
        addr: String,
    },

    /// Run benchmark suites.
    Bench {
        #[command(subcommand)]
        command: BenchCommands,
    },
}

#[derive(Subcommand, Debug)]
enum BenchCommands {
    /// Runtime-oriented benchmark suite.
    Run {
        /// Suite name or path.
        #[arg(default_value = "runtime")]
        suite: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Warmup runs per case.
        #[arg(long, default_value_t = 1)]
        warmup: u32,

        /// Measured runs per case.
        #[arg(long, default_value_t = 5)]
        iterations: u32,
    },

    /// Full compile benchmark suite.
    Compile {
        /// Suite name or path.
        #[arg(default_value = "compile")]
        suite: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Measured runs per case.
        #[arg(long, default_value_t = 3)]
        iterations: u32,
    },

    /// Incremental compile benchmark suite.
    Incremental {
        /// Suite name or path.
        #[arg(default_value = "incremental")]
        suite: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Measured runs per case.
        #[arg(long, default_value_t = 3)]
        iterations: u32,
    },

    /// Reflection overhead benchmark suite.
    Reflection {
        /// Suite name or path.
        #[arg(default_value = "runtime")]
        suite: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Warmup runs per case.
        #[arg(long, default_value_t = 1)]
        warmup: u32,

        /// Measured runs per case.
        #[arg(long, default_value_t = 5)]
        iterations: u32,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("sgc=info".parse().unwrap());
    fmt().with_env_filter(filter).with_target(false).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            input,
            output,
            opt_level,
            emit_llvm,
            force_rebuild,
            daemon,
            daemon_addr,
            reflect,
            reflect_module,
            reflect_symbol,
        } => {
            if daemon {
                let addr = resolve_daemon_addr(daemon_addr.as_deref());
                let outcome = dispatch_build_via_daemon(
                    &addr,
                    &input,
                    output.as_deref(),
                    opt_level,
                    emit_llvm,
                    force_rebuild,
                    reflect,
                    &reflect_module,
                    &reflect_symbol,
                )
                .await?;
                if matches!(outcome, DaemonDispatchOutcome::Handled) {
                    return Ok(());
                }
            }
            cmd_build(
                &input,
                output.as_deref(),
                opt_level,
                emit_llvm,
                force_rebuild,
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
            )
            .await
        }
        Commands::Run {
            input,
            opt_level,
            engine,
            force_rebuild,
            daemon,
            daemon_addr,
            reflect,
            reflect_module,
            reflect_symbol,
            args,
        } => {
            if daemon {
                let addr = resolve_daemon_addr(daemon_addr.as_deref());
                let outcome = dispatch_run_via_daemon(
                    &addr,
                    &input,
                    opt_level,
                    engine,
                    force_rebuild,
                    &args,
                    reflect,
                    &reflect_module,
                    &reflect_symbol,
                )
                .await?;
                if matches!(outcome, DaemonDispatchOutcome::Handled) {
                    return Ok(());
                }
            }
            cmd_run(
                &input,
                opt_level,
                engine,
                force_rebuild,
                &args,
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
            )
            .await
        }
        Commands::Check { input } => cmd_check(&input).await,
        Commands::Repl => cmd_repl().await,
        Commands::DumpAst { input } => cmd_dump_ast(&input).await,
        Commands::Daemon { addr } => cmd_daemon(&addr).await,
        Commands::Bench { command } => match command {
            BenchCommands::Run {
                suite,
                opt_level,
                warmup,
                iterations,
            } => cmd_bench_run(&suite, opt_level, warmup, iterations).await,
            BenchCommands::Compile {
                suite,
                opt_level,
                iterations,
            } => cmd_bench_compile(&suite, opt_level, iterations).await,
            BenchCommands::Incremental {
                suite,
                opt_level,
                iterations,
            } => cmd_bench_incremental(&suite, opt_level, iterations).await,
            BenchCommands::Reflection {
                suite,
                opt_level,
                warmup,
                iterations,
            } => cmd_bench_reflection(&suite, opt_level, warmup, iterations).await,
        },
    }
}

fn resolve_daemon_addr(explicit: Option<&str>) -> String {
    explicit
        .map(str::to_string)
        .or_else(|| std::env::var("SENGOO_DAEMON_ADDR").ok())
        .unwrap_or_else(|| DEFAULT_DAEMON_ADDR.to_string())
}

fn reflection_options_from_cli(
    mode: ReflectionMode,
    modules: &[String],
    symbols: &[String],
) -> ReflectionCliOptions {
    let mut normalized_modules = modules
        .iter()
        .map(|module| canonical_or_lossy(Path::new(module)))
        .collect::<Vec<_>>();
    normalized_modules.sort();
    normalized_modules.dedup();

    let mut normalized_symbols = symbols
        .iter()
        .map(|symbol| normalize_reflection_symbol_selector(symbol))
        .filter(|symbol| !symbol.is_empty())
        .collect::<Vec<_>>();
    normalized_symbols.sort();
    normalized_symbols.dedup();

    ReflectionCliOptions {
        mode,
        enabled: matches!(mode, ReflectionMode::On),
        modules: normalized_modules,
        symbols: normalized_symbols,
    }
}

fn normalize_reflection_symbol_selector(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(index) = trimmed.find(".sg::") {
        let module_end = index + 3;
        let suffix_start = index + 5;
        if module_end <= trimmed.len() && suffix_start <= trimmed.len() {
            let module = canonical_or_lossy(Path::new(&trimmed[..module_end]));
            let suffix = &trimmed[suffix_start..];
            if !suffix.trim().is_empty() {
                return format!("{}::{}", module, suffix);
            }
        }
    }

    trimmed.to_string()
}

fn import_path_segments_lower(path: &AstPath) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.name.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
}

fn import_decl_requests_reflection(import_decl: &Import) -> bool {
    let segments = import_path_segments_lower(&import_decl.path);
    if segments.is_empty() {
        return false;
    }

    if segments.len() == 1 && segments[0] == "reflect" {
        return true;
    }

    if segments.len() >= 2
        && (segments[0] == "std" || segments[0] == "sengoo")
        && segments[1] == "reflect"
    {
        return true;
    }

    if segments.len() == 1
        && (segments[0] == "std" || segments[0] == "sengoo")
        && matches!(&import_decl.kind, ImportKind::Selective(names) if names
            .iter()
            .any(|name| name.name.eq_ignore_ascii_case("reflect")))
    {
        return true;
    }

    false
}

fn decl_requests_reflection(decl: &Decl) -> bool {
    match &decl.kind {
        DeclKind::Import(import_decl) => import_decl_requests_reflection(import_decl),
        DeclKind::Module(module_decl) => module_decl.items.iter().any(decl_requests_reflection),
        _ => false,
    }
}

fn source_requests_reflection(source: &str) -> bool {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return false,
    };
    program.decls.iter().any(decl_requests_reflection)
}

fn resolve_reflection_options_for_snapshot(
    mut reflection: ReflectionCliOptions,
    snapshot: &ModuleGraphSnapshot,
) -> ReflectionCliOptions {
    reflection.enabled = match reflection.mode {
        ReflectionMode::On => true,
        ReflectionMode::Off => false,
        ReflectionMode::Auto => {
            if !reflection.modules.is_empty() || !reflection.symbols.is_empty() {
                true
            } else {
                !snapshot.reflection_import_modules.is_empty()
            }
        }
    };
    reflection
}

fn reflection_mode_note(reflection: &ReflectionCliOptions, snapshot: &ModuleGraphSnapshot) -> String {
    match reflection.mode {
        ReflectionMode::On => "reflection: forced on (--reflect=on)".to_string(),
        ReflectionMode::Off => "reflection: forced off (--reflect=off)".to_string(),
        ReflectionMode::Auto => {
            if !reflection.enabled {
                return "reflection: auto disabled (no reflect import detected)".to_string();
            }
            if !reflection.modules.is_empty() || !reflection.symbols.is_empty() {
                return "reflection: auto enabled by explicit selector filters".to_string();
            }
            if snapshot.reflection_import_modules.len() == 1 {
                return format!(
                    "reflection: auto enabled by import in {}",
                    snapshot.reflection_import_modules[0]
                );
            }
            format!(
                "reflection: auto enabled by imports in {} module(s)",
                snapshot.reflection_import_modules.len()
            )
        }
    }
}

fn daemon_request_build(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    reflect: ReflectionMode,
    reflect_module: &[String],
    reflect_symbol: &[String],
) -> DaemonRequest {
    DaemonRequest {
        protocol_version: DAEMON_PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        command: DaemonCommand::Build {
            input: input.to_string(),
            output: output.map(str::to_string),
            opt_level,
            emit_llvm,
            force_rebuild,
            reflect,
            reflect_module: reflect_module.to_vec(),
            reflect_symbol: reflect_symbol.to_vec(),
        },
    }
}

fn daemon_request_run(
    input: &str,
    opt_level: u8,
    engine: RunEngine,
    force_rebuild: bool,
    args: &[String],
    reflect: ReflectionMode,
    reflect_module: &[String],
    reflect_symbol: &[String],
) -> DaemonRequest {
    DaemonRequest {
        protocol_version: DAEMON_PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        command: DaemonCommand::Run {
            input: input.to_string(),
            opt_level,
            engine,
            force_rebuild,
            args: args.to_vec(),
            reflect,
            reflect_module: reflect_module.to_vec(),
            reflect_symbol: reflect_symbol.to_vec(),
        },
    }
}

async fn dispatch_build_via_daemon(
    addr: &str,
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    reflect: ReflectionMode,
    reflect_module: &[String],
    reflect_symbol: &[String],
) -> Result<DaemonDispatchOutcome> {
    let request = daemon_request_build(
        input,
        output,
        opt_level,
        emit_llvm,
        force_rebuild,
        reflect,
        reflect_module,
        reflect_symbol,
    );
    dispatch_daemon_request(addr, &request, "build").await
}

async fn dispatch_run_via_daemon(
    addr: &str,
    input: &str,
    opt_level: u8,
    engine: RunEngine,
    force_rebuild: bool,
    args: &[String],
    reflect: ReflectionMode,
    reflect_module: &[String],
    reflect_symbol: &[String],
) -> Result<DaemonDispatchOutcome> {
    let request = daemon_request_run(
        input,
        opt_level,
        engine,
        force_rebuild,
        args,
        reflect,
        reflect_module,
        reflect_symbol,
    );
    dispatch_daemon_request(addr, &request, "run").await
}

async fn dispatch_daemon_request(
    addr: &str,
    request: &DaemonRequest,
    command_label: &str,
) -> Result<DaemonDispatchOutcome> {
    let response = match send_daemon_request(addr, request).await {
        Ok(response) => response,
        Err(reason) => {
            println!("daemon fallback ({}): {}", command_label, reason);
            return Ok(DaemonDispatchOutcome::Fallback);
        }
    };

    if response.ok {
        println!("daemon {}: {}", command_label, response.message);
        return Ok(DaemonDispatchOutcome::Handled);
    }

    if response.recoverable {
        println!("daemon fallback ({}): {}", command_label, response.message);
        return Ok(DaemonDispatchOutcome::Fallback);
    }

    Err(miette::miette!("{}", response.message))
}

async fn send_daemon_request(
    addr: &str,
    request: &DaemonRequest,
) -> std::result::Result<DaemonResponse, String> {
    let stream = timeout(DAEMON_CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| format!("connect timeout to {}", addr))?
        .map_err(|e| format!("connect failed to {}: {}", addr, e))?;

    let (read_half, mut write_half) = stream.into_split();
    let payload = serde_json::to_string(request)
        .map_err(|e| format!("failed to serialize daemon request: {}", e))?;
    write_half
        .write_all(payload.as_bytes())
        .await
        .map_err(|e| format!("failed to send daemon request: {}", e))?;
    write_half
        .write_all(b"\n")
        .await
        .map_err(|e| format!("failed to terminate daemon request: {}", e))?;
    write_half
        .flush()
        .await
        .map_err(|e| format!("failed to flush daemon request: {}", e))?;

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    timeout(DAEMON_CONNECT_TIMEOUT, reader.read_line(&mut line))
        .await
        .map_err(|_| "daemon response timeout".to_string())?
        .map_err(|e| format!("failed to read daemon response: {}", e))?;

    if line.trim().is_empty() {
        return Err("daemon returned empty response".to_string());
    }

    serde_json::from_str::<DaemonResponse>(line.trim())
        .map_err(|e| format!("invalid daemon response: {}", e))
}

async fn cmd_daemon(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| miette::miette!("failed to bind daemon at {}: {}", addr, e))?;
    println!(
        "sgc daemon listening on {} (protocol v{}, server={})",
        addr,
        DAEMON_PROTOCOL_VERSION,
        env!("CARGO_PKG_VERSION")
    );

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|e| miette::miette!("daemon accept failed: {}", e))?;
        tokio::spawn(async move {
            if let Err(err) = handle_daemon_client(stream).await {
                eprintln!("daemon client {} error: {}", peer, err);
            }
        });
    }
}

async fn handle_daemon_client(stream: TcpStream) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    reader.read_line(&mut line).await.into_diagnostic()?;
    if line.trim().is_empty() {
        return Ok(());
    }

    let request: DaemonRequest = match serde_json::from_str(line.trim()) {
        Ok(req) => req,
        Err(err) => {
            let response = DaemonResponse {
                protocol_version: DAEMON_PROTOCOL_VERSION,
                server_version: env!("CARGO_PKG_VERSION").to_string(),
                ok: false,
                recoverable: true,
                message: format!("invalid daemon request: {}", err),
            };
            let encoded = serde_json::to_string(&response).into_diagnostic()?;
            write_half
                .write_all(encoded.as_bytes())
                .await
                .into_diagnostic()?;
            write_half.write_all(b"\n").await.into_diagnostic()?;
            write_half.flush().await.into_diagnostic()?;
            return Ok(());
        }
    };

    let response = execute_daemon_request(request).await;
    let encoded = serde_json::to_string(&response).into_diagnostic()?;
    write_half
        .write_all(encoded.as_bytes())
        .await
        .into_diagnostic()?;
    write_half.write_all(b"\n").await.into_diagnostic()?;
    write_half.flush().await.into_diagnostic()?;
    Ok(())
}

async fn execute_daemon_request(request: DaemonRequest) -> DaemonResponse {
    if request.protocol_version != DAEMON_PROTOCOL_VERSION {
        return DaemonResponse {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: false,
            recoverable: true,
            message: format!(
                "daemon protocol mismatch: client={} server={}",
                request.protocol_version, DAEMON_PROTOCOL_VERSION
            ),
        };
    }

    let result = match request.command {
        DaemonCommand::Build {
            input,
            output,
            opt_level,
            emit_llvm,
            force_rebuild,
            reflect,
            reflect_module,
            reflect_symbol,
        } => {
            cmd_build(
                &input,
                output.as_deref(),
                opt_level,
                emit_llvm,
                force_rebuild,
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
            )
            .await
        }
        DaemonCommand::Run {
            input,
            opt_level,
            engine,
            force_rebuild,
            args,
            reflect,
            reflect_module,
            reflect_symbol,
        } => {
            cmd_run(
                &input,
                opt_level,
                engine,
                force_rebuild,
                &args,
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
            )
            .await
        }
    };

    match result {
        Ok(()) => DaemonResponse {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: true,
            recoverable: false,
            message: "request completed by daemon".to_string(),
        },
        Err(err) => DaemonResponse {
            protocol_version: DAEMON_PROTOCOL_VERSION,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            ok: false,
            recoverable: false,
            message: format!("daemon request failed: {}", err),
        },
    }
}

fn find_runtime_c() -> Option<String> {
    if let Ok(path) = std::env::var("SENGOO_RUNTIME") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(Path::new("."));

        let candidate = exe_dir.join("runtime.c");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }

        if let Some(parent) = exe_dir.parent() {
            if let Some(grandparent) = parent.parent() {
                let candidate = grandparent.join("tools").join("stdlib").join("runtime.c");
                if candidate.exists() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    let candidate = Path::new("tools/stdlib/runtime.c");
    if candidate.exists() {
        return Some(candidate.to_string_lossy().to_string());
    }

    None
}

fn find_tool(tool: &str, windows_candidates: &[&str], unix_candidates: &[&str]) -> Option<String> {
    if std::env::consts::OS == "windows" {
        for path in windows_candidates {
            if Path::new(path).exists() {
                return Some((*path).to_string());
            }
        }

        let exe_name = format!("{}.exe", tool);
        if let Ok(output) = Command::new("where").arg(&exe_name).output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.lines().next().map(|s| s.trim().to_string());
                }
            }
        }
    } else {
        for path in unix_candidates {
            if Path::new(path).exists() {
                return Some((*path).to_string());
            }
        }

        if let Ok(output) = Command::new("which").arg(tool).output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    return path.lines().next().map(|s| s.trim().to_string());
                }
            }
        }
    }

    None
}

fn find_clang() -> Option<String> {
    find_tool(
        "clang",
        &[
            "C:\\Program Files\\LLVM\\bin\\clang.exe",
            "C:\\Program Files (x86)\\LLVM\\bin\\clang.exe",
            "clang.exe",
            "clang",
        ],
        &["clang", "/usr/bin/clang", "/usr/local/bin/clang"],
    )
}

fn find_lli() -> Option<String> {
    find_tool(
        "lli",
        &[
            "C:\\Program Files\\LLVM\\bin\\lli.exe",
            "C:\\Program Files (x86)\\LLVM\\bin\\lli.exe",
            "lli.exe",
            "lli",
        ],
        &["lli", "/usr/bin/lli", "/usr/local/bin/lli"],
    )
}

fn compile_source(source: &str, opt_level: u8) -> std::result::Result<String, String> {
    compile_source_with_phase_timings(source, opt_level)
        .map(|(llvm_ir, _)| llvm_ir)
        .map_err(|e| e.to_string())
}

fn compile_source_with_phase_timings(
    source: &str,
    opt_level: u8,
) -> Result<(String, BTreeMap<String, f64>)> {
    let mut phases = BTreeMap::new();

    let parse_start = Instant::now();
    let program = Parser::parse(source).map_err(|e| miette::miette!("parse failed: {}", e))?;
    phases.insert(
        "parse".to_string(),
        parse_start.elapsed().as_secs_f64() * 1000.0,
    );

    let typeck_start = Instant::now();
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| miette::miette!("typecheck failed: {}", e))?;
    phases.insert(
        "typeck".to_string(),
        typeck_start.elapsed().as_secs_f64() * 1000.0,
    );

    let mir_start = Instant::now();
    let hir_module = lower_ast(&program, checker.env());
    let mut mir_fns = lower_hir(&hir_module.items).map_err(|e| miette::miette!("{}", e))?;
    let mir_opt_level = MirOptLevel::from_u8(opt_level)
        .ok_or_else(|| miette::miette!("invalid optimization level: {}", opt_level))?;
    let pipeline = sengoo_compiler::mir::opt::pipeline_for_level(mir_opt_level);
    pipeline.run(&mut mir_fns);
    phases.insert(
        "mir".to_string(),
        mir_start.elapsed().as_secs_f64() * 1000.0,
    );

    let prune_start = Instant::now();
    prune_unreachable_mir_functions(&mut mir_fns);
    phases.insert(
        "mir_prune".to_string(),
        prune_start.elapsed().as_secs_f64() * 1000.0,
    );

    let codegen_start = Instant::now();
    let mut codegen = Codegen::new();
    let llvm_ir = codegen
        .codegen(&mir_fns)
        .map_err(|e| miette::miette!("codegen failed: {}", e))?;
    phases.insert(
        "codegen".to_string(),
        codegen_start.elapsed().as_secs_f64() * 1000.0,
    );
    phases.insert("link".to_string(), 0.0);

    Ok((llvm_ir, phases))
}

fn prune_unreachable_mir_functions(mir_fns: &mut Vec<MirFunction>) -> usize {
    if mir_fns.len() <= 1 {
        return 0;
    }

    let mut index_by_name = HashMap::new();
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        index_by_name.insert(mir_fn.name.clone(), idx);
    }

    let Some(&main_index) = index_by_name.get("main") else {
        return 0;
    };

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); mir_fns.len()];
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        edges[idx] = collect_mir_call_targets(mir_fn, &index_by_name);
    }

    let mut reachable = vec![false; mir_fns.len()];
    let mut stack = vec![main_index];
    while let Some(idx) = stack.pop() {
        if reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        for &target in &edges[idx] {
            if !reachable[target] {
                stack.push(target);
            }
        }
    }

    let before = mir_fns.len();
    let mut old_fns = std::mem::take(mir_fns);
    old_fns.reverse();
    let mut kept = Vec::with_capacity(before);
    while let Some(mir_fn) = old_fns.pop() {
        if let Some(&idx) = index_by_name.get(&mir_fn.name) {
            if reachable[idx] {
                kept.push(mir_fn);
            }
        }
    }
    let removed = before.saturating_sub(kept.len());
    *mir_fns = kept;
    removed
}

fn collect_mir_call_targets(
    mir_fn: &MirFunction,
    index_by_name: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for block in &mir_fn.basic_blocks {
        for inst in &block.instructions {
            if let MirInstruction::Call { func, .. } = inst {
                if let Some(&idx) = index_by_name.get(func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
        }
        if let Some(MirTerminator::Call { func, .. }) = &block.terminator {
            if let Some(&idx) = index_by_name.get(func) {
                if seen.insert(idx) {
                    targets.push(idx);
                }
            }
        }
    }
    targets
}

fn link_ir_with_clang_ms(
    llvm_ir: &str,
    case_name: &str,
    clang_exe: &str,
    runtime_c: Option<&str>,
    clang_opt_level: u8,
) -> Result<f64> {
    let tmp_dir = bench_root_dir().join("results").join(".tmp");
    fs::create_dir_all(&tmp_dir).into_diagnostic()?;

    let stamp = now_unix_ms();
    let base = sanitize_for_filename(case_name);
    let ll_path = tmp_dir.join(format!("{}-{}.ll", base, stamp));
    let exe_path = if cfg!(windows) {
        tmp_dir.join(format!("{}-{}.exe", base, stamp))
    } else {
        tmp_dir.join(format!("{}-{}", base, stamp))
    };

    fs::write(&ll_path, llvm_ir).into_diagnostic()?;
    let link_start = Instant::now();
    compile_native_binary(clang_exe, &ll_path, &exe_path, runtime_c, clang_opt_level)?;
    let link_ms = link_start.elapsed().as_secs_f64() * 1000.0;

    let _ = fs::remove_file(&ll_path);
    let _ = fs::remove_file(&exe_path);
    Ok(link_ms)
}

fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

fn normalize_source_for_hash(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        let without_comment = line.split("//").next().unwrap_or_default().trim();
        if without_comment.is_empty() {
            continue;
        }
        out.push_str(without_comment);
        out.push('\n');
    }
    out
}

fn implementation_fingerprint(source: &str) -> u64 {
    source_fingerprint(&normalize_source_for_hash(source))
}

fn visibility_label(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub",
        Visibility::Private => "priv",
    }
}

fn ast_path_signature(path: &AstPath) -> String {
    if path.segments.is_empty() {
        return "<empty>".to_string();
    }
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

fn trait_bound_signature(bound: &TraitBound) -> String {
    let mut rendered = ast_path_signature(&bound.path);
    if !bound.params.is_empty() {
        let params = bound
            .params
            .iter()
            .map(type_signature)
            .collect::<Vec<_>>()
            .join(",");
        rendered.push('<');
        rendered.push_str(&params);
        rendered.push('>');
    }
    rendered
}

fn type_signature(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::Path(path) => ast_path_signature(path),
        TypeKind::Tuple(types) => {
            let inner = types
                .iter()
                .map(type_signature)
                .collect::<Vec<_>>()
                .join(",");
            format!("({})", inner)
        }
        TypeKind::Array(elem, len) => format!("[{};{}]", type_signature(elem), len),
        TypeKind::Slice(elem) => format!("[{}]", type_signature(elem)),
        TypeKind::Ptr { base, is_mut } => {
            if *is_mut {
                format!("*mut {}", type_signature(base))
            } else {
                format!("*const {}", type_signature(base))
            }
        }
        TypeKind::Ref { base, is_mut } => {
            if *is_mut {
                format!("&mut {}", type_signature(base))
            } else {
                format!("&{}", type_signature(base))
            }
        }
        TypeKind::Fn { params, ret } => {
            let params_repr = params
                .iter()
                .map(type_signature)
                .collect::<Vec<_>>()
                .join(",");
            match ret {
                Some(ret) => format!("fn({})->{}", params_repr, type_signature(ret)),
                None => format!("fn({})", params_repr),
            }
        }
        TypeKind::Never => "!".to_string(),
        TypeKind::Infer => "_".to_string(),
        TypeKind::Dyn(bounds) => {
            let joined = bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            format!("dyn {}", joined)
        }
        TypeKind::ImplTrait(bounds) => {
            let joined = bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            format!("impl {}", joined)
        }
    }
}

fn param_signature(param: &Param) -> String {
    format!(
        "{}{}:{}",
        if param.is_mut { "mut " } else { "" },
        param.name.name,
        type_signature(&param.ty)
    )
}

fn self_param_signature(self_param: Option<SelfParam>) -> &'static str {
    match self_param {
        Some(SelfParam::Borrowed) => "&self",
        Some(SelfParam::BorrowedMut) => "&mut self",
        Some(SelfParam::Owned) => "self",
        Some(SelfParam::OwnedMut) => "mut self",
        None => "-",
    }
}

fn function_signature(function: &Function) -> String {
    let type_params = function
        .type_params
        .iter()
        .map(|tp| {
            let mut repr = tp.name.name.clone();
            if !tp.bounds.is_empty() {
                let bounds = tp
                    .bounds
                    .iter()
                    .map(trait_bound_signature)
                    .collect::<Vec<_>>()
                    .join("+");
                repr.push(':');
                repr.push_str(&bounds);
            }
            if let Some(default) = &tp.default {
                repr.push('=');
                repr.push_str(&type_signature(default));
            }
            repr
        })
        .collect::<Vec<_>>()
        .join(",");
    let params = function
        .params
        .iter()
        .map(param_signature)
        .collect::<Vec<_>>()
        .join(",");
    let ret = function
        .return_type
        .as_ref()
        .map(type_signature)
        .unwrap_or_else(|| "unit".to_string());
    format!(
        "{}|{}|async={}|self={}|tp=[{}]|params=[{}]|ret={}",
        visibility_label(function.vis),
        function.name.name,
        function.is_async,
        self_param_signature(function.self_param),
        type_params,
        params,
        ret
    )
}

fn variant_field_signature(field: &VariantField) -> String {
    match field {
        VariantField::Named(name, ty) => format!("{}:{}", name.name, type_signature(ty)),
        VariantField::Unnamed(ty) => type_signature(ty),
    }
}

fn append_decl_interface_signature(out: &mut String, decl: &Decl) {
    match &decl.kind {
        DeclKind::Function(function) => {
            out.push_str("fn|");
            out.push_str(&function_signature(function));
            out.push('\n');
        }
        DeclKind::Struct(struct_decl) => {
            let fields = struct_decl
                .fields
                .iter()
                .map(|field| match &field.name {
                    Some(name) => format!(
                        "{}:{}:{}",
                        visibility_label(field.vis),
                        name.name,
                        type_signature(&field.ty)
                    ),
                    None => format!(
                        "{}:_:{}",
                        visibility_label(field.vis),
                        type_signature(&field.ty)
                    ),
                })
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "struct|{}|{}|tp={}|fields=[{}]\n",
                visibility_label(struct_decl.vis),
                struct_decl.name.name,
                struct_decl.type_params.len(),
                fields
            ));
        }
        DeclKind::Enum(enum_decl) => {
            let variants = enum_decl
                .variants
                .iter()
                .map(|variant| {
                    let fields = variant
                        .fields
                        .iter()
                        .map(variant_field_signature)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}({})", variant.name.name, fields)
                })
                .collect::<Vec<_>>()
                .join("|");
            out.push_str(&format!(
                "enum|{}|{}|tp={}|variants=[{}]\n",
                visibility_label(enum_decl.vis),
                enum_decl.name.name,
                enum_decl.type_params.len(),
                variants
            ));
        }
        DeclKind::Class(class_decl) => {
            let members = class_decl
                .members
                .iter()
                .map(|member| match member {
                    ClassMember::Field(field) => match &field.name {
                        Some(name) => format!("field:{}:{}", name.name, type_signature(&field.ty)),
                        None => format!("field:_:{}", type_signature(&field.ty)),
                    },
                    ClassMember::Method(function) => {
                        format!("method:{}", function_signature(function))
                    }
                })
                .collect::<Vec<_>>()
                .join(";");
            let extends = class_decl
                .extends
                .as_ref()
                .map(ast_path_signature)
                .unwrap_or_else(|| "-".to_string());
            let implements = class_decl
                .implements
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            out.push_str(&format!(
                "class|{}|{}|tp={}|extends={}|impl={}|members=[{}]\n",
                visibility_label(class_decl.vis),
                class_decl.name.name,
                class_decl.type_params.len(),
                extends,
                implements,
                members
            ));
        }
        DeclKind::Trait(trait_decl) => {
            let bounds = trait_decl
                .bounds
                .iter()
                .map(trait_bound_signature)
                .collect::<Vec<_>>()
                .join("+");
            let items = trait_decl
                .items
                .iter()
                .map(|item| match item {
                    TraitItem::Function(function) => format!("fn:{}", function_signature(function)),
                    TraitItem::Const(const_decl) => {
                        format!(
                            "const:{}:{}",
                            const_decl.name.name,
                            type_signature(&const_decl.ty)
                        )
                    }
                    TraitItem::Type(alias) => {
                        format!("type:{}={}", alias.name.name, type_signature(&alias.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "trait|{}|{}|tp={}|bounds={}|items=[{}]\n",
                visibility_label(trait_decl.vis),
                trait_decl.name.name,
                trait_decl.type_params.len(),
                bounds,
                items
            ));
        }
        DeclKind::Impl(impl_decl) => {
            let trait_path = impl_decl
                .trait_path
                .as_ref()
                .map(ast_path_signature)
                .unwrap_or_else(|| "-".to_string());
            let methods = impl_decl
                .items
                .iter()
                .map(function_signature)
                .collect::<Vec<_>>()
                .join(";");
            out.push_str(&format!(
                "impl|{}|target={}|trait={}|tp={}|methods=[{}]\n",
                visibility_label(impl_decl.vis),
                type_signature(&impl_decl.target_type),
                trait_path,
                impl_decl.type_params.len(),
                methods
            ));
        }
        DeclKind::TypeAlias(alias) => {
            out.push_str(&format!(
                "type|{}|{}={}\n",
                visibility_label(alias.vis),
                alias.name.name,
                type_signature(&alias.ty)
            ));
        }
        DeclKind::Const(const_decl) => {
            out.push_str(&format!(
                "const|{}|{}:{}\n",
                visibility_label(const_decl.vis),
                const_decl.name.name,
                type_signature(&const_decl.ty)
            ));
        }
        DeclKind::Static(static_decl) => {
            out.push_str(&format!(
                "static|{}|mut={}|{}:{}\n",
                visibility_label(static_decl.vis),
                static_decl.is_mut,
                static_decl.name.name,
                type_signature(&static_decl.ty)
            ));
        }
        DeclKind::Import(import_decl) => {
            let kind = match &import_decl.kind {
                ImportKind::Simple => "simple".to_string(),
                ImportKind::Wildcard => "wildcard".to_string(),
                ImportKind::Selective(names) => format!(
                    "selective:{}",
                    names
                        .iter()
                        .map(|ident| ident.name.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            };
            let alias = import_decl
                .alias
                .as_ref()
                .map(|ident| ident.name.as_str())
                .unwrap_or("-");
            out.push_str(&format!(
                "import|{}|kind={}|alias={}\n",
                ast_path_signature(&import_decl.path),
                kind,
                alias
            ));
        }
        DeclKind::Module(module_decl) => {
            out.push_str(&format!(
                "module|{}|{}|items={}\n",
                visibility_label(module_decl.vis),
                module_decl.name.name,
                module_decl.items.len()
            ));
            for item in &module_decl.items {
                append_decl_interface_signature(out, item);
            }
        }
    }
}

fn ast_interface_signature(source: &str) -> Option<String> {
    let program = Parser::parse(source).ok()?;
    let mut out = String::new();
    for decl in &program.decls {
        append_decl_interface_signature(&mut out, decl);
    }
    Some(out)
}

fn source_span_slice<'a>(source: &'a str, span: Span) -> Option<&'a str> {
    source.get(span.lo as usize..span.hi as usize)
}

fn call_target_signature(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Ident(ident) => Some(ident.name.clone()),
        ExprKind::Path(path) => Some(ast_path_signature(path)),
        _ => None,
    }
}

fn collect_calls_in_expr(expr: &Expr, calls: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Continue => {}
        ExprKind::Unary { operand, .. }
        | ExprKind::Await(operand)
        | ExprKind::Try(operand)
        | ExprKind::Paren(operand) => {
            collect_calls_in_expr(operand, calls);
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::AssignOp {
            target: left,
            value: right,
            ..
        }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_calls_in_expr(left, calls);
            collect_calls_in_expr(right, calls);
        }
        ExprKind::Call { func, args } => {
            if let Some(target) = call_target_signature(func) {
                calls.push(target);
            }
            collect_calls_in_expr(func, calls);
            for arg in args {
                collect_calls_in_expr(arg, calls);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            calls.push(format!("method::{}", method.name));
            collect_calls_in_expr(receiver, calls);
            for arg in args {
                collect_calls_in_expr(arg, calls);
            }
        }
        ExprKind::Block(block)
        | ExprKind::Loop(block)
        | ExprKind::AsyncBlock(block)
        | ExprKind::ParallelBlock(block) => {
            for stmt in &block.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_calls_in_expr(cond, calls);
            for stmt in &then_branch.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
            if let Some(else_expr) = else_branch.as_deref() {
                collect_calls_in_expr(else_expr, calls);
            }
        }
        ExprKind::While { cond, body } => {
            collect_calls_in_expr(cond, calls);
            for stmt in &body.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::For { iter, body, .. } => {
            collect_calls_in_expr(iter, calls);
            for stmt in &body.stmts {
                collect_calls_in_stmt(stmt, calls);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_calls_in_expr(scrutinee, calls);
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_calls_in_expr(guard, calls);
                }
                collect_calls_in_expr(&arm.body, calls);
            }
        }
        ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => {
            if let Some(value) = value.as_deref() {
                collect_calls_in_expr(value, calls);
            }
        }
        ExprKind::Field { base, .. } => {
            collect_calls_in_expr(base, calls);
        }
        ExprKind::Array(elements) | ExprKind::Tuple(elements) => {
            for elem in elements {
                collect_calls_in_expr(elem, calls);
            }
        }
        ExprKind::Struct { fields, base, .. } => {
            for field in fields {
                collect_calls_in_expr(&field.value, calls);
            }
            if let Some(base) = base.as_deref() {
                collect_calls_in_expr(base, calls);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_calls_in_expr(start, calls);
            }
            if let Some(end) = end.as_deref() {
                collect_calls_in_expr(end, calls);
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_calls_in_expr(body, calls);
        }
        ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => {
            collect_calls_in_expr(expr, calls);
        }
    }
}

fn collect_calls_in_stmt(stmt: &Stmt, calls: &mut Vec<String>) {
    match &stmt.kind {
        StmtKind::Let {
            value: Some(value), ..
        } => collect_calls_in_expr(value, calls),
        StmtKind::Const { value, .. } => collect_calls_in_expr(value, calls),
        StmtKind::Expr(expr) => collect_calls_in_expr(expr, calls),
        StmtKind::Item(_) | StmtKind::Let { value: None, .. } => {}
    }
}

fn function_symbol(module_path: &str, scope: &[String], name: &str) -> String {
    let mut parts = Vec::with_capacity(scope.len() + 2);
    parts.push(module_path.to_string());
    parts.extend(scope.iter().cloned());
    parts.push(name.to_string());
    parts.join("::")
}

fn push_function_fingerprint(
    out: &mut Vec<FunctionFingerprint>,
    module_path: &str,
    scope: &[String],
    function: &Function,
    source: &str,
) {
    let abi_hash = source_fingerprint(&function_signature(function));
    let body_hash = source_span_slice(source, function.body.span)
        .map(implementation_fingerprint)
        .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));

    let mut calls = Vec::new();
    for stmt in &function.body.stmts {
        collect_calls_in_stmt(stmt, &mut calls);
    }
    calls.sort();
    calls.dedup();

    out.push(FunctionFingerprint {
        symbol: function_symbol(module_path, scope, &function.name.name),
        abi_hash,
        body_hash,
        calls,
        module_imports: Vec::new(),
    });
}

fn collect_function_fingerprints_from_decl(
    out: &mut Vec<FunctionFingerprint>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
    source: &str,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            push_function_fingerprint(out, module_path, scope, function, source);
        }
        DeclKind::Class(class_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    push_function_fingerprint(out, module_path, &scoped, function, source);
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    push_function_fingerprint(out, module_path, &scoped, function, source);
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                push_function_fingerprint(out, module_path, &scoped, function, source);
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_function_fingerprints_from_decl(out, module_path, &scoped, item, source);
            }
        }
        _ => {}
    }
}

fn function_fingerprints_for_module(module_path: &str, source: &str) -> Vec<FunctionFingerprint> {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return Vec::new(),
    };

    let mut functions = Vec::new();
    for decl in &program.decls {
        collect_function_fingerprints_from_decl(&mut functions, module_path, &[], decl, source);
    }

    let mut simple_to_symbol = HashMap::<String, Option<String>>::new();
    for function in &functions {
        let simple = function
            .symbol
            .rsplit("::")
            .next()
            .unwrap_or_default()
            .to_string();
        match simple_to_symbol.get_mut(&simple) {
            Some(entry) => *entry = None,
            None => {
                simple_to_symbol.insert(simple, Some(function.symbol.clone()));
            }
        }
    }

    for function in &mut functions {
        for call in &mut function.calls {
            if call.contains("::") {
                continue;
            }
            if let Some(Some(symbol)) = simple_to_symbol.get(call) {
                *call = symbol.clone();
            }
        }
        function.calls.sort();
        function.calls.dedup();
    }

    functions.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    functions
}

fn push_function_signature_info(
    out: &mut Vec<FunctionSignatureInfo>,
    module_path: &str,
    scope: &[String],
    function: &Function,
) {
    out.push(FunctionSignatureInfo {
        symbol: function_symbol(module_path, scope, &function.name.name),
        signature: function_signature(function),
    });
}

fn collect_function_signatures_from_decl(
    out: &mut Vec<FunctionSignatureInfo>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            push_function_signature_info(out, module_path, scope, function);
        }
        DeclKind::Class(class_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    push_function_signature_info(out, module_path, &scoped, function);
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    push_function_signature_info(out, module_path, &scoped, function);
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                push_function_signature_info(out, module_path, &scoped, function);
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_function_signatures_from_decl(out, module_path, &scoped, item);
            }
        }
        _ => {}
    }
}

fn function_signatures_for_module(module_path: &str, source: &str) -> Vec<FunctionSignatureInfo> {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return Vec::new(),
    };

    let mut signatures = Vec::new();
    for decl in &program.decls {
        collect_function_signatures_from_decl(&mut signatures, module_path, &[], decl);
    }
    signatures.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    signatures.dedup_by(|a, b| a.symbol == b.symbol);
    signatures
}

fn reflection_sidecar_path_for_artifact(artifact_path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}.sgreflect.json",
        artifact_path.to_string_lossy()
    ))
}

fn llvm_defined_function_names(llvm_ir: &str) -> HashSet<String> {
    let mut symbols = HashSet::new();
    for line in llvm_ir.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("define ") {
            continue;
        }
        let Some(at_index) = trimmed.find('@') else {
            continue;
        };
        let after_at = &trimmed[at_index + 1..];
        let Some(paren_index) = after_at.find('(') else {
            continue;
        };
        let mut symbol = after_at[..paren_index].trim().to_string();
        if let Some(unquoted) = symbol
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        {
            symbol = unquoted.to_string();
        }
        if !symbol.is_empty() {
            symbols.insert(symbol);
        }
    }
    symbols
}

fn read_llvm_defined_function_names(path: &Path) -> Result<HashSet<String>> {
    let llvm_ir = fs::read_to_string(path).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to read LLVM IR for reflection metadata {}: {}",
            path.to_string_lossy(),
            e
        )
    })?;
    Ok(llvm_defined_function_names(&llvm_ir))
}

fn validate_reflection_metadata(metadata: &ReflectionMetadata) -> Result<()> {
    if metadata.schema_version != REFLECTION_SCHEMA_VERSION {
        return Err(miette::miette!(
            "reflection metadata schema mismatch: expected {} got {}",
            REFLECTION_SCHEMA_VERSION,
            metadata.schema_version
        ));
    }
    if metadata.compiler_version.trim().is_empty() {
        return Err(miette::miette!(
            "reflection metadata missing compiler_version"
        ));
    }
    if metadata.compatible_compiler_versions.is_empty() {
        return Err(miette::miette!(
            "reflection metadata missing compatible_compiler_versions"
        ));
    }
    if metadata
        .compatible_compiler_versions
        .iter()
        .any(|version| version.trim().is_empty())
    {
        return Err(miette::miette!(
            "reflection metadata contains empty compatible compiler version"
        ));
    }
    if metadata.root_module.trim().is_empty() {
        return Err(miette::miette!("reflection metadata missing root_module"));
    }

    let mut module_ids = HashSet::<String>::new();
    for module in &metadata.modules {
        if module.module_id.trim().is_empty() {
            return Err(miette::miette!(
                "reflection metadata contains empty module id"
            ));
        }
        if !module_ids.insert(module.module_id.clone()) {
            return Err(miette::miette!(
                "reflection metadata contains duplicate module id: {}",
                module.module_id
            ));
        }

        let mut symbol_ids = HashSet::<String>::new();
        for symbol in &module.symbols {
            if symbol.symbol.trim().is_empty() {
                return Err(miette::miette!(
                    "reflection metadata contains empty symbol in module {}",
                    module.module_id
                ));
            }
            if symbol.signature.trim().is_empty() {
                return Err(miette::miette!(
                    "reflection metadata contains empty signature for symbol {}",
                    symbol.symbol
                ));
            }
            if let Some(native_symbol) = &symbol.native_symbol {
                if native_symbol.trim().is_empty() {
                    return Err(miette::miette!(
                        "reflection metadata contains empty native symbol for {}",
                        symbol.symbol
                    ));
                }
            }
            if !symbol
                .symbol
                .starts_with(&(module.module_id.clone() + "::"))
            {
                return Err(miette::miette!(
                    "reflection symbol {} does not belong to module {}",
                    symbol.symbol,
                    module.module_id
                ));
            }
            if !symbol_ids.insert(symbol.symbol.clone()) {
                return Err(miette::miette!(
                    "reflection metadata contains duplicate symbol {} in module {}",
                    symbol.symbol,
                    module.module_id
                ));
            }
        }
    }
    Ok(())
}

fn build_reflection_metadata(
    graph_v2: &BuildGraphV2,
    reflection: &ReflectionCliOptions,
    llvm_defined_symbols: Option<&HashSet<String>>,
) -> Result<Option<ReflectionMetadata>> {
    if !reflection.enabled {
        return Ok(None);
    }

    let available_modules = graph_v2
        .nodes
        .iter()
        .map(|node| node.module_path.clone())
        .collect::<HashSet<_>>();
    let mut selected_modules = if !reflection.modules.is_empty() {
        reflection.modules.clone()
    } else if !reflection.symbols.is_empty() {
        available_modules.iter().cloned().collect::<Vec<_>>()
    } else {
        vec![graph_v2.root_module.clone()]
    };
    selected_modules.sort();
    selected_modules.dedup();

    for module in &selected_modules {
        if !available_modules.contains(module) {
            return Err(miette::miette!(
                "reflection module not found in build graph: {}",
                module
            ));
        }
    }

    let mut selected_full_symbols = HashSet::<String>::new();
    let mut selected_short_symbols = HashSet::<String>::new();
    for selector in &reflection.symbols {
        if selector.contains("::") {
            selected_full_symbols.insert(selector.clone());
        } else {
            selected_short_symbols.insert(selector.clone());
        }
    }
    let filter_by_symbol = !selected_full_symbols.is_empty() || !selected_short_symbols.is_empty();
    let mut unresolved_full_symbols = selected_full_symbols.clone();
    let mut unresolved_short_symbols = selected_short_symbols.clone();

    let mut modules = Vec::new();
    for module in selected_modules {
        let source = fs::read_to_string(&module).into_diagnostic().map_err(|e| {
            miette::miette!(
                "failed to read module for reflection metadata {}: {}",
                module,
                e
            )
        })?;
        let mut signatures = function_signatures_for_module(&module, &source)
            .into_iter()
            .map(|entry| ReflectionSymbolMetadata {
                symbol: entry.symbol,
                signature: entry.signature,
                native_symbol: None,
            })
            .collect::<Vec<_>>();

        if filter_by_symbol {
            signatures.retain(|entry| {
                let mut matched = false;
                if selected_full_symbols.contains(&entry.symbol) {
                    matched = true;
                }
                let short = entry.symbol.rsplit("::").next().unwrap_or_default();
                if selected_short_symbols.contains(short) {
                    matched = true;
                }
                matched
            });
        }
        signatures.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        signatures.dedup_by(|a, b| a.symbol == b.symbol);

        if let Some(llvm_defined_symbols) = llvm_defined_symbols {
            let mut short_counts = HashMap::<String, usize>::new();
            for entry in &signatures {
                let short = entry
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string();
                *short_counts.entry(short).or_insert(0) += 1;
            }

            let mut filtered = Vec::new();
            for mut entry in signatures {
                let short = entry
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let explicitly_selected = selected_full_symbols.contains(&entry.symbol)
                    || selected_short_symbols.contains(&short);

                if short_counts.get(&short).copied().unwrap_or_default() > 1 {
                    if explicitly_selected {
                        return Err(miette::miette!(
                            "reflection symbol {} has ambiguous native binding name {}",
                            entry.symbol,
                            short
                        ));
                    }
                    continue;
                }

                if llvm_defined_symbols.contains(&short) {
                    entry.native_symbol = Some(short.clone());
                    unresolved_full_symbols.remove(&entry.symbol);
                    unresolved_short_symbols.remove(&short);
                    filtered.push(entry);
                } else if explicitly_selected {
                    return Err(miette::miette!(
                        "reflection symbol {} is not emitted in LLVM IR (native symbol: {})",
                        entry.symbol,
                        short
                    ));
                }
            }
            signatures = filtered;
        } else {
            for entry in &signatures {
                unresolved_full_symbols.remove(&entry.symbol);
                let short = entry.symbol.rsplit("::").next().unwrap_or_default();
                unresolved_short_symbols.remove(short);
            }
        }

        if !filter_by_symbol || !signatures.is_empty() {
            modules.push(ReflectionModuleMetadata {
                module_id: module,
                symbols: signatures,
            });
        }
    }

    if !unresolved_full_symbols.is_empty() || !unresolved_short_symbols.is_empty() {
        let mut unresolved = unresolved_full_symbols
            .into_iter()
            .chain(unresolved_short_symbols)
            .collect::<Vec<_>>();
        unresolved.sort();
        return Err(miette::miette!(
            "reflection symbol(s) not found in selected modules: {}",
            unresolved.join(", ")
        ));
    }

    modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

    let metadata = ReflectionMetadata {
        schema_version: REFLECTION_SCHEMA_VERSION,
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        compatible_compiler_versions: vec![env!("CARGO_PKG_VERSION").to_string()],
        root_module: graph_v2.root_module.clone(),
        modules,
    };
    validate_reflection_metadata(&metadata)?;
    Ok(Some(metadata))
}

fn maybe_emit_reflection_sidecar(
    artifact_path: &Path,
    graph_v2: &BuildGraphV2,
    reflection: &ReflectionCliOptions,
    llvm_ir_path: Option<&Path>,
) -> Result<()> {
    let sidecar_path = reflection_sidecar_path_for_artifact(artifact_path);
    if !reflection.enabled {
        if sidecar_path.exists() {
            fs::remove_file(&sidecar_path)
                .into_diagnostic()
                .map_err(|e| {
                    miette::miette!(
                        "failed to remove stale reflection metadata {}: {}",
                        sidecar_path.to_string_lossy(),
                        e
                    )
                })?;
        }
        return Ok(());
    }

    let llvm_defined_symbols = if let Some(llvm_ir_path) = llvm_ir_path {
        Some(read_llvm_defined_function_names(llvm_ir_path)?)
    } else {
        None
    };

    let Some(metadata) =
        build_reflection_metadata(graph_v2, reflection, llvm_defined_symbols.as_ref())?
    else {
        return Ok(());
    };
    let bytes = serde_json::to_vec_pretty(&metadata)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to serialize reflection metadata sidecar: {}", e))?;
    fs::write(&sidecar_path, bytes)
        .into_diagnostic()
        .map_err(|e| {
            miette::miette!(
                "failed to write reflection metadata sidecar {}: {}",
                sidecar_path.to_string_lossy(),
                e
            )
        })?;
    println!("Reflection metadata: {}", sidecar_path.to_string_lossy());
    Ok(())
}

fn interface_fingerprint(source: &str) -> u64 {
    if let Some(interface_repr) = ast_interface_signature(source) {
        return source_fingerprint(&interface_repr);
    }

    let normalized = normalize_source_for_hash(source);
    let mut fallback_repr = String::new();
    for line in normalized.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("import ")
            || trimmed.starts_with("pub import ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("pub def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("impl ")
        {
            fallback_repr.push_str(trimmed);
            fallback_repr.push('\n');
        }
    }
    source_fingerprint(&fallback_repr)
}

fn module_invalidation_stats(
    before: &[ModuleFingerprint],
    after: &[ModuleFingerprint],
) -> ModuleInvalidationStats {
    let before_map: HashMap<&str, &ModuleFingerprint> =
        before.iter().map(|fp| (fp.path.as_str(), fp)).collect();
    let after_map: HashMap<&str, &ModuleFingerprint> =
        after.iter().map(|fp| (fp.path.as_str(), fp)).collect();

    let mut all_paths = HashSet::new();
    all_paths.extend(before_map.keys().copied());
    all_paths.extend(after_map.keys().copied());

    let mut stats = ModuleInvalidationStats {
        total_modules: all_paths.len() as u32,
        ..Default::default()
    };

    for path in all_paths {
        match (before_map.get(path), after_map.get(path)) {
            (Some(old), Some(new)) => {
                if old.hash == new.hash && old.interface_hash == new.interface_hash {
                    stats.reused_modules += 1;
                } else if old.interface_hash != new.interface_hash {
                    stats.rebuilt_modules += 1;
                    stats.interface_changed_modules += 1;
                } else {
                    stats.rebuilt_modules += 1;
                    stats.implementation_only_changed_modules += 1;
                }
            }
            _ => {
                stats.rebuilt_modules += 1;
                stats.interface_changed_modules += 1;
            }
        }
    }

    stats
}

fn collect_module_changes(
    before: &[ModuleFingerprint],
    after: &[ModuleFingerprint],
) -> Vec<(String, ModuleChangeKind)> {
    let before_map: HashMap<&str, &ModuleFingerprint> =
        before.iter().map(|fp| (fp.path.as_str(), fp)).collect();
    let after_map: HashMap<&str, &ModuleFingerprint> =
        after.iter().map(|fp| (fp.path.as_str(), fp)).collect();

    let mut all_paths = HashSet::new();
    all_paths.extend(before_map.keys().copied());
    all_paths.extend(after_map.keys().copied());

    let mut changes = Vec::new();
    for path in all_paths {
        let change = match (before_map.get(path), after_map.get(path)) {
            (Some(old), Some(new))
                if old.hash == new.hash && old.interface_hash == new.interface_hash =>
            {
                None
            }
            (Some(old), Some(new)) if old.interface_hash == new.interface_hash => {
                Some(ModuleChangeKind::ImplOnly)
            }
            (Some(_), Some(_)) => Some(ModuleChangeKind::Interface),
            _ => Some(ModuleChangeKind::Interface),
        };

        if let Some(kind) = change {
            changes.push((path.to_string(), kind));
        }
    }

    changes
}

fn add_reverse_edges(graph: &BuildGraphV2, reverse: &mut HashMap<String, HashSet<String>>) {
    for node in &graph.nodes {
        for dep in &node.depends_on {
            reverse
                .entry(dep.clone())
                .or_default()
                .insert(node.module_path.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionNodeState {
    module_path: String,
    abi_hash: u64,
    body_hash: u64,
}

fn collect_function_state(graph: &BuildGraphV2) -> HashMap<String, FunctionNodeState> {
    let mut out = HashMap::new();
    for node in &graph.nodes {
        for function in &node.functions {
            out.insert(
                function.symbol.clone(),
                FunctionNodeState {
                    module_path: node.module_path.clone(),
                    abi_hash: function.abi_hash,
                    body_hash: function.body_hash,
                },
            );
        }
    }
    out
}

fn collect_function_changes(
    previous_graph: Option<&BuildGraphV2>,
    current_graph: &BuildGraphV2,
) -> Vec<(String, FunctionChangeKind)> {
    let previous = previous_graph
        .map(collect_function_state)
        .unwrap_or_default();
    let current = collect_function_state(current_graph);

    let mut all_symbols = HashSet::new();
    all_symbols.extend(previous.keys().cloned());
    all_symbols.extend(current.keys().cloned());

    let mut changes = Vec::new();
    for symbol in all_symbols {
        let change = match (previous.get(&symbol), current.get(&symbol)) {
            (Some(prev), Some(curr))
                if prev.abi_hash == curr.abi_hash && prev.body_hash == curr.body_hash =>
            {
                None
            }
            (Some(prev), Some(curr)) if prev.abi_hash == curr.abi_hash => {
                Some(FunctionChangeKind::ImplOnly)
            }
            (Some(_), Some(_)) => Some(FunctionChangeKind::Interface),
            _ => Some(FunctionChangeKind::Interface),
        };
        if let Some(kind) = change {
            changes.push((symbol, kind));
        }
    }

    changes.sort_by(|a, b| a.0.cmp(&b.0));
    changes
}

fn collect_impl_only_impacted_symbols(
    previous_symbols: &[FunctionFingerprint],
    current_symbols: &[FunctionFingerprint],
) -> Vec<String> {
    let previous_map = previous_symbols
        .iter()
        .map(|function| (function.symbol.clone(), function))
        .collect::<HashMap<_, _>>();
    let current_map = current_symbols
        .iter()
        .map(|function| (function.symbol.clone(), function))
        .collect::<HashMap<_, _>>();

    let mut all_symbols = HashSet::<String>::new();
    all_symbols.extend(previous_map.keys().cloned());
    all_symbols.extend(current_map.keys().cloned());

    let mut changed_symbols = Vec::<String>::new();
    let mut fallback_to_full = false;

    for symbol in all_symbols {
        match (previous_map.get(&symbol), current_map.get(&symbol)) {
            (Some(previous), Some(current))
                if previous.abi_hash == current.abi_hash
                    && previous.body_hash == current.body_hash => {}
            (Some(previous), Some(current)) if previous.abi_hash == current.abi_hash => {
                changed_symbols.push(symbol);
            }
            _ => {
                fallback_to_full = true;
                break;
            }
        }
    }

    if fallback_to_full {
        let mut all_current = current_map.keys().cloned().collect::<Vec<_>>();
        all_current.sort();
        all_current.dedup();
        return all_current;
    }

    changed_symbols.sort();
    changed_symbols.dedup();
    if changed_symbols.is_empty() {
        return changed_symbols;
    }

    let current_symbol_set = current_map.keys().cloned().collect::<HashSet<_>>();
    let mut reverse_calls = HashMap::<String, HashSet<String>>::new();
    for function in previous_symbols.iter().chain(current_symbols.iter()) {
        for callee in &function.calls {
            reverse_calls
                .entry(callee.clone())
                .or_default()
                .insert(function.symbol.clone());
        }
    }

    let mut impacted = changed_symbols.clone();
    let mut queue = changed_symbols;
    let mut seen = impacted.iter().cloned().collect::<HashSet<_>>();

    while let Some(symbol) = queue.pop() {
        if let Some(callers) = reverse_calls.get(&symbol) {
            let mut sorted = callers.iter().cloned().collect::<Vec<_>>();
            sorted.sort();
            for caller in sorted {
                if !current_symbol_set.contains(&caller) {
                    continue;
                }
                if seen.insert(caller.clone()) {
                    impacted.push(caller.clone());
                    queue.push(caller);
                }
            }
        }
    }

    impacted.sort();
    impacted.dedup();
    impacted
}

fn add_reverse_call_edges(graph: &BuildGraphV2, reverse: &mut HashMap<String, HashSet<String>>) {
    for node in &graph.nodes {
        for function in &node.functions {
            for callee in &function.calls {
                reverse
                    .entry(callee.clone())
                    .or_default()
                    .insert(function.symbol.clone());
            }
        }
    }
}

fn edit_class_label(class: EditClass) -> &'static str {
    match class {
        EditClass::Noop => "noop",
        EditClass::ImplOnly => "impl_only",
        EditClass::InterfaceChange => "interface_change",
    }
}

fn incremental_link_mode_from_env() -> IncrementalLinkMode {
    match std::env::var("SENGOO_INCREMENTAL_LINK") {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" => IncrementalLinkMode::Off,
            _ => IncrementalLinkMode::Auto,
        },
        Err(_) => IncrementalLinkMode::Auto,
    }
}

fn classify_edit_impact(
    previous_root_interface_hash: u64,
    previous_root_implementation_hash: u64,
    root_interface_hash: u64,
    root_implementation_hash: u64,
    before_modules: &[ModuleFingerprint],
    after_modules: &[ModuleFingerprint],
    previous_graph: Option<&BuildGraphV2>,
    current_graph: &BuildGraphV2,
) -> EditImpact {
    let mut module_changes: Vec<(String, ModuleChangeKind)> = Vec::new();

    if previous_root_interface_hash == 0
        && previous_root_implementation_hash == 0
        && (root_interface_hash != 0 || root_implementation_hash != 0)
    {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::Interface,
        ));
    } else if previous_root_interface_hash != root_interface_hash {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::Interface,
        ));
    } else if previous_root_implementation_hash != root_implementation_hash {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::ImplOnly,
        ));
    }

    module_changes.extend(collect_module_changes(before_modules, after_modules));
    module_changes.sort_by(|a, b| a.0.cmp(&b.0));
    module_changes.dedup_by(|a, b| a.0 == b.0);

    let function_changes = collect_function_changes(previous_graph, current_graph);

    let has_interface_change = module_changes
        .iter()
        .any(|(_, kind)| matches!(kind, ModuleChangeKind::Interface))
        || function_changes
            .iter()
            .any(|(_, kind)| matches!(kind, FunctionChangeKind::Interface));

    let class = if module_changes.is_empty() && function_changes.is_empty() {
        EditClass::Noop
    } else if has_interface_change {
        EditClass::InterfaceChange
    } else {
        EditClass::ImplOnly
    };

    let mut changed_modules = module_changes
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    changed_modules.sort();
    changed_modules.dedup();

    let mut changed_functions = function_changes
        .iter()
        .map(|(symbol, _)| symbol.clone())
        .collect::<Vec<_>>();
    changed_functions.sort();
    changed_functions.dedup();

    let mut impacted_modules = changed_modules.clone();
    let mut impacted_functions = changed_functions.clone();

    if matches!(class, EditClass::InterfaceChange) {
        let mut reverse_modules: HashMap<String, HashSet<String>> = HashMap::new();
        add_reverse_edges(current_graph, &mut reverse_modules);
        if let Some(previous_graph) = previous_graph {
            add_reverse_edges(previous_graph, &mut reverse_modules);
        }

        let mut queue = module_changes
            .iter()
            .filter_map(|(path, kind)| {
                if matches!(kind, ModuleChangeKind::Interface) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut seen: HashSet<String> = impacted_modules.iter().cloned().collect();

        while let Some(node) = queue.pop() {
            if let Some(parents) = reverse_modules.get(&node) {
                let mut sorted = parents.iter().cloned().collect::<Vec<_>>();
                sorted.sort();
                for parent in sorted {
                    if seen.insert(parent.clone()) {
                        impacted_modules.push(parent.clone());
                        queue.push(parent);
                    }
                }
            }
        }

        let mut reverse_calls: HashMap<String, HashSet<String>> = HashMap::new();
        add_reverse_call_edges(current_graph, &mut reverse_calls);
        if let Some(previous_graph) = previous_graph {
            add_reverse_call_edges(previous_graph, &mut reverse_calls);
        }

        let mut function_queue = function_changes
            .iter()
            .filter_map(|(symbol, kind)| {
                if matches!(kind, FunctionChangeKind::Interface) {
                    Some(symbol.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut seen_functions: HashSet<String> = impacted_functions.iter().cloned().collect();

        while let Some(symbol) = function_queue.pop() {
            if let Some(callers) = reverse_calls.get(&symbol) {
                let mut sorted = callers.iter().cloned().collect::<Vec<_>>();
                sorted.sort();
                for caller in sorted {
                    if seen_functions.insert(caller.clone()) {
                        impacted_functions.push(caller.clone());
                        function_queue.push(caller);
                    }
                }
            }
        }
    }

    impacted_modules.sort();
    impacted_modules.dedup();
    impacted_functions.sort();
    impacted_functions.dedup();

    let mut function_to_module = HashMap::<String, String>::new();
    if let Some(previous_graph) = previous_graph {
        for (symbol, state) in collect_function_state(previous_graph) {
            function_to_module
                .entry(symbol)
                .or_insert(state.module_path);
        }
    }
    for (symbol, state) in collect_function_state(current_graph) {
        function_to_module.insert(symbol, state.module_path);
    }

    for symbol in &impacted_functions {
        if let Some(module_path) = function_to_module.get(symbol) {
            impacted_modules.push(module_path.clone());
        }
    }
    impacted_modules.sort();
    impacted_modules.dedup();

    EditImpact {
        class,
        changed_modules,
        impacted_modules,
        changed_functions,
        impacted_functions,
    }
}

fn format_edit_impact_lines(impact: &EditImpact) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "edit classification: {}",
        edit_class_label(impact.class)
    ));
    if !impact.changed_modules.is_empty() {
        lines.push(format!(
            "changed modules: {}",
            impact.changed_modules.join(", ")
        ));
    }
    if !impact.impacted_modules.is_empty() {
        lines.push(format!(
            "impacted modules: {}",
            impact.impacted_modules.join(", ")
        ));
    }
    if !impact.changed_functions.is_empty() {
        lines.push(format!(
            "changed functions: {}",
            impact.changed_functions.join(", ")
        ));
    }
    if !impact.impacted_functions.is_empty() {
        lines.push(format!(
            "impacted functions: {}",
            impact.impacted_functions.join(", ")
        ));
    }
    lines
}

fn resolve_import_candidates(source_dir: &Path, import_path: &AstPath) -> Vec<PathBuf> {
    if import_path.segments.is_empty() {
        return Vec::new();
    }

    let mut joined = PathBuf::new();
    for seg in &import_path.segments {
        joined.push(&seg.name);
    }

    let mut candidates = vec![
        source_dir.join(&joined).with_extension("sg"),
        source_dir.join(&joined).join("mod.sg"),
        source_dir.join(&joined).join("index.sg"),
    ];
    candidates.dedup();
    candidates
}

fn resolve_direct_import_dependencies(source_dir: &Path, source: &str) -> Vec<PathBuf> {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return Vec::new(),
    };

    let mut deps = program
        .decls
        .iter()
        .filter_map(|decl| match &decl.kind {
            DeclKind::Import(import_decl) => Some(import_decl),
            _ => None,
        })
        .filter_map(|import_decl| {
            resolve_import_candidates(source_dir, &import_decl.path)
                .into_iter()
                .find(|p| p.exists())
        })
        .map(|path| fs::canonicalize(&path).unwrap_or(path))
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn collect_module_sources_with_edges(
    input_path: &Path,
    root_source: &str,
) -> BTreeMap<String, ModuleSourceInfo> {
    let root_path = fs::canonicalize(input_path).unwrap_or_else(|_| input_path.to_path_buf());
    let mut queue = vec![(root_path, root_source.to_string())];
    let mut sources = BTreeMap::new();

    while let Some((module_path, source)) = queue.pop() {
        let module_key = canonical_or_lossy(&module_path);
        if sources.contains_key(&module_key) {
            continue;
        }

        let source_dir = module_path.parent().unwrap_or(Path::new("."));
        let deps = resolve_direct_import_dependencies(source_dir, &source);
        let mut dep_keys = deps
            .iter()
            .map(|dep| canonical_or_lossy(dep))
            .collect::<Vec<_>>();
        dep_keys.sort();
        dep_keys.dedup();

        sources.insert(
            module_key.clone(),
            ModuleSourceInfo {
                source: source.clone(),
                depends_on: dep_keys,
            },
        );

        for dep in deps.into_iter().rev() {
            if let Ok(dep_source) = fs::read_to_string(&dep) {
                queue.push((dep, dep_source));
            }
        }
    }

    sources
}

#[allow(dead_code)]
fn module_dependency_levels(dependency_edges: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut indegree = HashMap::<String, usize>::new();
    let mut reverse = HashMap::<String, Vec<String>>::new();

    for node in dependency_edges.keys() {
        indegree.entry(node.clone()).or_insert(0);
    }

    for (node, deps) in dependency_edges {
        let mut unique = deps.clone();
        unique.sort();
        unique.dedup();

        let dep_count = unique
            .iter()
            .filter(|dep| dependency_edges.contains_key(dep.as_str()))
            .count();
        indegree.insert(node.clone(), dep_count);

        for dep in unique {
            indegree.entry(dep.clone()).or_insert(0);
            reverse.entry(dep).or_default().push(node.clone());
        }
    }

    for dependents in reverse.values_mut() {
        dependents.sort();
        dependents.dedup();
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect::<Vec<_>>();
    ready.sort();
    ready.dedup();

    let mut levels = Vec::new();
    let mut processed = HashSet::<String>::new();

    while !ready.is_empty() {
        let batch = ready.clone();
        ready.clear();
        levels.push(batch.clone());

        for node in batch {
            processed.insert(node.clone());
            if let Some(dependents) = reverse.get(&node) {
                for dependent in dependents {
                    if let Some(degree) = indegree.get_mut(dependent) {
                        if *degree > 0 {
                            *degree -= 1;
                        }
                        if *degree == 0 && !processed.contains(dependent) {
                            ready.push(dependent.clone());
                        }
                    }
                }
            }
        }

        ready.sort();
        ready.dedup();
    }

    let mut unresolved = indegree
        .iter()
        .filter_map(|(node, degree)| {
            if *degree > 0 && !processed.contains(node) {
                Some(node.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !unresolved.is_empty() {
        unresolved.sort();
        levels.push(unresolved);
    }

    levels
}

fn frontend_probe_module_full(
    _path: &str,
    source: &str,
) -> std::result::Result<(u64, u64), String> {
    let parsed = Parser::parse(source).map_err(|e| format!("parse failed: {}", e))?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .map_err(|e| format!("typecheck failed: {}", e))?;
    let hir = lower_ast(&parsed, checker.env());
    let _ = lower_hir(&hir.items).map_err(|e| format!("lower failed: {}", e))?;

    Ok((
        interface_fingerprint(source),
        implementation_fingerprint(source),
    ))
}

fn frontend_probe_module_body_only(
    _path: &str,
    source: &str,
    impacted_symbols: &[String],
) -> std::result::Result<(u64, u64), String> {
    let parsed = Parser::parse(source).map_err(|e| format!("parse failed: {}", e))?;

    let checked_function_names = impacted_symbols
        .iter()
        .filter_map(|symbol| symbol.rsplit("::").next())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();

    if checked_function_names.is_empty() {
        return Ok((
            interface_fingerprint(source),
            implementation_fingerprint(source),
        ));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program_with_filtered_function_bodies(&parsed, &checked_function_names)
        .map_err(|e| format!("typecheck failed: {}", e))?;

    Ok((
        interface_fingerprint(source),
        implementation_fingerprint(source),
    ))
}

fn hir_fragment_fingerprint(functions: &[FunctionFingerprint]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for function in functions {
        function.symbol.hash(&mut hasher);
        function.abi_hash.hash(&mut hasher);
        function.body_hash.hash(&mut hasher);
        for call in &function.calls {
            call.hash(&mut hasher);
        }
        for import in &function.module_imports {
            import.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn frontend_cache_entry_for_module(
    module_path: &str,
    info: &ModuleSourceInfo,
) -> FrontendModuleCacheEntryV4 {
    let mut depends_on = info.depends_on.clone();
    depends_on.sort();
    depends_on.dedup();

    let mut symbols = function_fingerprints_for_module(module_path, &info.source);
    for symbol in &mut symbols {
        if symbol.module_imports.is_empty() {
            symbol.module_imports = depends_on.clone();
        }
    }
    symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    FrontendModuleCacheEntryV4 {
        module_id: module_path.to_string(),
        source_hash: source_fingerprint(&info.source),
        parse_hash: source_fingerprint(&info.source),
        interface_hash: interface_fingerprint(&info.source),
        body_hash: implementation_fingerprint(&info.source),
        hir_hash: hir_fragment_fingerprint(&symbols),
        depends_on,
        symbols,
    }
}

fn collect_module_graph_snapshot(
    input_path: &Path,
    source: &str,
    previous_graph: Option<&BuildGraphV2>,
    previous_frontend_session: Option<&FrontendSessionStoreV4>,
    probe_mode: FrontendProbeMode,
) -> ModuleGraphSnapshot {
    let root_module = canonical_or_lossy(input_path);
    let module_sources = collect_module_sources_with_edges(input_path, source);
    let mut reflection_import_modules = module_sources
        .iter()
        .filter_map(|(path, info)| {
            if source_requests_reflection(&info.source) {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    reflection_import_modules.sort();
    reflection_import_modules.dedup();

    let mut dependency_edges = module_sources
        .iter()
        .map(|(path, info)| (path.clone(), info.depends_on.clone()))
        .collect::<BTreeMap<_, _>>();
    dependency_edges.entry(root_module.clone()).or_default();

    let mut diagnostics = Vec::new();
    let mut previous_entry_by_module = HashMap::<String, FrontendModuleCacheEntryV4>::new();
    if let Some(previous) = previous_frontend_session {
        if previous.schema_version != BUILD_GRAPH_SCHEMA_VERSION {
            diagnostics.push(format!(
                "frontend session fallback: schema mismatch ({} -> {})",
                previous.schema_version, BUILD_GRAPH_SCHEMA_VERSION
            ));
        } else if previous.compiler_version != env!("CARGO_PKG_VERSION") {
            diagnostics.push(format!(
                "frontend session fallback: compiler version mismatch ({} -> {})",
                previous.compiler_version,
                env!("CARGO_PKG_VERSION")
            ));
        } else if previous.root_module != root_module {
            diagnostics.push("frontend session fallback: root module changed".to_string());
        } else {
            for module in &previous.modules {
                previous_entry_by_module.insert(module.module_id.clone(), module.clone());
            }
        }
    }

    let mut module_entries = BTreeMap::<String, FrontendModuleCacheEntryV4>::new();
    let mut reused_modules = Vec::new();
    let mut rebuilt_modules = Vec::new();

    for (path, info) in &module_sources {
        let mut expected_depends_on = info.depends_on.clone();
        expected_depends_on.sort();
        expected_depends_on.dedup();
        let source_hash = source_fingerprint(&info.source);

        let reused = previous_entry_by_module.get(path).filter(|previous| {
            previous.source_hash == source_hash && previous.depends_on == expected_depends_on
        });

        if let Some(previous) = reused {
            module_entries.insert(path.clone(), previous.clone());
            reused_modules.push(path.clone());
        } else {
            let current = frontend_cache_entry_for_module(path, info);
            module_entries.insert(path.clone(), current);
            rebuilt_modules.push(path.clone());
        }
    }

    let (verify_full_modules, mut verify_body_symbols) = match probe_mode {
        FrontendProbeMode::FastNoVerify => (HashSet::new(), BTreeMap::<String, Vec<String>>::new()),
        FrontendProbeMode::VerifyAll => (
            module_sources.keys().cloned().collect::<HashSet<_>>(),
            BTreeMap::<String, Vec<String>>::new(),
        ),
        FrontendProbeMode::VerifyChangedAndDependents => {
            let mut full_modules = HashSet::<String>::new();
            let mut body_symbols = BTreeMap::<String, Vec<String>>::new();
            let mut queue = Vec::<String>::new();

            for module in &rebuilt_modules {
                let Some(current) = module_entries.get(module) else {
                    continue;
                };
                let previous = previous_entry_by_module.get(module);
                let interface_changed = previous
                    .map(|entry| entry.interface_hash != current.interface_hash)
                    .unwrap_or(true);

                if interface_changed {
                    if full_modules.insert(module.clone()) {
                        queue.push(module.clone());
                    }
                    continue;
                }

                if let Some(previous) = previous {
                    let impacted =
                        collect_impl_only_impacted_symbols(&previous.symbols, &current.symbols);
                    if !impacted.is_empty() {
                        body_symbols.insert(module.clone(), impacted);
                    }
                }
            }

            if !queue.is_empty() {
                let mut reverse_edges = HashMap::<String, HashSet<String>>::new();
                for (node, deps) in &dependency_edges {
                    for dep in deps {
                        reverse_edges
                            .entry(dep.clone())
                            .or_default()
                            .insert(node.clone());
                    }
                }
                if let Some(previous_graph) = previous_graph {
                    for node in &previous_graph.nodes {
                        for dep in &node.depends_on {
                            reverse_edges
                                .entry(dep.clone())
                                .or_default()
                                .insert(node.module_path.clone());
                        }
                    }
                }

                while let Some(module) = queue.pop() {
                    if let Some(parents) = reverse_edges.get(&module) {
                        let mut sorted = parents.iter().cloned().collect::<Vec<_>>();
                        sorted.sort();
                        for parent in sorted {
                            if full_modules.insert(parent.clone()) {
                                queue.push(parent);
                            }
                        }
                    }
                }
            }

            for module in &full_modules {
                body_symbols.remove(module);
            }

            (full_modules, body_symbols)
        }
    };

    if !verify_full_modules.is_empty() {
        let mut sorted = verify_full_modules.into_iter().collect::<Vec<_>>();
        sorted.sort();
        for module in sorted {
            if let Some(info) = module_sources.get(&module) {
                if let Err(message) = frontend_probe_module_full(&module, &info.source) {
                    diagnostics.push(format!("{}: {}", module, message));
                }
            }
        }
    }

    if !verify_body_symbols.is_empty() {
        let entries = std::mem::take(&mut verify_body_symbols);
        for (module, impacted_symbols) in entries {
            if impacted_symbols.is_empty() {
                continue;
            }
            if let Some(info) = module_sources.get(&module) {
                if let Err(message) =
                    frontend_probe_module_body_only(&module, &info.source, &impacted_symbols)
                {
                    diagnostics.push(format!("{}: {}", module, message));
                }
            }
        }
    }

    diagnostics.sort();
    diagnostics.dedup();

    let mut module_fingerprints = module_entries
        .iter()
        .filter_map(|(path, entry)| {
            if *path == root_module {
                None
            } else {
                Some(ModuleFingerprint {
                    path: path.clone(),
                    interface_hash: entry.interface_hash,
                    hash: entry.body_hash,
                })
            }
        })
        .collect::<Vec<_>>();
    module_fingerprints.sort_by(|a, b| a.path.cmp(&b.path));

    let mut module_function_fingerprints = module_entries
        .iter()
        .map(|(path, entry)| (path.clone(), entry.symbols.clone()))
        .collect::<BTreeMap<_, _>>();
    module_function_fingerprints
        .entry(root_module.clone())
        .or_default();

    let mut frontend_modules = module_entries.into_values().collect::<Vec<_>>();
    frontend_modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

    ModuleGraphSnapshot {
        module_fingerprints,
        module_function_fingerprints,
        dependency_edges,
        reflection_import_modules,
        diagnostics,
        frontend_session_store: FrontendSessionStoreV4 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            root_module,
            modules: frontend_modules,
        },
        reused_modules,
        rebuilt_modules,
    }
}

fn module_fingerprints_for_source(input_path: &Path, source: &str) -> Vec<ModuleFingerprint> {
    collect_module_graph_snapshot(input_path, source, None, None, FrontendProbeMode::VerifyAll)
        .module_fingerprints
}

fn object_file_extension() -> &'static str {
    if cfg!(windows) {
        "obj"
    } else {
        "o"
    }
}

fn canonical_or_lossy(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn build_graph_v2_for_source(
    input_path: &Path,
    module_fingerprints: &[ModuleFingerprint],
    dependency_edges: &BTreeMap<String, Vec<String>>,
    root_object_path: Option<&Path>,
    root_interface_hash: u64,
    root_implementation_hash: u64,
) -> BuildGraphV2 {
    let root_module = canonical_or_lossy(input_path);
    let mut fingerprint_map = module_fingerprints
        .iter()
        .map(|fp| (fp.path.clone(), (fp.interface_hash, fp.hash)))
        .collect::<HashMap<_, _>>();
    fingerprint_map.insert(
        root_module.clone(),
        (root_interface_hash, root_implementation_hash),
    );

    let mut all_paths = HashSet::<String>::new();
    all_paths.insert(root_module.clone());
    all_paths.extend(module_fingerprints.iter().map(|fp| fp.path.clone()));
    for (path, deps) in dependency_edges {
        all_paths.insert(path.clone());
        all_paths.extend(deps.iter().cloned());
    }

    let mut node_paths = all_paths.into_iter().collect::<Vec<_>>();
    node_paths.sort();

    let mut nodes = Vec::with_capacity(node_paths.len());
    for path in node_paths {
        let (interface_hash, implementation_hash) =
            fingerprint_map.get(&path).copied().unwrap_or_default();
        let mut depends_on = dependency_edges.get(&path).cloned().unwrap_or_default();
        depends_on.sort();
        depends_on.dedup();
        nodes.push(BuildGraphNodeV2 {
            module_path: path.clone(),
            interface_hash,
            implementation_hash,
            depends_on,
            object_path: if path == root_module {
                root_object_path.map(canonical_or_lossy)
            } else {
                None
            },
            functions: Vec::new(),
        });
    }

    BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module,
        nodes,
    }
}

fn build_graph_v2_with_function_fingerprints_for_source(
    input_path: &Path,
    module_fingerprints: &[ModuleFingerprint],
    module_function_fingerprints: &BTreeMap<String, Vec<FunctionFingerprint>>,
    dependency_edges: &BTreeMap<String, Vec<String>>,
    root_object_path: Option<&Path>,
    root_interface_hash: u64,
    root_implementation_hash: u64,
) -> BuildGraphV2 {
    let mut graph = build_graph_v2_for_source(
        input_path,
        module_fingerprints,
        dependency_edges,
        root_object_path,
        root_interface_hash,
        root_implementation_hash,
    );

    for node in &mut graph.nodes {
        let mut functions = module_function_fingerprints
            .get(&node.module_path)
            .cloned()
            .unwrap_or_default();
        for function in &mut functions {
            if function.module_imports.is_empty() {
                function.module_imports = node.depends_on.clone();
            }
        }
        functions.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        node.functions = functions;
    }

    graph
}

fn can_use_incremental_link_with_metadata(
    previous: &BuildCacheMetadata,
    llvm_ir_hash: u64,
    object_path: &Path,
    output_path: &str,
    runtime_c: Option<&str>,
    opt_level: u8,
    graph_v2: &BuildGraphV2,
) -> std::result::Result<(), String> {
    if previous.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        return Err("cache schema version changed".to_string());
    }
    if previous.emit_llvm {
        return Err("previous artifact is LLVM-only".to_string());
    }
    if previous.opt_level != opt_level {
        return Err("optimization level changed".to_string());
    }
    if previous.output_path != output_path {
        return Err("output path changed".to_string());
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return Err("runtime linkage input changed".to_string());
    }
    if previous.llvm_ir_hash != llvm_ir_hash {
        return Err("LLVM IR changed".to_string());
    }
    let Some(prev_object) = previous.object_path.as_deref() else {
        return Err("previous object path missing".to_string());
    };
    if !Path::new(prev_object).exists() {
        return Err("previous object artifact missing".to_string());
    }
    if canonical_or_lossy(Path::new(prev_object)) != canonical_or_lossy(object_path) {
        return Err("object path changed".to_string());
    }
    if previous.build_graph_v2.as_ref() != Some(graph_v2) {
        return Err("build graph changed".to_string());
    }
    Ok(())
}

fn can_use_incremental_link_with_run_metadata(
    previous: &RunCacheMetadata,
    llvm_ir_hash: u64,
    object_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    graph_v2: &BuildGraphV2,
) -> std::result::Result<(), String> {
    if previous.opt_level != opt_level {
        return Err("optimization level changed".to_string());
    }
    if previous.requested_engine != requested_engine || previous.resolved_engine != resolved_engine
    {
        return Err("engine selection changed".to_string());
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return Err("runtime linkage input changed".to_string());
    }
    if previous.llvm_ir_hash != llvm_ir_hash {
        return Err("LLVM IR changed".to_string());
    }
    let Some(prev_object) = previous.object_path.as_deref() else {
        return Err("previous object path missing".to_string());
    };
    if !Path::new(prev_object).exists() {
        return Err("previous object artifact missing".to_string());
    }
    if canonical_or_lossy(Path::new(prev_object)) != canonical_or_lossy(object_path) {
        return Err("object path changed".to_string());
    }
    if previous.build_graph_v2.as_ref() != Some(graph_v2) {
        return Err("build graph changed".to_string());
    }
    Ok(())
}

fn resolve_engine(requested: RunEngine, has_clang: bool, has_lli: bool) -> Result<RunEngine> {
    match requested {
        RunEngine::Auto => {
            if has_clang {
                Ok(RunEngine::Native)
            } else if has_lli {
                Ok(RunEngine::Lli)
            } else {
                Err(miette::miette!(
                    "unable to run: neither clang (native) nor lli (JIT) was found"
                ))
            }
        }
        RunEngine::Native => {
            if has_clang {
                Ok(RunEngine::Native)
            } else {
                Err(miette::miette!("compile failed"))
            }
        }
        RunEngine::Lli => {
            if has_lli {
                Ok(RunEngine::Lli)
            } else {
                Err(miette::miette!("compile failed"))
            }
        }
    }
}
fn cache_key(
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<String>,
) -> RunCacheKey {
    RunCacheKey {
        source_hash,
        module_fingerprints,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c,
    }
}

fn build_cache_key(
    source_hash: u64,
    module_fingerprints: Vec<ModuleFingerprint>,
    opt_level: u8,
    emit_llvm: bool,
    runtime_c: Option<String>,
    output_path: String,
) -> BuildCacheKey {
    BuildCacheKey {
        source_hash,
        module_fingerprints,
        opt_level,
        emit_llvm,
        runtime_c,
        output_path,
    }
}

fn metadata_matches(metadata: &RunCacheMetadata, key: &RunCacheKey) -> bool {
    metadata.source_hash == key.source_hash
        && metadata.module_fingerprints == key.module_fingerprints
        && metadata.opt_level == key.opt_level
        && metadata.requested_engine == key.requested_engine
        && metadata.resolved_engine == key.resolved_engine
        && metadata.runtime_c == key.runtime_c
}

fn build_metadata_matches(metadata: &BuildCacheMetadata, key: &BuildCacheKey) -> bool {
    metadata.cache_schema_version == BUILD_GRAPH_SCHEMA_VERSION
        && metadata.source_hash == key.source_hash
        && metadata.module_fingerprints == key.module_fingerprints
        && metadata.opt_level == key.opt_level
        && metadata.emit_llvm == key.emit_llvm
        && metadata.runtime_c == key.runtime_c
        && metadata.output_path == key.output_path
}

fn build_cache_mismatch_reasons(metadata: &BuildCacheMetadata, key: &BuildCacheKey) -> Vec<String> {
    let mut reasons = Vec::new();

    if metadata.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        reasons.push(format!(
            "cache schema version changed ({} -> {})",
            metadata.cache_schema_version, BUILD_GRAPH_SCHEMA_VERSION
        ));
    }
    if metadata.source_hash != key.source_hash {
        reasons.push("source changed".to_string());
    }
    if metadata.module_fingerprints != key.module_fingerprints {
        let stats =
            module_invalidation_stats(&metadata.module_fingerprints, &key.module_fingerprints);
        if stats.interface_changed_modules > 0 {
            reasons.push(format!(
                "module interfaces changed ({} module(s))",
                stats.interface_changed_modules
            ));
        }
        if stats.implementation_only_changed_modules > 0 {
            reasons.push(format!(
                "module implementations changed ({} module(s))",
                stats.implementation_only_changed_modules
            ));
        }
    }
    if metadata.opt_level != key.opt_level {
        reasons.push(format!(
            "optimization level changed ({} -> {})",
            metadata.opt_level, key.opt_level
        ));
    }
    if metadata.emit_llvm != key.emit_llvm {
        reasons.push(format!(
            "emit mode changed (emit_llvm {} -> {})",
            metadata.emit_llvm, key.emit_llvm
        ));
    }
    if metadata.runtime_c != key.runtime_c {
        reasons.push("runtime path changed".to_string());
    }
    if metadata.output_path != key.output_path {
        reasons.push("output path changed".to_string());
    }

    if reasons.is_empty() {
        reasons.push("build cache metadata mismatch".to_string());
    }
    reasons
}

fn derive_build_workset_plan(
    previous: Option<&BuildCacheMetadata>,
    impact: Option<&EditImpact>,
    root_module: &str,
    emit_llvm: bool,
    opt_level: u8,
    output_path: &str,
    runtime_c: Option<&str>,
) -> BuildWorksetPlan {
    let Some(previous) = previous else {
        return BuildWorksetPlan::FullRebuild;
    };
    if previous.cache_schema_version != BUILD_GRAPH_SCHEMA_VERSION {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.emit_llvm != emit_llvm {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.opt_level != opt_level {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.output_path != output_path {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return BuildWorksetPlan::FullRebuild;
    }

    derive_workset_plan_from_impact(impact, root_module)
}

fn derive_run_workset_plan(
    previous: Option<&RunCacheMetadata>,
    impact: Option<&EditImpact>,
    root_module: &str,
    opt_level: u8,
    requested_engine: RunEngine,
    resolved_engine: RunEngine,
    runtime_c: Option<&str>,
) -> BuildWorksetPlan {
    let Some(previous) = previous else {
        return BuildWorksetPlan::FullRebuild;
    };
    if previous.opt_level != opt_level {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.requested_engine != requested_engine || previous.resolved_engine != resolved_engine
    {
        return BuildWorksetPlan::FullRebuild;
    }
    if previous.runtime_c.as_deref() != runtime_c {
        return BuildWorksetPlan::FullRebuild;
    }

    derive_workset_plan_from_impact(impact, root_module)
}

fn derive_workset_plan_from_impact(
    impact: Option<&EditImpact>,
    root_module: &str,
) -> BuildWorksetPlan {
    let Some(impact) = impact else {
        return BuildWorksetPlan::FullRebuild;
    };
    match impact.class {
        EditClass::Noop => BuildWorksetPlan::ReusePreviousArtifacts,
        EditClass::InterfaceChange => BuildWorksetPlan::FullRebuild,
        EditClass::ImplOnly => {
            let touches_root = impact
                .changed_modules
                .iter()
                .chain(impact.impacted_modules.iter())
                .any(|module| module == root_module);
            if touches_root {
                BuildWorksetPlan::RebuildImpactedRoot
            } else {
                BuildWorksetPlan::ReusePreviousArtifacts
            }
        }
    }
}

fn derive_codegen_workset_manifest(
    graph: &BuildGraphV2,
    impact: Option<&EditImpact>,
    plan: BuildWorksetPlan,
) -> CodegenWorksetManifest {
    let mut all_modules = graph
        .nodes
        .iter()
        .map(|node| node.module_path.clone())
        .collect::<Vec<_>>();
    all_modules.push(graph.root_module.clone());
    all_modules.sort();
    all_modules.dedup();

    let mut changed_modules = impact
        .map(|edit| edit.changed_modules.clone())
        .unwrap_or_default();
    changed_modules.sort();
    changed_modules.dedup();

    let mut impacted_modules = impact
        .map(|edit| edit.impacted_modules.clone())
        .unwrap_or_default();
    impacted_modules.sort();
    impacted_modules.dedup();

    let mut changed_symbols = impact
        .map(|edit| edit.changed_functions.clone())
        .unwrap_or_default();
    changed_symbols.sort();
    changed_symbols.dedup();

    let mut impacted_symbols = impact
        .map(|edit| edit.impacted_functions.clone())
        .unwrap_or_default();
    impacted_symbols.sort();
    impacted_symbols.dedup();

    let mut all_symbols = graph
        .nodes
        .iter()
        .flat_map(|node| {
            node.functions
                .iter()
                .map(|function| function.symbol.clone())
        })
        .collect::<Vec<_>>();
    all_symbols.sort();
    all_symbols.dedup();

    let mut rebuild_modules = match plan {
        BuildWorksetPlan::ReusePreviousArtifacts => Vec::new(),
        BuildWorksetPlan::RebuildImpactedRoot => {
            if impacted_modules.is_empty() {
                vec![graph.root_module.clone()]
            } else {
                impacted_modules.clone()
            }
        }
        BuildWorksetPlan::FullRebuild => all_modules.clone(),
    };
    rebuild_modules.sort();
    rebuild_modules.dedup();

    let rebuild_set = rebuild_modules.iter().cloned().collect::<HashSet<_>>();
    let reuse_modules = all_modules
        .iter()
        .filter(|module| !rebuild_set.contains(*module))
        .cloned()
        .collect::<Vec<_>>();

    let mut rebuild_symbols = match plan {
        BuildWorksetPlan::ReusePreviousArtifacts => Vec::new(),
        BuildWorksetPlan::RebuildImpactedRoot => {
            if impacted_symbols.is_empty() {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.module_path == graph.root_module)
                    .map(|node| {
                        node.functions
                            .iter()
                            .map(|function| function.symbol.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            } else {
                impacted_symbols.clone()
            }
        }
        BuildWorksetPlan::FullRebuild => all_symbols.clone(),
    };
    rebuild_symbols.sort();
    rebuild_symbols.dedup();

    let rebuild_symbol_set = rebuild_symbols.iter().cloned().collect::<HashSet<_>>();
    let reuse_symbols = all_symbols
        .iter()
        .filter(|symbol| !rebuild_symbol_set.contains(*symbol))
        .cloned()
        .collect::<Vec<_>>();

    CodegenWorksetManifest {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module: graph.root_module.clone(),
        plan,
        edit_class: impact.map(|edit| edit.class),
        changed_modules,
        impacted_modules,
        changed_symbols,
        impacted_symbols,
        rebuild_modules,
        reuse_modules,
        rebuild_symbols,
        reuse_symbols,
    }
}

fn codegen_workset_manifest_path(build_dir: &Path, stem: &str, command_kind: &str) -> PathBuf {
    build_dir
        .join("workset")
        .join(format!("{}.{}.workset.json", stem, command_kind))
}

fn save_codegen_workset_manifest(path: &Path, manifest: &CodegenWorksetManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|e| miette::miette!("failed to serialize workset manifest: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write workset manifest: {}", e))
}

fn cache_mismatch_reasons(metadata: &RunCacheMetadata, key: &RunCacheKey) -> Vec<String> {
    let mut reasons = Vec::new();

    if metadata.source_hash != key.source_hash {
        reasons.push("source changed".to_string());
    }
    if metadata.module_fingerprints != key.module_fingerprints {
        let stats =
            module_invalidation_stats(&metadata.module_fingerprints, &key.module_fingerprints);
        if stats.interface_changed_modules > 0 {
            reasons.push(format!(
                "module interfaces changed ({} module(s))",
                stats.interface_changed_modules
            ));
        }
        if stats.implementation_only_changed_modules > 0 {
            reasons.push(format!(
                "module implementations changed ({} module(s))",
                stats.implementation_only_changed_modules
            ));
        }
    }
    if metadata.opt_level != key.opt_level {
        reasons.push(format!(
            "optimization level changed ({} -> {})",
            metadata.opt_level, key.opt_level
        ));
    }
    if metadata.requested_engine != key.requested_engine {
        reasons.push(format!(
            "requested engine changed ({:?} -> {:?})",
            metadata.requested_engine, key.requested_engine
        ));
    }
    if metadata.resolved_engine != key.resolved_engine {
        reasons.push(format!(
            "resolved engine changed ({:?} -> {:?})",
            metadata.resolved_engine, key.resolved_engine
        ));
    }
    if metadata.runtime_c != key.runtime_c {
        reasons.push("runtime path changed".to_string());
    }

    if reasons.is_empty() {
        reasons.push("cache metadata mismatch".to_string());
    }

    reasons
}

fn load_run_cache(path: &Path) -> Option<RunCacheMetadata> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn load_build_cache(path: &Path) -> Option<BuildCacheMetadata> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn frontend_session_store_path(build_dir: &Path, stem: &str) -> PathBuf {
    build_dir
        .join("workset")
        .join(format!("{}.frontend-session-v4.json", stem))
}

fn load_frontend_session_store(path: &Path) -> Option<FrontendSessionStoreV4> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_run_cache(path: &Path, metadata: &RunCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize run cache metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write run cache metadata: {}", e))
}

fn save_build_cache(path: &Path, metadata: &BuildCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize build cache metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write build cache metadata: {}", e))
}

fn save_frontend_session_store(path: &Path, metadata: &FrontendSessionStoreV4) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize frontend session metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write frontend session metadata: {}", e))
}

fn runtime_object_cache_path(runtime_c_path: &Path, opt_level: u8) -> Result<PathBuf> {
    let canonical =
        fs::canonicalize(runtime_c_path).unwrap_or_else(|_| runtime_c_path.to_path_buf());
    let meta = fs::metadata(&canonical).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to stat runtime source {}: {}",
            canonical.display(),
            e
        )
    })?;
    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    canonical.to_string_lossy().hash(&mut hasher);
    meta.len().hash(&mut hasher);
    modified_secs.hash(&mut hasher);
    opt_level.hash(&mut hasher);
    let key = hasher.finish();

    let ext = if cfg!(windows) { "obj" } else { "o" };
    let cache_dir = std::env::temp_dir()
        .join("sengoo")
        .join("runtime-obj-cache");
    fs::create_dir_all(&cache_dir).into_diagnostic()?;
    Ok(cache_dir.join(format!("runtime-{}-O{}.{}", key, opt_level, ext)))
}

fn ensure_runtime_object(clang_exe: &str, runtime_c: &str, opt_level: u8) -> Result<PathBuf> {
    let runtime_c_path = Path::new(runtime_c);
    let object_path = runtime_object_cache_path(runtime_c_path, opt_level)?;
    if object_path.exists() {
        return Ok(object_path);
    }

    let status = Command::new(clang_exe)
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level))
        .arg("-c")
        .arg(runtime_c_path)
        .arg("-o")
        .arg(&object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for runtime object: {}", e))?;

    if !status.success() {
        return Err(miette::miette!(
            "compile failed while preparing runtime object cache"
        ));
    }

    Ok(object_path)
}

fn compile_ir_to_object(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    opt_level: u8,
) -> Result<()> {
    let status = Command::new(clang_exe)
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level))
        .arg("-c")
        .arg(llvm_ir_path)
        .arg("-o")
        .arg(object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for object compilation: {}", e))?;

    if !status.success() {
        return Err(miette::miette!("compile failed"));
    }
    Ok(())
}

fn link_native_binary_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
) -> Result<()> {
    let mode = linker_mode_from_env();
    let lld_state = LLD_AVAILABILITY.load(Ordering::Relaxed);
    let try_lld_first = match mode {
        LinkerMode::Lld => true,
        LinkerMode::System => false,
        LinkerMode::Auto => lld_state != LINKER_UNAVAILABLE,
    };

    if try_lld_first {
        let lld_status = run_link_command(clang_exe, object_paths, executable_path, true)?;
        if lld_status.success() {
            if matches!(mode, LinkerMode::Auto) {
                LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
            }
            return Ok(());
        }
        if matches!(mode, LinkerMode::Lld) {
            return Err(miette::miette!("compile failed (lld linker mode)"));
        }
        LLD_AVAILABILITY.store(LINKER_UNAVAILABLE, Ordering::Relaxed);
        println!("link fallback: lld unavailable, retrying with system linker");
    }

    let status = run_link_command(clang_exe, object_paths, executable_path, false)?;
    if !status.success() {
        return Err(miette::miette!("compile failed"));
    }
    Ok(())
}

fn linker_mode_from_env() -> LinkerMode {
    parse_linker_mode(std::env::var("SENGOO_LINKER").ok().as_deref())
}

fn parse_linker_mode(value: Option<&str>) -> LinkerMode {
    let Some(value) = value else {
        return LinkerMode::Auto;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "lld" => LinkerMode::Lld,
        "system" => LinkerMode::System,
        _ => LinkerMode::Auto,
    }
}

fn run_link_command(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    use_lld: bool,
) -> Result<std::process::ExitStatus> {
    let mut clang_cmd = Command::new(clang_exe);
    clang_cmd.arg("-Wno-override-module");
    if use_lld {
        clang_cmd.arg("-fuse-ld=lld");
    }
    for object in object_paths {
        clang_cmd.arg(object);
    }
    clang_cmd.arg("-o").arg(executable_path);
    clang_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang linker: {}", e))
}

fn compile_native_binary(
    clang_exe: &str,
    llvm_ir_path: &Path,
    executable_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
) -> Result<()> {
    let object_path = executable_path.with_extension(object_file_extension());
    compile_ir_to_object(clang_exe, llvm_ir_path, &object_path, opt_level)?;
    let mut object_paths = vec![object_path];
    if let Some(runtime_c) = runtime_c {
        let runtime_obj = ensure_runtime_object(clang_exe, runtime_c, opt_level)?;
        object_paths.push(runtime_obj);
    }
    link_native_binary_from_objects(clang_exe, &object_paths, executable_path)?;
    Ok(())
}

fn run_native_binary(executable_path: &Path) -> Result<()> {
    let run_output = Command::new(executable_path)
        .output()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to execute native binary: {}", e))?;

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    if !stdout.is_empty() {
        print!("{}", stdout);
    }

    let stderr = String::from_utf8_lossy(&run_output.stderr);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    if let Some(code) = run_output.status.code() {
        println!("exit code: {}", code);
    }

    Ok(())
}

fn run_with_lli(lli_exe: &str, llvm_ir_path: &Path) -> Result<()> {
    let output = Command::new(lli_exe)
        .arg(llvm_ir_path)
        .output()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke lli: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        print!("{}", stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    if !output.status.success() {
        return Err(miette::miette!("compile failed"));
    }

    Ok(())
}

fn artifact_exists(metadata: &RunCacheMetadata) -> bool {
    match metadata.resolved_engine {
        RunEngine::Native => metadata
            .executable_path
            .as_ref()
            .is_some_and(|p| Path::new(p).exists()),
        RunEngine::Lli => Path::new(&metadata.llvm_ir_path).exists(),
        RunEngine::Auto => false,
    }
}

fn build_artifact_exists(metadata: &BuildCacheMetadata) -> bool {
    if metadata.emit_llvm {
        return Path::new(&metadata.output_path).exists();
    }

    Path::new(&metadata.llvm_ir_path).exists() && Path::new(&metadata.output_path).exists()
}

fn derive_cached_native_recovery_plan(
    llvm_ir_exists: bool,
    object_exists: bool,
) -> Option<CachedNativeRecoveryPlan> {
    if object_exists {
        Some(CachedNativeRecoveryPlan::RelinkFromObject)
    } else if llvm_ir_exists {
        Some(CachedNativeRecoveryPlan::RebuildObjectFromCachedIr)
    } else {
        None
    }
}

fn recover_native_output_from_cached_artifacts(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    output_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
) -> Result<CachedNativeRecoveryPlan> {
    let recovery_plan =
        derive_cached_native_recovery_plan(llvm_ir_path.exists(), object_path.exists())
            .ok_or_else(|| miette::miette!("cached object and LLVM IR are both missing"))?;

    if matches!(
        recovery_plan,
        CachedNativeRecoveryPlan::RebuildObjectFromCachedIr
    ) {
        compile_ir_to_object(clang_exe, llvm_ir_path, object_path, opt_level)?;
    }

    let mut object_paths = vec![object_path.to_path_buf()];
    if let Some(runtime_c) = runtime_c {
        object_paths.push(ensure_runtime_object(clang_exe, runtime_c, opt_level)?);
    }
    link_native_binary_from_objects(clang_exe, &object_paths, output_path)?;

    Ok(recovery_plan)
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn bench_root_dir() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        for dir in cwd.ancestors() {
            let candidate = dir.join("bench");
            if candidate.join("suites").exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("bench")
}

fn resolve_bench_suite_path(kind: &str, suite: &str) -> Result<PathBuf> {
    let default_dir = bench_root_dir().join("suites").join(kind);
    if suite == kind || suite == "default" {
        return Ok(default_dir);
    }

    let suite_path = Path::new(suite);
    if suite_path.exists() {
        return Ok(suite_path.to_path_buf());
    }

    let candidate = default_dir.join(suite);
    if candidate.exists() {
        return Ok(candidate);
    }

    let candidate_sg = default_dir.join(format!("{}.sg", suite));
    if candidate_sg.exists() {
        return Ok(candidate_sg);
    }

    Err(miette::miette!(
        "benchmark suite not found: '{}' (kind={})",
        suite,
        kind
    ))
}

fn collect_bench_cases(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut cases = fs::read_dir(path)
        .into_diagnostic()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sg"))
        .collect::<Vec<_>>();

    cases.sort();
    if cases.is_empty() {
        return Err(miette::miette!(
            "no benchmark cases found under {}",
            path.to_string_lossy()
        ));
    }
    Ok(cases)
}

fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(idx).copied()
}

fn run_sgc_command(args: &[String]) -> Result<()> {
    let exe = std::env::current_exe().into_diagnostic()?;
    let output = Command::new(exe).args(args).output().into_diagnostic()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!(
            "benchmark command failed: sgc {}\n{}",
            args.join(" "),
            stderr
        ));
    }
    Ok(())
}

fn measure_sgc_command_ms(args: &[String]) -> Result<f64> {
    let start = Instant::now();
    run_sgc_command(args)?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

fn sanitize_for_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            _ => c,
        })
        .collect()
}

fn write_bench_report(report: &BenchReport) -> Result<PathBuf> {
    let out_dir = bench_root_dir().join("results");
    fs::create_dir_all(&out_dir).into_diagnostic()?;

    let file_name = format!(
        "{}-{}-{}.json",
        now_unix_ms(),
        sanitize_for_filename(&report.kind),
        sanitize_for_filename(&report.suite)
    );
    let output = out_dir.join(file_name);

    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|e| miette::miette!("failed to serialize benchmark report: {}", e))?;
    fs::write(&output, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write benchmark report: {}", e))?;
    Ok(output)
}

fn baseline_case_key(kind: &str, suite: &str, case_name: &str) -> String {
    format!("{}/{}/{}", kind, suite, case_name)
}

fn read_bench_baseline() -> Result<BenchBaseline, String> {
    let path = bench_root_dir().join("baseline.json");
    let bytes =
        fs::read(&path).map_err(|_| format!("baseline missing: {}", path.to_string_lossy()))?;

    // Windows editors often write UTF-8 BOM. Accept both BOM and non-BOM JSON.
    let bytes = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(bytes.as_slice());
    serde_json::from_slice::<BenchBaseline>(bytes)
        .map_err(|err| format!("baseline parse error: {} ({})", path.to_string_lossy(), err))
}

fn diff_report_against_baseline(report: &BenchReport) -> Vec<String> {
    let mut lines = Vec::new();
    let baseline = match read_bench_baseline() {
        Ok(baseline) => baseline,
        Err(err) => {
            lines.push(err);
            return lines;
        }
    };

    for case in &report.cases {
        let key = baseline_case_key(&report.kind, &report.suite, &case.name);
        let Some(base_case) = baseline.cases.get(&key) else {
            continue;
        };

        match report.kind.as_str() {
            "runtime" => {
                if let (Some(curr), Some(base)) = (case.p50_ms, base_case.p50_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} p50: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                    if let Some(target) = baseline.targets.runtime_median_improvement_pct {
                        let improvement = ((base - curr) / base) * 100.0;
                        lines.push(format!(
                            "{} runtime improvement: {:.2}% (target {:.2}%)",
                            case.name, improvement, target
                        ));
                    }
                }
            }
            "compile" => {
                if let (Some(curr), Some(base)) = (case.total_ms, base_case.total_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} total: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                    if let Some(target) = baseline.targets.full_compile_reduction_pct {
                        let reduction = ((base - curr) / base) * 100.0;
                        lines.push(format!(
                            "{} full compile reduction: {:.2}% (target {:.2}%)",
                            case.name, reduction, target
                        ));
                    }
                }
            }
            "incremental" => {
                if let (Some(curr), Some(base)) = (case.before_ms, base_case.before_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} before: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }

                if let (Some(curr), Some(base)) = (case.after_ms, base_case.after_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} after: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }

                if let (Some(before), Some(after), Some(target)) = (
                    case.before_ms,
                    case.after_ms,
                    baseline.targets.incremental_compile_reduction_pct,
                ) {
                    let gain = ((before - after) / before) * 100.0;
                    lines.push(format!(
                        "{} reduction vs same-run before: {:.2}% (target {:.2}%)",
                        case.name, gain, target
                    ));
                }
            }
            "reflection" => {
                if let (Some(curr), Some(base)) = (case.p50_ms, base_case.p50_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} p50: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }
            }
            _ => {}
        }
    }

    if lines.is_empty() {
        lines.push(
            "baseline loaded, but no matching case metrics were found for this report".to_string(),
        );
    }
    lines
}

async fn cmd_bench_run(suite: &str, opt_level: u8, warmup: u32, iterations: u32) -> Result<()> {
    let suite_path = resolve_bench_suite_path("runtime", suite)?;
    let cases = collect_bench_cases(&suite_path)?;

    println!(
        "Benchmark runtime suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());

        let mut sample_ms = Vec::new();
        for i in 0..warmup {
            let mut args = vec![
                "run".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            if i == 0 {
                args.push("--force-rebuild".to_string());
            }
            run_sgc_command(&args)?;
        }

        for _ in 0..iterations {
            let args = vec![
                "run".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            sample_ms.push(measure_sgc_command_ms(&args)?);
        }

        let p50 = percentile(&sample_ms, 0.50);
        let p95 = percentile(&sample_ms, 0.95);
        println!(
            "  - {}: p50={:.2}ms p95={:.2}ms",
            case_name,
            p50.unwrap_or_default(),
            p95.unwrap_or_default()
        );

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup,
            sample_ms,
            p50_ms: p50,
            p95_ms: p95,
            phases: None,
            total_ms: None,
            before_ms: None,
            after_ms: None,
            cache_reused_modules: None,
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "runtime".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Runtime benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

async fn cmd_bench_compile(suite: &str, opt_level: u8, iterations: u32) -> Result<()> {
    let suite_path = resolve_bench_suite_path("compile", suite)?;
    let cases = collect_bench_cases(&suite_path)?;
    let clang = find_clang();
    let runtime_c = find_runtime_c();
    if clang.is_none() {
        println!("  ! clang not found, compile benchmark link phase will be 0ms");
    }

    println!(
        "Benchmark compile suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());
        let source = fs::read_to_string(&case)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to read benchmark case {}: {}", case_name, e))?;

        let mut sample_ms = Vec::new();
        let mut phase_totals: BTreeMap<String, f64> = BTreeMap::new();
        // Use O0 for the external clang link step to reduce backend noise in compile KPI.
        let bench_link_opt_level = 0;
        for _ in 0..iterations {
            let (llvm_ir, mut phases) = compile_source_with_phase_timings(&source, opt_level)?;
            if let Some(clang_exe) = clang.as_deref() {
                let link_ms = link_ir_with_clang_ms(
                    &llvm_ir,
                    &case_name,
                    clang_exe,
                    runtime_c.as_deref(),
                    bench_link_opt_level,
                )?;
                phases.insert("link".to_string(), link_ms);
            }

            let total_ms = phases.values().sum();
            sample_ms.push(total_ms);
            for (phase, value) in phases {
                *phase_totals.entry(phase).or_insert(0.0) += value;
            }
        }

        let avg_ms = if sample_ms.is_empty() {
            0.0
        } else {
            sample_ms.iter().sum::<f64>() / sample_ms.len() as f64
        };
        let mut phase_avg = BTreeMap::new();
        if iterations > 0 {
            for (phase, total) in phase_totals {
                phase_avg.insert(phase, total / iterations as f64);
            }
        }
        for required in ["parse", "typeck", "mir", "codegen", "link"] {
            phase_avg.entry(required.to_string()).or_insert(0.0);
        }

        println!("  - {}: avg={:.2}ms", case_name, avg_ms);

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup: 0,
            sample_ms: sample_ms.clone(),
            p50_ms: percentile(&sample_ms, 0.50),
            p95_ms: percentile(&sample_ms, 0.95),
            phases: Some(phase_avg),
            total_ms: Some(avg_ms),
            before_ms: None,
            after_ms: None,
            cache_reused_modules: None,
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "compile".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Compile benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

async fn cmd_bench_incremental(suite: &str, opt_level: u8, iterations: u32) -> Result<()> {
    let suite_path = resolve_bench_suite_path("incremental", suite)?;
    let cases = collect_bench_cases(&suite_path)?
        .into_iter()
        .filter(|case| {
            case.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_root.sg"))
        })
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return Err(miette::miette!(
            "no incremental benchmark root cases found under {}",
            suite_path.to_string_lossy()
        ));
    }

    println!(
        "Benchmark incremental suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());

        let original = fs::read_to_string(&case)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to read benchmark case {}: {}", case_name, e))?;

        let mut before_samples = Vec::new();
        let mut after_samples = Vec::new();
        let mut reused_module_samples = Vec::new();

        for i in 0..iterations {
            fs::write(&case, &original)
                .into_diagnostic()
                .map_err(|e| miette::miette!("failed to reset benchmark case: {}", e))?;

            let before_args = vec![
                "build".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
                "--force-rebuild".to_string(),
            ];
            before_samples.push(measure_sgc_command_ms(&before_args)?);
            let before_modules = module_fingerprints_for_source(&case, &original);

            let mut mutated = original.clone();
            mutated.push_str(&format!("\n// bench-incremental-mut-{}\n", i));
            fs::write(&case, &mutated)
                .into_diagnostic()
                .map_err(|e| miette::miette!("failed to mutate benchmark case: {}", e))?;
            let after_modules = module_fingerprints_for_source(&case, &mutated);
            let reused_modules =
                module_invalidation_stats(&before_modules, &after_modules).reused_modules;
            reused_module_samples.push(reused_modules);

            let after_args = vec![
                "build".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            after_samples.push(measure_sgc_command_ms(&after_args)?);
        }

        fs::write(&case, original)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to restore benchmark case: {}", e))?;

        let before_avg = if before_samples.is_empty() {
            0.0
        } else {
            before_samples.iter().sum::<f64>() / before_samples.len() as f64
        };
        let after_avg = if after_samples.is_empty() {
            0.0
        } else {
            after_samples.iter().sum::<f64>() / after_samples.len() as f64
        };
        let reused_avg = if reused_module_samples.is_empty() {
            0
        } else {
            (reused_module_samples.iter().sum::<u32>() as f64 / reused_module_samples.len() as f64)
                .round() as u32
        };

        println!(
            "  - {}: before={:.2}ms after={:.2}ms reused_modules={}",
            case_name, before_avg, after_avg, reused_avg
        );

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup: 0,
            sample_ms: Vec::new(),
            p50_ms: None,
            p95_ms: None,
            phases: None,
            total_ms: None,
            before_ms: Some(before_avg),
            after_ms: Some(after_avg),
            cache_reused_modules: Some(reused_avg),
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "incremental".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Incremental benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

fn default_build_output_path_for_case(case: &Path) -> PathBuf {
    let stem = case.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = case.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    let ext = if cfg!(windows) { ".exe" } else { "" };
    build_dir.join(format!("{}{}", stem, ext))
}

fn reflection_shared_library_extension() -> &'static str {
    if cfg!(windows) {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

fn reflection_shared_library_path_for_artifact(artifact_path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}.sgreflect.{}",
        artifact_path.to_string_lossy(),
        reflection_shared_library_extension()
    ))
}

fn reflection_native_export_symbols_from_sidecar(sidecar_path: &Path) -> Result<Vec<String>> {
    let bytes = fs::read(sidecar_path).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to read reflection sidecar for native export symbols {}: {}",
            sidecar_path.to_string_lossy(),
            e
        )
    })?;
    let metadata: ReflectionMetadata =
        serde_json::from_slice(&bytes)
            .into_diagnostic()
            .map_err(|e| {
                miette::miette!(
                    "failed to parse reflection sidecar for native export symbols {}: {}",
                    sidecar_path.to_string_lossy(),
                    e
                )
            })?;
    validate_reflection_metadata(&metadata)?;

    let mut symbols = HashSet::<String>::new();
    for module in metadata.modules {
        for symbol in module.symbols {
            let exported = symbol.native_symbol.unwrap_or_else(|| {
                symbol
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string()
            });
            if !exported.trim().is_empty() {
                symbols.insert(exported);
            }
        }
    }
    let mut exported = symbols.into_iter().collect::<Vec<_>>();
    exported.sort();
    Ok(exported)
}

fn run_shared_link_command(
    clang_exe: &str,
    object_paths: &[PathBuf],
    shared_library_path: &Path,
    use_lld: bool,
    extra_linker_flags: &[String],
) -> Result<std::process::ExitStatus> {
    let mut clang_cmd = Command::new(clang_exe);
    clang_cmd.arg("-Wno-override-module");
    if use_lld {
        clang_cmd.arg("-fuse-ld=lld");
    }
    clang_cmd.arg("-shared");
    for object in object_paths {
        clang_cmd.arg(object);
    }
    for flag in extra_linker_flags {
        clang_cmd.arg(flag);
    }
    clang_cmd.arg("-o").arg(shared_library_path);
    clang_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang shared linker: {}", e))
}

fn link_shared_library_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    shared_library_path: &Path,
    export_symbols: &[String],
) -> Result<()> {
    let mode = linker_mode_from_env();
    let lld_state = LLD_AVAILABILITY.load(Ordering::Relaxed);
    let try_lld_first = match mode {
        LinkerMode::Lld => true,
        LinkerMode::System => false,
        LinkerMode::Auto => lld_state != LINKER_UNAVAILABLE,
    };

    let export_linker_flags = if cfg!(windows) {
        export_symbols
            .iter()
            .map(|symbol| format!("-Wl,/EXPORT:{}", symbol))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let export_all_linker_flags = if cfg!(windows) {
        vec!["-Wl,--export-all-symbols".to_string()]
    } else {
        Vec::new()
    };

    if try_lld_first {
        let lld_status = run_shared_link_command(
            clang_exe,
            object_paths,
            shared_library_path,
            true,
            &export_linker_flags,
        )?;
        if lld_status.success() {
            if matches!(mode, LinkerMode::Auto) {
                LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
            }
            return Ok(());
        }
        if cfg!(windows) {
            let lld_export_all_status = run_shared_link_command(
                clang_exe,
                object_paths,
                shared_library_path,
                true,
                &export_all_linker_flags,
            )?;
            if lld_export_all_status.success() {
                if matches!(mode, LinkerMode::Auto) {
                    LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
                }
                return Ok(());
            }
        }
        if matches!(mode, LinkerMode::Lld) {
            return Err(miette::miette!("compile failed (lld linker mode)"));
        }
        LLD_AVAILABILITY.store(LINKER_UNAVAILABLE, Ordering::Relaxed);
        println!("link fallback: lld unavailable, retrying with system linker");
    }

    let status = run_shared_link_command(
        clang_exe,
        object_paths,
        shared_library_path,
        false,
        &export_linker_flags,
    )?;
    if status.success() {
        return Ok(());
    }
    if cfg!(windows) {
        let status_export_all = run_shared_link_command(
            clang_exe,
            object_paths,
            shared_library_path,
            false,
            &export_all_linker_flags,
        )?;
        if status_export_all.success() {
            return Ok(());
        }
    }
    Err(miette::miette!("compile failed"))
}

fn compile_reflection_shared_library(
    clang_exe: &str,
    llvm_ir_path: &Path,
    shared_library_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    export_symbols: &[String],
) -> Result<()> {
    let object_path = shared_library_path.with_extension(object_file_extension());
    compile_ir_to_object(clang_exe, llvm_ir_path, &object_path, opt_level)?;
    let mut object_paths = vec![object_path];
    if let Some(runtime_c) = runtime_c {
        object_paths.push(ensure_runtime_object(clang_exe, runtime_c, opt_level)?);
    }
    link_shared_library_from_objects(
        clang_exe,
        &object_paths,
        shared_library_path,
        export_symbols,
    )?;
    Ok(())
}

fn maybe_prepare_reflection_native_library(
    clang_exe: Option<&str>,
    runtime_c: Option<&str>,
    llvm_ir_path: &Path,
    artifact_path: &Path,
    sidecar_path: &Path,
    opt_level: u8,
) -> Result<Option<PathBuf>> {
    let Some(clang_exe) = clang_exe else {
        return Ok(None);
    };
    if !llvm_ir_path.exists() || !sidecar_path.exists() {
        return Ok(None);
    }

    let export_symbols = reflection_native_export_symbols_from_sidecar(sidecar_path)?;
    if export_symbols.is_empty() {
        return Ok(None);
    }

    let shared_library_path = reflection_shared_library_path_for_artifact(artifact_path);
    compile_reflection_shared_library(
        clang_exe,
        llvm_ir_path,
        &shared_library_path,
        runtime_c,
        opt_level,
        &export_symbols,
    )?;
    Ok(Some(shared_library_path))
}

fn signature_is_zero_arity_i64(signature: &str) -> bool {
    let mut params = None::<&str>;
    let mut ret = None::<&str>;

    for part in signature.split('|') {
        if let Some(value) = part.strip_prefix("params=[") {
            params = value.strip_suffix(']');
        } else if let Some(value) = part.strip_prefix("ret=") {
            ret = Some(value.trim());
        }
    }

    matches!(ret, Some("i64")) && matches!(params, Some(raw) if raw.trim().is_empty())
}

fn select_reflection_i64_zero_arity_symbol(
    symbols: &[RuntimeReflectionSymbolMetadata],
) -> Option<String> {
    for preferred in ["reflect_probe", "main"] {
        for symbol in symbols {
            let short = symbol.symbol.rsplit("::").next().unwrap_or_default();
            if short == preferred && signature_is_zero_arity_i64(&symbol.signature) {
                return Some(short.to_string());
            }
        }
    }

    for symbol in symbols {
        let short = symbol.symbol.rsplit("::").next().unwrap_or_default();
        if signature_is_zero_arity_i64(&symbol.signature) {
            return Some(short.to_string());
        }
    }

    None
}

fn measure_reflection_used_ms(
    sidecar_path: &Path,
    module_id: &str,
    native_library_path: Option<&Path>,
) -> Result<(f64, bool)> {
    let runtime = ReflectionRuntime::new(sidecar_path);
    let start = Instant::now();
    let symbols = runtime
        .list_symbols(module_id)
        .map_err(|e| miette::miette!("reflection API list failed: {}", e))?;
    let symbol = select_reflection_i64_zero_arity_symbol(&symbols).ok_or_else(|| {
        miette::miette!(
            "no zero-arity i64 symbol found for reflection invoke in module {}",
            module_id
        )
    })?;

    let mut native_bound = false;
    if let Some(native_library_path) = native_library_path {
        native_bound = runtime
            .register_i64_native_bindings_from_library(native_library_path)
            .is_ok();
    }
    if !native_bound {
        runtime
            .register_fn(module_id, &symbol, |_args| {
                Ok(sengoo_runtime::ReflectValue::I64(0))
            })
            .map_err(|e| miette::miette!("reflection API register failed: {}", e))?;
    }

    runtime
        .call_i64(module_id, &symbol, &[])
        .map_err(|e| miette::miette!("reflection API typed invoke failed: {}", e))?;
    Ok((start.elapsed().as_secs_f64() * 1000.0, native_bound))
}

async fn cmd_bench_reflection(
    suite: &str,
    opt_level: u8,
    warmup: u32,
    iterations: u32,
) -> Result<()> {
    let suite_path = resolve_bench_suite_path("runtime", suite)?;
    let cases = collect_bench_cases(&suite_path)?;
    let case = cases
        .first()
        .cloned()
        .ok_or_else(|| miette::miette!("no reflection benchmark case found"))?;
    let case_name = case
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| case.to_string_lossy().to_string());

    println!(
        "Benchmark reflection suite: {} (case={})",
        suite_path.to_string_lossy(),
        case_name
    );

    let base_args = vec![
        "build".to_string(),
        case.to_string_lossy().to_string(),
        "-O".to_string(),
        opt_level.to_string(),
        "--force-rebuild".to_string(),
    ];
    let mut reflect_args = base_args.clone();
    reflect_args.push("--reflect".to_string());

    let module_id = canonical_or_lossy(&case);
    let artifact_path = default_build_output_path_for_case(&case);
    let sidecar_path = reflection_sidecar_path_for_artifact(&artifact_path);
    let llvm_ir_path = artifact_path.with_extension("ll");
    let clang_exe = find_clang();
    let runtime_c = find_runtime_c();
    let mut native_prepare_warning: Option<String> = None;
    let mut native_bound_measurements = 0u32;

    for _ in 0..warmup {
        let _ = measure_sgc_command_ms(&base_args)?;
        let _ = measure_sgc_command_ms(&reflect_args)?;
        let _ = measure_sgc_command_ms(&reflect_args)?;
        if !sidecar_path.exists() {
            return Err(miette::miette!(
                "reflection sidecar missing during warmup: {}",
                sidecar_path.to_string_lossy()
            ));
        }
        let native_library_path = match maybe_prepare_reflection_native_library(
            clang_exe.as_deref(),
            runtime_c.as_deref(),
            &llvm_ir_path,
            &artifact_path,
            &sidecar_path,
            opt_level,
        ) {
            Ok(path) => path,
            Err(err) => {
                if native_prepare_warning.is_none() {
                    native_prepare_warning = Some(err.to_string());
                }
                None
            }
        };
        let _ =
            measure_reflection_used_ms(&sidecar_path, &module_id, native_library_path.as_deref())?;
    }

    let mut disabled_samples = Vec::new();
    let mut enabled_unused_samples = Vec::new();
    let mut enabled_used_samples = Vec::new();
    for _ in 0..iterations {
        disabled_samples.push(measure_sgc_command_ms(&base_args)?);

        enabled_unused_samples.push(measure_sgc_command_ms(&reflect_args)?);

        let build_ms = measure_sgc_command_ms(&reflect_args)?;
        if !sidecar_path.exists() {
            return Err(miette::miette!(
                "reflection sidecar missing after reflected build: {}",
                sidecar_path.to_string_lossy()
            ));
        }
        let native_library_path = match maybe_prepare_reflection_native_library(
            clang_exe.as_deref(),
            runtime_c.as_deref(),
            &llvm_ir_path,
            &artifact_path,
            &sidecar_path,
            opt_level,
        ) {
            Ok(path) => path,
            Err(err) => {
                if native_prepare_warning.is_none() {
                    native_prepare_warning = Some(err.to_string());
                }
                None
            }
        };
        let (used_ms, native_bound) =
            measure_reflection_used_ms(&sidecar_path, &module_id, native_library_path.as_deref())?;
        if native_bound {
            native_bound_measurements += 1;
        }
        enabled_used_samples.push(build_ms + used_ms);
    }

    let avg = |samples: &[f64]| -> f64 {
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f64>() / samples.len() as f64
        }
    };
    let disabled_p50 = percentile(&disabled_samples, 0.50).unwrap_or(0.0);
    let enabled_unused_p50 = percentile(&enabled_unused_samples, 0.50).unwrap_or(0.0);
    let enabled_used_p50 = percentile(&enabled_used_samples, 0.50).unwrap_or(0.0);
    if disabled_p50 > 0.0 {
        println!(
            "  - disabled p50={:.2}ms, enabled-unused overhead={:+.2}%, enabled-used overhead={:+.2}%",
            disabled_p50,
            ((enabled_unused_p50 - disabled_p50) / disabled_p50) * 100.0,
            ((enabled_used_p50 - disabled_p50) / disabled_p50) * 100.0,
        );
    }
    if let Some(warning) = native_prepare_warning {
        println!(
            "  - note: native reflection binding unavailable in bench, fallback handler used ({})",
            warning
        );
    } else if iterations > 0 {
        println!(
            "  - native reflection binding used in {}/{} measured iteration(s)",
            native_bound_measurements, iterations
        );
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "reflection".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: vec![
            BenchCaseResult {
                name: "disabled".to_string(),
                iterations,
                warmup,
                sample_ms: disabled_samples.clone(),
                p50_ms: percentile(&disabled_samples, 0.50),
                p95_ms: percentile(&disabled_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&disabled_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
            },
            BenchCaseResult {
                name: "enabled-unused".to_string(),
                iterations,
                warmup,
                sample_ms: enabled_unused_samples.clone(),
                p50_ms: percentile(&enabled_unused_samples, 0.50),
                p95_ms: percentile(&enabled_unused_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&enabled_unused_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
            },
            BenchCaseResult {
                name: "enabled-used".to_string(),
                iterations,
                warmup,
                sample_ms: enabled_used_samples.clone(),
                p50_ms: percentile(&enabled_used_samples, 0.50),
                p95_ms: percentile(&enabled_used_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&enabled_used_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
            },
        ],
    };

    let out = write_bench_report(&report)?;
    println!("Reflection benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

async fn cmd_build(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    reflection: ReflectionCliOptions,
) -> Result<()> {
    println!("Building: {}", input);

    let input_path = Path::new(input);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = input_path.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    fs::create_dir_all(&build_dir).into_diagnostic()?;

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;

    let source_hash = implementation_fingerprint(&source);
    let root_implementation_hash = source_hash;
    let root_interface_hash = interface_fingerprint(&source);
    let cache_path = build_dir.join(format!("{}.build-cache.json", stem));
    let frontend_session_path = frontend_session_store_path(&build_dir, &stem);
    let previous_build_metadata_seed = load_build_cache(&cache_path);
    let previous_frontend_session = load_frontend_session_store(&frontend_session_path);
    let probe_mode = if force_rebuild {
        FrontendProbeMode::FastNoVerify
    } else {
        FrontendProbeMode::VerifyChangedAndDependents
    };
    let graph_snapshot = collect_module_graph_snapshot(
        input_path,
        &source,
        previous_build_metadata_seed
            .as_ref()
            .and_then(|metadata| metadata.build_graph_v2.as_ref()),
        previous_frontend_session.as_ref(),
        probe_mode,
    );
    let reflection = resolve_reflection_options_for_snapshot(reflection, &graph_snapshot);
    println!("{}", reflection_mode_note(&reflection, &graph_snapshot));
    let module_fingerprints = graph_snapshot.module_fingerprints.clone();
    if !graph_snapshot.diagnostics.is_empty() {
        println!("frontend probe diagnostics (stable order):");
        for line in &graph_snapshot.diagnostics {
            println!("  - {}", line);
        }
    }
    println!(
        "frontend session: reused_modules={} rebuilt_modules={}",
        graph_snapshot.reused_modules.len(),
        graph_snapshot.rebuilt_modules.len()
    );
    if let Err(err) = save_frontend_session_store(
        &frontend_session_path,
        &graph_snapshot.frontend_session_store,
    ) {
        println!("frontend session fallback: {}", err);
    }
    let runtime_c = find_runtime_c();

    let output_file = if let Some(out) = output {
        out.to_string()
    } else if emit_llvm {
        build_dir
            .join(format!("{}.ll", stem))
            .to_string_lossy()
            .to_string()
    } else {
        let ext = if cfg!(windows) { ".exe" } else { "" };
        build_dir
            .join(format!("{}{}", stem, ext))
            .to_string_lossy()
            .to_string()
    };

    let llvm_ir_path = if emit_llvm {
        PathBuf::from(&output_file)
    } else {
        build_dir.join(format!("{}.ll", stem))
    };
    let object_path = if emit_llvm {
        None
    } else {
        Some(build_dir.join(format!("{}.{}", stem, object_file_extension())))
    };
    let graph_v2 = build_graph_v2_with_function_fingerprints_for_source(
        input_path,
        &module_fingerprints,
        &graph_snapshot.module_function_fingerprints,
        &graph_snapshot.dependency_edges,
        object_path.as_deref(),
        root_interface_hash,
        root_implementation_hash,
    );
    let key = build_cache_key(
        source_hash,
        module_fingerprints.clone(),
        opt_level,
        emit_llvm,
        runtime_c.clone(),
        output_file.clone(),
    );
    let mut edit_impact: Option<EditImpact> = None;

    let previous_build_metadata = if force_rebuild {
        println!("build cache bypassed: --force-rebuild");
        None
    } else if let Some(metadata) = previous_build_metadata_seed.clone() {
        if build_metadata_matches(&metadata, &key) {
            if build_artifact_exists(&metadata) {
                println!(
                    "build cache hit (opt=O{}, emit_llvm={})",
                    metadata.opt_level, metadata.emit_llvm
                );
                maybe_emit_reflection_sidecar(
                    Path::new(&metadata.output_path),
                    &graph_v2,
                    &reflection,
                    Some(Path::new(&metadata.llvm_ir_path)),
                )?;
                println!("Build output: {}", metadata.output_path);
                return Ok(());
            }
            println!("build cache miss: cached artifacts are missing");
        } else {
            println!("build cache miss: metadata changed");
            for reason in build_cache_mismatch_reasons(&metadata, &key) {
                println!("  - {}", reason);
            }
            let impact = classify_edit_impact(
                metadata.root_interface_hash,
                metadata.root_implementation_hash,
                root_interface_hash,
                root_implementation_hash,
                &metadata.module_fingerprints,
                &module_fingerprints,
                metadata.build_graph_v2.as_ref(),
                &graph_v2,
            );
            for line in format_edit_impact_lines(&impact) {
                println!("  - {}", line);
            }
            edit_impact = Some(impact);
        }
        Some(metadata)
    } else {
        println!(
            "build cache miss: no cache metadata at {}",
            cache_path.to_string_lossy()
        );
        None
    };

    let workset_plan = derive_build_workset_plan(
        previous_build_metadata.as_ref(),
        edit_impact.as_ref(),
        &graph_v2.root_module,
        emit_llvm,
        opt_level,
        &output_file,
        runtime_c.as_deref(),
    );
    let workset_manifest =
        derive_codegen_workset_manifest(&graph_v2, edit_impact.as_ref(), workset_plan);
    let build_workset_manifest_path = codegen_workset_manifest_path(&build_dir, &stem, "build");
    save_codegen_workset_manifest(&build_workset_manifest_path, &workset_manifest)?;
    println!(
        "codegen workset: rebuild_modules={} reuse_modules={} rebuild_symbols={} reuse_symbols={}",
        workset_manifest.rebuild_modules.len(),
        workset_manifest.reuse_modules.len(),
        workset_manifest.rebuild_symbols.len(),
        workset_manifest.reuse_symbols.len(),
    );
    println!(
        "codegen workset manifest: {}",
        build_workset_manifest_path.to_string_lossy()
    );
    match workset_plan {
        BuildWorksetPlan::ReusePreviousArtifacts => {
            if let Some(previous) = previous_build_metadata.as_ref() {
                if build_artifact_exists(previous) {
                    let class_label = edit_impact
                        .as_ref()
                        .map(|impact| edit_class_label(impact.class))
                        .unwrap_or("unknown");
                    println!(
                        "build workset plan: reuse previous artifacts ({})",
                        class_label
                    );
                    maybe_emit_reflection_sidecar(
                        Path::new(&previous.output_path),
                        &graph_v2,
                        &reflection,
                        Some(Path::new(&previous.llvm_ir_path)),
                    )?;
                    println!("Build output: {}", previous.output_path);
                    return Ok(());
                }
                if !emit_llvm {
                    let expected_object_path = object_path.as_deref().ok_or_else(|| {
                        miette::miette!("internal error: missing object path for native build")
                    })?;
                    let previous_object_path = previous.object_path.as_deref();
                    if let Some(previous_object_path) = previous_object_path {
                        if canonical_or_lossy(Path::new(previous_object_path))
                            == canonical_or_lossy(expected_object_path)
                        {
                            if let Some(clang_exe) = find_clang() {
                                let output_path = Path::new(&output_file);
                                match recover_native_output_from_cached_artifacts(
                                    &clang_exe,
                                    Path::new(&previous.llvm_ir_path),
                                    expected_object_path,
                                    output_path,
                                    runtime_c.as_deref(),
                                    opt_level,
                                ) {
                                    Ok(recovery) => {
                                        let label = match recovery {
                                            CachedNativeRecoveryPlan::RelinkFromObject => {
                                                "relinked cached object"
                                            }
                                            CachedNativeRecoveryPlan::RebuildObjectFromCachedIr => {
                                                "rebuilt object from cached LLVM IR and relinked"
                                            }
                                        };
                                        println!("build workset plan: {}", label);
                                        println!("Build output: {}", output_file);
                                        return Ok(());
                                    }
                                    Err(err) => {
                                        println!("build workset fallback: {}", err);
                                    }
                                }
                            } else {
                                println!(
                                    "build workset fallback: clang unavailable for cached relink"
                                );
                            }
                        } else {
                            println!("build workset fallback: cached object path changed");
                        }
                    } else {
                        println!("build workset fallback: cached object path missing");
                    }
                } else {
                    println!("build workset fallback: previous artifacts are missing");
                }
            }
        }
        BuildWorksetPlan::RebuildImpactedRoot => {
            println!("build workset plan: rebuild impacted root module");
        }
        BuildWorksetPlan::FullRebuild => {
            if edit_impact.is_some() {
                println!("build workset plan: full rebuild");
            }
        }
    }

    let llvm_ir = match compile_source(&source, opt_level) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
            return Err(miette::miette!("compile failed"));
        }
    };

    fs::write(&llvm_ir_path, &llvm_ir)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write LLVM IR: {}", e))?;
    let llvm_ir_hash = source_fingerprint(&llvm_ir);

    if emit_llvm {
        maybe_emit_reflection_sidecar(
            Path::new(&output_file),
            &graph_v2,
            &reflection,
            Some(&llvm_ir_path),
        )?;
        let metadata = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash,
            root_interface_hash,
            root_implementation_hash,
            module_fingerprints,
            opt_level,
            emit_llvm: true,
            runtime_c,
            llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
            output_path: output_file.clone(),
            llvm_ir_hash,
            object_path: None,
            build_graph_v2: Some(graph_v2),
        };
        save_build_cache(&cache_path, &metadata)?;
        println!("LLVM IR written to {}", output_file);
        return Ok(());
    }

    let clang_exe = find_clang().ok_or_else(|| {
        miette::miette!(
            "clang is required to build native binaries. Install LLVM/Clang or use --emit-llvm"
        )
    })?;
    let object_path = object_path
        .clone()
        .ok_or_else(|| miette::miette!("internal error: missing object path"))?;
    let output_path = Path::new(&output_file);

    let incremental_mode = incremental_link_mode_from_env();
    if matches!(incremental_mode, IncrementalLinkMode::Off) {
        println!("incremental link disabled: SENGOO_INCREMENTAL_LINK=off");
    }
    let incremental_check = if matches!(incremental_mode, IncrementalLinkMode::Off) {
        None
    } else {
        previous_build_metadata.as_ref().map(|previous| {
            can_use_incremental_link_with_metadata(
                previous,
                llvm_ir_hash,
                &object_path,
                &output_file,
                runtime_c.as_deref(),
                opt_level,
                &graph_v2,
            )
        })
    };

    match incremental_check {
        Some(Ok(())) => {
            println!(
                "incremental link: reusing object {}",
                object_path.to_string_lossy()
            );
        }
        Some(Err(reason)) => {
            println!("incremental link fallback: {}", reason);
            if let Some(previous) = previous_build_metadata.as_ref() {
                let impact = classify_edit_impact(
                    previous.root_interface_hash,
                    previous.root_implementation_hash,
                    root_interface_hash,
                    root_implementation_hash,
                    &previous.module_fingerprints,
                    &module_fingerprints,
                    previous.build_graph_v2.as_ref(),
                    &graph_v2,
                );
                for line in format_edit_impact_lines(&impact) {
                    println!("  - {}", line);
                }
            }
            compile_ir_to_object(&clang_exe, &llvm_ir_path, &object_path, opt_level)?;
        }
        None => {
            compile_ir_to_object(&clang_exe, &llvm_ir_path, &object_path, opt_level)?;
        }
    }

    let mut object_paths = vec![object_path.clone()];
    if let Some(runtime_c) = runtime_c.as_deref() {
        object_paths.push(ensure_runtime_object(&clang_exe, runtime_c, opt_level)?);
    }
    link_native_binary_from_objects(&clang_exe, &object_paths, output_path)?;
    maybe_emit_reflection_sidecar(
        Path::new(&output_file),
        &graph_v2,
        &reflection,
        Some(&llvm_ir_path),
    )?;

    let metadata = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash,
        root_interface_hash,
        root_implementation_hash,
        module_fingerprints,
        opt_level,
        emit_llvm: false,
        runtime_c,
        llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
        output_path: output_file.clone(),
        llvm_ir_hash,
        object_path: Some(object_path.to_string_lossy().to_string()),
        build_graph_v2: Some(graph_v2),
    };
    save_build_cache(&cache_path, &metadata)?;

    println!("Build output: {}", output_file);
    Ok(())
}

async fn cmd_run(
    input: &str,
    opt_level: u8,
    requested_engine: RunEngine,
    force_rebuild: bool,
    _args: &[String],
    reflection: ReflectionCliOptions,
) -> Result<()> {
    println!("Running: {}", input);

    let input_path = Path::new(input);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = input_path.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    fs::create_dir_all(&build_dir).into_diagnostic()?;

    let llvm_ir_path = build_dir.join(format!("{}.ll", stem));
    let executable_path = if cfg!(windows) {
        build_dir.join(format!("{}.exe", stem))
    } else {
        build_dir.join(stem.to_string())
    };
    let cache_path = build_dir.join(format!("{}.run-cache.json", stem));
    let frontend_session_path = frontend_session_store_path(&build_dir, &stem);
    let previous_run_metadata_seed = load_run_cache(&cache_path);
    let previous_frontend_session = load_frontend_session_store(&frontend_session_path);

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;
    let source_hash = implementation_fingerprint(&source);
    let root_implementation_hash = source_hash;
    let root_interface_hash = interface_fingerprint(&source);
    let probe_mode = if force_rebuild {
        FrontendProbeMode::FastNoVerify
    } else {
        FrontendProbeMode::VerifyChangedAndDependents
    };
    let graph_snapshot = collect_module_graph_snapshot(
        input_path,
        &source,
        previous_run_metadata_seed
            .as_ref()
            .and_then(|metadata| metadata.build_graph_v2.as_ref()),
        previous_frontend_session.as_ref(),
        probe_mode,
    );
    let reflection = resolve_reflection_options_for_snapshot(reflection, &graph_snapshot);
    println!("{}", reflection_mode_note(&reflection, &graph_snapshot));
    let module_fingerprints = graph_snapshot.module_fingerprints.clone();
    if !graph_snapshot.diagnostics.is_empty() {
        println!("frontend probe diagnostics (stable order):");
        for line in &graph_snapshot.diagnostics {
            println!("  - {}", line);
        }
    }
    println!(
        "frontend session: reused_modules={} rebuilt_modules={}",
        graph_snapshot.reused_modules.len(),
        graph_snapshot.rebuilt_modules.len()
    );
    if let Err(err) = save_frontend_session_store(
        &frontend_session_path,
        &graph_snapshot.frontend_session_store,
    ) {
        println!("frontend session fallback: {}", err);
    }
    let object_path = build_dir.join(format!("{}.{}", stem, object_file_extension()));
    let graph_v2 = build_graph_v2_with_function_fingerprints_for_source(
        input_path,
        &module_fingerprints,
        &graph_snapshot.module_function_fingerprints,
        &graph_snapshot.dependency_edges,
        Some(&object_path),
        root_interface_hash,
        root_implementation_hash,
    );

    let runtime_c = find_runtime_c();
    let clang_exe = find_clang();
    let lli_exe = find_lli();

    let resolved_engine = resolve_engine(requested_engine, clang_exe.is_some(), lli_exe.is_some())?;

    let key = cache_key(
        source_hash,
        module_fingerprints.clone(),
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c.clone(),
    );
    let mut edit_impact: Option<EditImpact> = None;

    let previous_run_metadata = if force_rebuild {
        println!("cache bypassed: --force-rebuild");
        None
    } else if let Some(metadata) = previous_run_metadata_seed.clone() {
        if metadata_matches(&metadata, &key) {
            if artifact_exists(&metadata) {
                println!(
                    "cache hit (engine={:?}, modules={})",
                    metadata.resolved_engine,
                    metadata.module_fingerprints.len()
                );
                return match metadata.resolved_engine {
                    RunEngine::Native => {
                        let exe = metadata.executable_path.as_deref().ok_or_else(|| {
                            miette::miette!("cache corrupted: missing native executable path")
                        })?;
                        maybe_emit_reflection_sidecar(
                            Path::new(exe),
                            &graph_v2,
                            &reflection,
                            Some(Path::new(&metadata.llvm_ir_path)),
                        )?;
                        run_native_binary(Path::new(exe))
                    }
                    RunEngine::Lli => {
                        let lli = lli_exe.as_deref().ok_or_else(|| {
                            miette::miette!("cache hit but lli is unavailable; try --force-rebuild")
                        })?;
                        maybe_emit_reflection_sidecar(
                            Path::new(&metadata.llvm_ir_path),
                            &graph_v2,
                            &reflection,
                            Some(Path::new(&metadata.llvm_ir_path)),
                        )?;
                        run_with_lli(lli, Path::new(&metadata.llvm_ir_path))
                    }
                    RunEngine::Auto => Err(miette::miette!("compile failed")),
                };
            } else {
                println!("cache miss: cached artifacts are missing");
            }
        } else {
            println!("cache miss: metadata changed");
            for reason in cache_mismatch_reasons(&metadata, &key) {
                println!("  - {}", reason);
            }
            let impact = classify_edit_impact(
                metadata.root_interface_hash,
                metadata.root_implementation_hash,
                root_interface_hash,
                root_implementation_hash,
                &metadata.module_fingerprints,
                &module_fingerprints,
                metadata.build_graph_v2.as_ref(),
                &graph_v2,
            );
            for line in format_edit_impact_lines(&impact) {
                println!("  - {}", line);
            }
            edit_impact = Some(impact);
        }
        Some(metadata)
    } else {
        println!(
            "cache miss: no cache metadata at {}",
            cache_path.to_string_lossy()
        );
        None
    };

    let workset_plan = derive_run_workset_plan(
        previous_run_metadata.as_ref(),
        edit_impact.as_ref(),
        &graph_v2.root_module,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c.as_deref(),
    );
    let workset_manifest =
        derive_codegen_workset_manifest(&graph_v2, edit_impact.as_ref(), workset_plan);
    let run_workset_manifest_path = codegen_workset_manifest_path(&build_dir, &stem, "run");
    save_codegen_workset_manifest(&run_workset_manifest_path, &workset_manifest)?;
    println!(
        "codegen workset: rebuild_modules={} reuse_modules={} rebuild_symbols={} reuse_symbols={}",
        workset_manifest.rebuild_modules.len(),
        workset_manifest.reuse_modules.len(),
        workset_manifest.rebuild_symbols.len(),
        workset_manifest.reuse_symbols.len(),
    );
    println!(
        "codegen workset manifest: {}",
        run_workset_manifest_path.to_string_lossy()
    );
    if let BuildWorksetPlan::ReusePreviousArtifacts = workset_plan {
        if let Some(previous) = previous_run_metadata.as_ref() {
            if artifact_exists(previous) {
                let class_label = edit_impact
                    .as_ref()
                    .map(|impact| edit_class_label(impact.class))
                    .unwrap_or("unknown");
                println!(
                    "run workset plan: reuse previous artifacts ({})",
                    class_label
                );
                return match previous.resolved_engine {
                    RunEngine::Native => {
                        let exe = previous.executable_path.as_deref().ok_or_else(|| {
                            miette::miette!("cache corrupted: missing native executable path")
                        })?;
                        run_native_binary(Path::new(exe))
                    }
                    RunEngine::Lli => {
                        let lli = lli_exe.as_deref().ok_or_else(|| {
                            miette::miette!("cache hit but lli is unavailable; try --force-rebuild")
                        })?;
                        run_with_lli(lli, Path::new(&previous.llvm_ir_path))
                    }
                    RunEngine::Auto => Err(miette::miette!("compile failed")),
                };
            }
            if matches!(resolved_engine, RunEngine::Native)
                && matches!(previous.resolved_engine, RunEngine::Native)
            {
                if let Some(previous_object_path) = previous.object_path.as_deref() {
                    if canonical_or_lossy(Path::new(previous_object_path))
                        == canonical_or_lossy(&object_path)
                    {
                        if let Some(clang) = clang_exe.as_deref() {
                            match recover_native_output_from_cached_artifacts(
                                clang,
                                Path::new(&previous.llvm_ir_path),
                                &object_path,
                                &executable_path,
                                runtime_c.as_deref(),
                                opt_level,
                            ) {
                                Ok(recovery) => {
                                    let label = match recovery {
                                        CachedNativeRecoveryPlan::RelinkFromObject => {
                                            "relinked cached object"
                                        }
                                        CachedNativeRecoveryPlan::RebuildObjectFromCachedIr => {
                                            "rebuilt object from cached LLVM IR and relinked"
                                        }
                                    };
                                    println!("run workset plan: {}", label);
                                    return run_native_binary(&executable_path);
                                }
                                Err(err) => {
                                    println!("run workset fallback: {}", err);
                                }
                            }
                        } else {
                            println!("run workset fallback: clang unavailable for cached relink");
                        }
                    } else {
                        println!("run workset fallback: cached object path changed");
                    }
                } else {
                    println!("run workset fallback: cached object path missing");
                }
            } else {
                println!("run workset fallback: previous artifacts are missing");
            }
        }
    } else if matches!(workset_plan, BuildWorksetPlan::RebuildImpactedRoot) {
        println!("run workset plan: rebuild impacted root module");
    } else if edit_impact.is_some() {
        println!("run workset plan: full rebuild");
    }

    let llvm_ir = match compile_source(&source, opt_level) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
            return Err(miette::miette!("compile failed"));
        }
    };

    fs::write(&llvm_ir_path, &llvm_ir)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write LLVM IR: {}", e))?;
    let llvm_ir_hash = source_fingerprint(&llvm_ir);

    match resolved_engine {
        RunEngine::Native => {
            let clang = clang_exe
                .as_deref()
                .ok_or_else(|| miette::miette!("clang is required for --engine native"))?;

            let incremental_mode = incremental_link_mode_from_env();
            if matches!(incremental_mode, IncrementalLinkMode::Off) {
                println!("incremental link disabled: SENGOO_INCREMENTAL_LINK=off");
            }
            let incremental_check = if matches!(incremental_mode, IncrementalLinkMode::Off) {
                None
            } else {
                previous_run_metadata.as_ref().map(|previous| {
                    can_use_incremental_link_with_run_metadata(
                        previous,
                        llvm_ir_hash,
                        &object_path,
                        runtime_c.as_deref(),
                        opt_level,
                        requested_engine,
                        resolved_engine,
                        &graph_v2,
                    )
                })
            };

            match incremental_check {
                Some(Ok(())) => {
                    println!(
                        "incremental link: reusing object {}",
                        object_path.to_string_lossy()
                    );
                }
                Some(Err(reason)) => {
                    println!("incremental link fallback: {}", reason);
                    if let Some(previous) = previous_run_metadata.as_ref() {
                        let impact = classify_edit_impact(
                            previous.root_interface_hash,
                            previous.root_implementation_hash,
                            root_interface_hash,
                            root_implementation_hash,
                            &previous.module_fingerprints,
                            &module_fingerprints,
                            previous.build_graph_v2.as_ref(),
                            &graph_v2,
                        );
                        for line in format_edit_impact_lines(&impact) {
                            println!("  - {}", line);
                        }
                    }
                    compile_ir_to_object(clang, &llvm_ir_path, &object_path, opt_level)?;
                }
                None => {
                    compile_ir_to_object(clang, &llvm_ir_path, &object_path, opt_level)?;
                }
            }

            let mut object_paths = vec![object_path.clone()];
            if let Some(runtime_c) = runtime_c.as_deref() {
                object_paths.push(ensure_runtime_object(clang, runtime_c, opt_level)?);
            }
            link_native_binary_from_objects(clang, &object_paths, &executable_path)?;
            run_native_binary(&executable_path)?;
        }
        RunEngine::Lli => {
            let lli = lli_exe
                .as_deref()
                .ok_or_else(|| miette::miette!("lli is required for --engine lli"))?;
            run_with_lli(lli, &llvm_ir_path)?;
        }
        RunEngine::Auto => {
            return Err(miette::miette!(
                "internal error: resolved_engine should not be auto"
            ))
        }
    }
    let reflection_artifact_path = match resolved_engine {
        RunEngine::Native => executable_path.as_path(),
        RunEngine::Lli => llvm_ir_path.as_path(),
        RunEngine::Auto => {
            return Err(miette::miette!(
                "internal error: resolved_engine should not be auto"
            ))
        }
    };
    maybe_emit_reflection_sidecar(
        reflection_artifact_path,
        &graph_v2,
        &reflection,
        Some(&llvm_ir_path),
    )?;
    let metadata = RunCacheMetadata {
        source_hash,
        root_interface_hash,
        root_implementation_hash,
        module_fingerprints,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c,
        llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
        executable_path: if matches!(resolved_engine, RunEngine::Native) {
            Some(executable_path.to_string_lossy().to_string())
        } else {
            None
        },
        llvm_ir_hash,
        object_path: if matches!(resolved_engine, RunEngine::Native) {
            Some(object_path.to_string_lossy().to_string())
        } else {
            None
        },
        build_graph_v2: Some(graph_v2),
    };
    save_run_cache(&cache_path, &metadata)?;

    Ok(())
}

async fn cmd_check(input: &str) -> Result<()> {
    println!("Checking: {}", input);

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;

    match compile_source(&source, 0) {
        Ok(_) => {
            println!("Type check passed");
            Ok(())
        }
        Err(e) => {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
            Err(miette::miette!("compile failed"))
        }
    }
}

async fn cmd_repl() -> Result<()> {
    println!("Sengoo REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("REPL is not implemented yet");
    println!("type 'exit' to quit");
    Ok(())
}
async fn cmd_dump_ast(input: &str) -> Result<()> {
    println!("Dump AST: {}", input);
    println!("Parser dump_ast is not implemented yet");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        bench_root_dir, build_cache_key, build_graph_v2_for_source, build_metadata_matches,
        build_reflection_metadata, cache_key, cache_mismatch_reasons,
        can_use_incremental_link_with_metadata, can_use_incremental_link_with_run_metadata,
        classify_edit_impact, cmd_build, collect_bench_cases, collect_impl_only_impacted_symbols,
        collect_module_graph_snapshot, compile_ir_to_object, compile_native_binary, compile_source,
        compile_source_with_phase_timings, daemon_request_build, derive_build_workset_plan,
        derive_cached_native_recovery_plan, derive_codegen_workset_manifest,
        derive_run_workset_plan, dispatch_build_via_daemon, edit_class_label,
        ensure_runtime_object, find_clang, find_runtime_c, handle_daemon_client,
        link_native_binary_from_objects, maybe_emit_reflection_sidecar, metadata_matches,
        module_dependency_levels, module_fingerprints_for_source, module_invalidation_stats,
        parse_linker_mode, reflection_options_from_cli, reflection_sidecar_path_for_artifact,
        resolve_bench_suite_path, resolve_daemon_addr, resolve_engine,
        select_reflection_i64_zero_arity_symbol, send_daemon_request, signature_is_zero_arity_i64,
        validate_reflection_metadata, BuildCacheMetadata, BuildGraphNodeV2, BuildGraphV2,
        BuildWorksetPlan, CachedNativeRecoveryPlan, Cli, DaemonDispatchOutcome, EditClass,
        EditImpact, FrontendProbeMode, FunctionFingerprint, LinkerMode, ModuleFingerprint,
        ReflectionMetadata, ReflectionMode, RunCacheMetadata, RunEngine, BUILD_GRAPH_SCHEMA_VERSION,
        DAEMON_PROTOCOL_VERSION, DEFAULT_DAEMON_ADDR,
    };
    use clap::Parser as _;
    use std::collections::{BTreeMap, HashSet};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tokio::net::TcpListener;

    fn fp(path: &str, interface_hash: u64, hash: u64) -> ModuleFingerprint {
        ModuleFingerprint {
            path: path.to_string(),
            interface_hash,
            hash,
        }
    }

    fn metadata_for_test() -> RunCacheMetadata {
        RunCacheMetadata {
            source_hash: 123,
            root_interface_hash: 101,
            root_implementation_hash: 123,
            module_fingerprints: vec![fp("tests/mod_a.sg", 11, 11)],
            opt_level: 1,
            requested_engine: RunEngine::Auto,
            resolved_engine: RunEngine::Native,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/a.ll".to_string(),
            executable_path: Some("tests/build/a.exe".to_string()),
            llvm_ir_hash: 999,
            object_path: Some("tests/build/a.obj".to_string()),
            build_graph_v2: None,
        }
    }

    fn temp_object_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sengoo-sgc-{}-{}.{}",
            name,
            std::process::id(),
            if cfg!(windows) { "obj" } else { "o" }
        ));
        fs::write(&path, b"obj").unwrap();
        path
    }

    fn temp_artifact(name: &str, ext: &str) -> std::path::PathBuf {
        let stem = format!("sengoo-sgc-{}-{}", name, std::process::id());
        if ext.is_empty() {
            std::env::temp_dir().join(stem)
        } else {
            std::env::temp_dir().join(format!("{}.{}", stem, ext))
        }
    }

    fn temp_sg_module(name: &str, source: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sengoo-sgc-reflect-{}-{}.sg",
            name,
            std::process::id()
        ));
        fs::write(&path, source).unwrap();
        path
    }

    fn reflection_graph_for_module(path: &Path) -> BuildGraphV2 {
        let module_id = super::canonical_or_lossy(path);
        BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: module_id.clone(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: module_id,
                interface_hash: 1,
                implementation_hash: 1,
                depends_on: Vec::new(),
                object_path: None,
                functions: Vec::new(),
            }],
        }
    }

    fn classify_root_edit(before: &str, after: &str) -> super::EditImpact {
        let before_interface = super::interface_fingerprint(before);
        let before_impl = super::implementation_fingerprint(before);
        let after_interface = super::interface_fingerprint(after);
        let after_impl = super::implementation_fingerprint(after);
        let mut edges = BTreeMap::new();
        edges.insert(
            super::canonical_or_lossy(Path::new("tests/main.sg")),
            Vec::new(),
        );
        let graph = build_graph_v2_for_source(
            Path::new("tests/main.sg"),
            &[],
            &edges,
            None,
            after_interface,
            after_impl,
        );
        classify_edit_impact(
            before_interface,
            before_impl,
            after_interface,
            after_impl,
            &[],
            &[],
            None,
            &graph,
        )
    }

    #[test]
    fn auto_prefers_native_when_available() {
        let resolved = resolve_engine(RunEngine::Auto, true, true).unwrap();
        assert_eq!(resolved, RunEngine::Native);
    }

    #[test]
    fn auto_falls_back_to_lli_when_native_unavailable() {
        let resolved = resolve_engine(RunEngine::Auto, false, true).unwrap();
        assert_eq!(resolved, RunEngine::Lli);
    }

    #[test]
    fn explicit_engine_is_validated() {
        assert!(resolve_engine(RunEngine::Native, false, true).is_err());
        assert!(resolve_engine(RunEngine::Lli, true, false).is_err());
    }

    #[test]
    fn linker_mode_defaults_to_auto() {
        assert_eq!(parse_linker_mode(None), LinkerMode::Auto);
        assert_eq!(parse_linker_mode(Some("")), LinkerMode::Auto);
        assert_eq!(parse_linker_mode(Some("unknown")), LinkerMode::Auto);
    }

    #[test]
    fn linker_mode_parses_lld_and_system() {
        assert_eq!(parse_linker_mode(Some("lld")), LinkerMode::Lld);
        assert_eq!(parse_linker_mode(Some("system")), LinkerMode::System);
        assert_eq!(parse_linker_mode(Some(" LLD ")), LinkerMode::Lld);
    }

    #[test]
    fn cache_miss_when_opt_level_changes() {
        let metadata = metadata_for_test();
        let key = cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 11)],
            2,
            RunEngine::Auto,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c".to_string()),
        );
        assert!(!metadata_matches(&metadata, &key));
    }

    #[test]
    fn cache_miss_when_engine_changes() {
        let metadata = metadata_for_test();
        let key = cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 11)],
            1,
            RunEngine::Auto,
            RunEngine::Lli,
            Some("tools/stdlib/runtime.c".to_string()),
        );
        assert!(!metadata_matches(&metadata, &key));
    }

    #[test]
    fn cache_hit_when_key_matches() {
        let metadata = metadata_for_test();
        let key = cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 11)],
            1,
            RunEngine::Auto,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c".to_string()),
        );
        assert!(metadata_matches(&metadata, &key));
    }

    #[test]
    fn cache_miss_when_module_dependency_changes() {
        let metadata = metadata_for_test();
        let key = cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 99)],
            1,
            RunEngine::Auto,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c".to_string()),
        );
        assert!(!metadata_matches(&metadata, &key));
    }

    #[test]
    fn cache_mismatch_reasons_include_module_changes() {
        let metadata = metadata_for_test();
        let key = cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 99)],
            1,
            RunEngine::Auto,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c".to_string()),
        );
        let reasons = cache_mismatch_reasons(&metadata, &key);
        assert!(reasons
            .iter()
            .any(|r| r.contains("module implementations changed")));
    }

    #[test]
    fn benchmark_scaffold_exists() {
        let root = bench_root_dir();
        assert!(root.join("baseline.json").exists());
        assert!(root.join("suites/runtime/basic_loop.sg").exists());
        assert!(root.join("suites/compile/mod_tree_root.sg").exists());
        assert!(root.join("suites/incremental/change_impl_root.sg").exists());
        assert!(root.join("suites/incremental/math_util.sg").exists());
    }

    #[test]
    fn bench_subcommands_parse() {
        assert!(Cli::try_parse_from(["sgc", "bench", "run", "runtime"]).is_ok());
        assert!(Cli::try_parse_from(["sgc", "bench", "compile", "compile"]).is_ok());
        assert!(Cli::try_parse_from(["sgc", "bench", "incremental", "incremental"]).is_ok());
        assert!(Cli::try_parse_from(["sgc", "bench", "reflection", "runtime"]).is_ok());
    }

    #[test]
    fn build_force_rebuild_flag_parses() {
        assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--force-rebuild"]).is_ok());
    }

    #[test]
    fn daemon_subcommand_parses() {
        assert!(Cli::try_parse_from(["sgc", "daemon"]).is_ok());
        assert!(Cli::try_parse_from(["sgc", "daemon", "--addr", "127.0.0.1:50000"]).is_ok());
    }

    #[test]
    fn build_and_run_daemon_flags_parse() {
        assert!(Cli::try_parse_from(["sgc", "build", "tests/demo.sg", "--daemon"]).is_ok());
        assert!(Cli::try_parse_from([
            "sgc",
            "run",
            "tests/demo.sg",
            "--daemon",
            "--daemon-addr",
            "127.0.0.1:50000",
        ])
        .is_ok());
    }

    #[test]
    fn reflection_flags_parse_for_build_and_run() {
        assert!(Cli::try_parse_from([
            "sgc",
            "build",
            "tests/demo.sg",
            "--reflect",
            "--reflect-module",
            "tests/demo.sg",
            "--reflect-symbol",
            "tests/demo.sg::main",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "sgc",
            "run",
            "tests/demo.sg",
            "--reflect",
            "--reflect-symbol",
            "tests/demo.sg::main",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "sgc",
            "build",
            "tests/demo.sg",
            "--reflect=off",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "sgc",
            "run",
            "tests/demo.sg",
            "--reflect=auto",
        ])
        .is_ok());
    }

    #[test]
    fn source_requests_reflection_detects_common_import_forms() {
        assert!(super::source_requests_reflection("import reflect;\ndef main() -> i64 { 0 }\n"));
        assert!(super::source_requests_reflection(
            "import std::reflect;\ndef main() -> i64 { 0 }\n"
        ));
        assert!(super::source_requests_reflection(
            "import std{io, reflect};\ndef main() -> i64 { 0 }\n"
        ));
        assert!(!super::source_requests_reflection(
            "import std::io;\ndef main() -> i64 { 0 }\n"
        ));
    }

    #[test]
    fn reflection_auto_mode_enables_when_dependency_imports_reflect() {
        let root = std::env::temp_dir().join(format!(
            "sengoo-sgc-reflect-auto-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let root_module = root.join("main.sg");
        let dep_module = root.join("util.sg");
        let std_dir = root.join("std");
        fs::create_dir_all(&std_dir).unwrap();
        let std_reflect = std_dir.join("reflect.sg");

        fs::write(
            &root_module,
            "import util;\ndef main() -> i64 { util_value() }\n",
        )
        .unwrap();
        fs::write(
            &dep_module,
            "import std::reflect;\ndef util_value() -> i64 { 1 }\n",
        )
        .unwrap();
        fs::write(&std_reflect, "def meta_probe() -> i64 { 1 }\n").unwrap();

        let root_source = fs::read_to_string(&root_module).unwrap();
        let snapshot = super::collect_module_graph_snapshot(
            &root_module,
            &root_source,
            None,
            None,
            super::FrontendProbeMode::FastNoVerify,
        );
        let dep_id = super::canonical_or_lossy(&dep_module);
        assert!(snapshot.reflection_import_modules.contains(&dep_id));

        let auto = super::resolve_reflection_options_for_snapshot(
            super::reflection_options_from_cli(ReflectionMode::Auto, &[], &[]),
            &snapshot,
        );
        assert!(auto.enabled);

        let forced_off = super::resolve_reflection_options_for_snapshot(
            super::reflection_options_from_cli(ReflectionMode::Off, &[], &[]),
            &snapshot,
        );
        assert!(!forced_off.enabled);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reflection_signature_parser_detects_zero_arity_i64() {
        assert!(signature_is_zero_arity_i64(
            "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64"
        ));
        assert!(!signature_is_zero_arity_i64(
            "pub|main|async=false|self=-|tp=[]|params=[a:i64]|ret=i64"
        ));
        assert!(!signature_is_zero_arity_i64(
            "pub|main|async=false|self=-|tp=[]|params=[]|ret=bool"
        ));
    }

    #[test]
    fn reflection_symbol_selector_prefers_reflect_probe_over_main() {
        let symbols = vec![
            sengoo_runtime::ReflectionSymbolMetadata {
                symbol: "tests/demo.sg::main".to_string(),
                signature: "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64".to_string(),
                native_symbol: Some("main".to_string()),
            },
            sengoo_runtime::ReflectionSymbolMetadata {
                symbol: "tests/demo.sg::reflect_probe".to_string(),
                signature: "pub|reflect_probe|async=false|self=-|tp=[]|params=[]|ret=i64"
                    .to_string(),
                native_symbol: Some("reflect_probe".to_string()),
            },
        ];
        let picked = select_reflection_i64_zero_arity_symbol(&symbols);
        assert_eq!(picked.as_deref(), Some("reflect_probe"));
    }

    #[test]
    fn reflection_symbol_selector_falls_back_to_main() {
        let symbols = vec![sengoo_runtime::ReflectionSymbolMetadata {
            symbol: "tests/demo.sg::main".to_string(),
            signature: "pub|main|async=false|self=-|tp=[]|params=[]|ret=i64".to_string(),
            native_symbol: Some("main".to_string()),
        }];
        let picked = select_reflection_i64_zero_arity_symbol(&symbols);
        assert_eq!(picked.as_deref(), Some("main"));
    }

    #[test]
    fn reflection_symbol_selector_returns_none_without_supported_signature() {
        let symbols = vec![sengoo_runtime::ReflectionSymbolMetadata {
            symbol: "tests/demo.sg::flag".to_string(),
            signature: "pub|flag|async=false|self=-|tp=[]|params=[]|ret=bool".to_string(),
            native_symbol: Some("flag".to_string()),
        }];
        assert!(select_reflection_i64_zero_arity_symbol(&symbols).is_none());
    }

    #[test]
    fn daemon_addr_prefers_explicit_value() {
        let addr = resolve_daemon_addr(Some("127.0.0.1:50001"));
        assert_eq!(addr, "127.0.0.1:50001");
    }

    #[test]
    fn reflection_metadata_generation_filters_symbols() {
        let module_path = temp_sg_module(
            "meta-filter",
            "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
        );
        let module_id = super::canonical_or_lossy(&module_path);
        let graph = reflection_graph_for_module(&module_path);
        let options =
            reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
        let metadata = build_reflection_metadata(&graph, &options, None)
            .unwrap()
            .expect("reflection metadata");
        assert_eq!(metadata.schema_version, 1);
        assert_eq!(metadata.modules.len(), 1);
        assert_eq!(metadata.modules[0].symbols.len(), 1);
        assert_eq!(
            metadata.modules[0].symbols[0].symbol,
            format!("{}::add", module_id)
        );
        validate_reflection_metadata(&metadata).unwrap();
        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn reflection_metadata_generation_accepts_short_symbol_selector() {
        let module_path = temp_sg_module(
            "meta-short-filter",
            "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
        );
        let module_id = super::canonical_or_lossy(&module_path);
        let graph = reflection_graph_for_module(&module_path);
        let options = reflection_options_from_cli(ReflectionMode::On, &[], &[String::from("add")]);
        let metadata = build_reflection_metadata(&graph, &options, None)
            .unwrap()
            .expect("reflection metadata");
        assert_eq!(metadata.modules.len(), 1);
        assert_eq!(metadata.modules[0].symbols.len(), 1);
        assert_eq!(
            metadata.modules[0].symbols[0].symbol,
            format!("{}::add", module_id)
        );
        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn reflection_metadata_assigns_native_symbol_when_llvm_symbol_available() {
        let module_path = temp_sg_module(
            "meta-native-symbol",
            "def add(a: i64, b: i64) -> i64 { a + b }\ndef sub(a: i64, b: i64) -> i64 { a - b }\n",
        );
        let module_id = super::canonical_or_lossy(&module_path);
        let graph = reflection_graph_for_module(&module_path);
        let options =
            reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
        let llvm_defined = HashSet::from([String::from("add")]);
        let metadata = build_reflection_metadata(&graph, &options, Some(&llvm_defined))
            .unwrap()
            .expect("reflection metadata");

        assert_eq!(metadata.modules.len(), 1);
        assert_eq!(metadata.modules[0].symbols.len(), 1);
        assert_eq!(
            metadata.modules[0].symbols[0].native_symbol.as_deref(),
            Some("add")
        );
        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn reflection_metadata_rejects_symbol_missing_from_llvm_ir() {
        let module_path = temp_sg_module(
            "meta-missing-llvm",
            "def add(a: i64, b: i64) -> i64 { a + b }\n",
        );
        let module_id = super::canonical_or_lossy(&module_path);
        let graph = reflection_graph_for_module(&module_path);
        let options =
            reflection_options_from_cli(ReflectionMode::On, &[], &[format!("{}::add", module_id)]);
        let llvm_defined = HashSet::<String>::new();
        let err = build_reflection_metadata(&graph, &options, Some(&llvm_defined)).unwrap_err();
        assert!(err
            .to_string()
            .contains("is not emitted in LLVM IR (native symbol: add)"));
        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn reflection_metadata_rejects_unknown_symbol() {
        let module_path =
            temp_sg_module("meta-unknown", "def add(a: i64, b: i64) -> i64 { a + b }\n");
        let module_id = super::canonical_or_lossy(&module_path);
        let graph = reflection_graph_for_module(&module_path);
        let options = reflection_options_from_cli(
            ReflectionMode::On,
            &[],
            &[format!("{}::missing", module_id)],
        );
        let err = build_reflection_metadata(&graph, &options, None).unwrap_err();
        assert!(err
            .to_string()
            .contains("reflection symbol(s) not found in selected modules"));
        let _ = fs::remove_file(module_path);
    }

    #[test]
    fn reflection_sidecar_emit_and_disabled_cleanup() {
        let module_path = temp_sg_module("sidecar", "def main() -> i64 { 1 }\n");
        let graph = reflection_graph_for_module(&module_path);
        let artifact = temp_artifact("reflect-sidecar", "exe");

        let options = reflection_options_from_cli(ReflectionMode::On, &[], &[]);
        maybe_emit_reflection_sidecar(&artifact, &graph, &options, None).unwrap();
        let sidecar_path = reflection_sidecar_path_for_artifact(&artifact);
        assert!(sidecar_path.exists());
        let metadata: ReflectionMetadata =
            serde_json::from_slice(&fs::read(&sidecar_path).unwrap()).unwrap();
        validate_reflection_metadata(&metadata).unwrap();

        let disabled = reflection_options_from_cli(ReflectionMode::Off, &[], &[]);
        maybe_emit_reflection_sidecar(&artifact, &graph, &disabled, None).unwrap();
        assert!(!sidecar_path.exists());

        let _ = fs::remove_file(module_path);
        let _ = fs::remove_file(artifact);
    }

    #[test]
    fn daemon_build_request_uses_protocol_and_version() {
        let request = daemon_request_build(
            "tests/demo.sg",
            None,
            2,
            false,
            false,
            ReflectionMode::Off,
            &[],
            &[],
        );
        assert_eq!(request.protocol_version, DAEMON_PROTOCOL_VERSION);
        assert_eq!(request.client_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn daemon_default_addr_constant_has_host_and_port() {
        assert!(DEFAULT_DAEMON_ADDR.contains(':'));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn daemon_happy_path_handles_build_request() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_daemon_client(stream).await.unwrap();
        });

        let input = bench_root_dir().join("tests").join("simple_array.sg");
        let request = daemon_request_build(
            input.to_string_lossy().as_ref(),
            None,
            2,
            false,
            false,
            ReflectionMode::Off,
            &[],
            &[],
        );
        let response = send_daemon_request(&addr.to_string(), &request)
            .await
            .unwrap();
        assert!(response.ok, "{}", response.message);

        server.await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn daemon_and_oneshot_build_emit_same_workset_manifest() {
        let root =
            std::env::temp_dir().join(format!("sengoo-sgc-daemon-parity-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let input = root.join("main.sg");
        let input_text = "def main() -> i64 {\n    1\n}\n";
        fs::write(&input, input_text).unwrap();
        let input_string = input.to_string_lossy().to_string();
        let stem = input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let manifest_path = root
            .join("build")
            .join("workset")
            .join(format!("{}.build.workset.json", stem));

        cmd_build(
            &input_string,
            None,
            2,
            true,
            false,
            super::ReflectionCliOptions::default(),
        )
        .await
        .unwrap();
        let direct_manifest = fs::read_to_string(&manifest_path).unwrap();

        fs::remove_dir_all(root.join("build")).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_daemon_client(stream).await.unwrap();
        });

        let request = daemon_request_build(
            &input_string,
            None,
            2,
            true,
            false,
            ReflectionMode::Off,
            &[],
            &[],
        );
        let response = send_daemon_request(&addr.to_string(), &request)
            .await
            .unwrap();
        assert!(response.ok, "{}", response.message);
        server.await.unwrap();

        let daemon_manifest = fs::read_to_string(&manifest_path).unwrap();
        assert_eq!(direct_manifest, daemon_manifest);

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn daemon_client_fallback_when_server_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let input = bench_root_dir().join("tests").join("simple_array.sg");

        let outcome = dispatch_build_via_daemon(
            &addr.to_string(),
            input.to_string_lossy().as_ref(),
            None,
            2,
            false,
            false,
            ReflectionMode::Off,
            &[],
            &[],
        )
        .await
        .unwrap();
        assert_eq!(outcome, DaemonDispatchOutcome::Fallback);
    }

    #[test]
    fn build_graph_v2_contains_root_and_dependency_nodes() {
        let input = Path::new("tests/main.sg");
        let deps = vec![fp("tests/a.sg", 1, 11), fp("tests/b.sg", 2, 22)];
        let mut edges = BTreeMap::new();
        edges.insert(
            super::canonical_or_lossy(input),
            vec!["tests/a.sg".to_string(), "tests/b.sg".to_string()],
        );
        edges.insert("tests/a.sg".to_string(), Vec::new());
        edges.insert("tests/b.sg".to_string(), Vec::new());
        let graph = build_graph_v2_for_source(input, &deps, &edges, None, 88, 99);
        assert_eq!(graph.schema_version, BUILD_GRAPH_SCHEMA_VERSION);
        assert_eq!(graph.nodes.len(), 3);
        let root_module = super::canonical_or_lossy(input);
        let root = graph
            .nodes
            .iter()
            .find(|node| node.module_path == root_module)
            .expect("root node");
        assert_eq!(root.interface_hash, 88);
        assert_eq!(root.implementation_hash, 99);
        assert_eq!(root.depends_on.len(), 2);
        assert!(root.depends_on.contains(&"tests/a.sg".to_string()));
        assert!(root.depends_on.contains(&"tests/b.sg".to_string()));
    }

    #[test]
    fn build_cache_schema_mismatch_forces_metadata_miss() {
        let key = build_cache_key(
            123,
            vec![fp("tests/mod_a.sg", 11, 11)],
            1,
            false,
            Some("tools/stdlib/runtime.c".to_string()),
            "tests/build/a.exe".to_string(),
        );
        let metadata = BuildCacheMetadata {
            cache_schema_version: 1,
            source_hash: 123,
            root_interface_hash: 101,
            root_implementation_hash: 123,
            module_fingerprints: vec![fp("tests/mod_a.sg", 11, 11)],
            opt_level: 1,
            emit_llvm: false,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/a.ll".to_string(),
            output_path: "tests/build/a.exe".to_string(),
            llvm_ir_hash: 777,
            object_path: Some("tests/build/a.obj".to_string()),
            build_graph_v2: None,
        };
        assert!(!build_metadata_matches(&metadata, &key));
    }

    #[test]
    fn incremental_link_reuse_requires_matching_ir_hash() {
        let object_path = temp_object_file("ir-hash");
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 0,
                implementation_hash: 0,
                depends_on: vec![],
                object_path: Some(object_path.to_string_lossy().to_string()),
                functions: vec![],
            }],
        };
        let metadata = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash: 1,
            root_interface_hash: 1,
            root_implementation_hash: 1,
            module_fingerprints: vec![],
            opt_level: 2,
            emit_llvm: false,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            output_path: "tests/build/main.exe".to_string(),
            llvm_ir_hash: 10,
            object_path: Some(object_path.to_string_lossy().to_string()),
            build_graph_v2: Some(graph.clone()),
        };

        let err = can_use_incremental_link_with_metadata(
            &metadata,
            11,
            &object_path,
            "tests/build/main.exe",
            Some("tools/stdlib/runtime.c"),
            2,
            &graph,
        )
        .unwrap_err();
        assert!(err.contains("LLVM IR changed"));

        let _ = fs::remove_file(&object_path);
    }

    #[test]
    fn run_incremental_link_reuse_accepts_matching_metadata() {
        let object_path = temp_object_file("run-ok");
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 0,
                implementation_hash: 0,
                depends_on: vec![],
                object_path: Some(object_path.to_string_lossy().to_string()),
                functions: vec![],
            }],
        };
        let metadata = RunCacheMetadata {
            source_hash: 1,
            root_interface_hash: 1,
            root_implementation_hash: 1,
            module_fingerprints: vec![],
            opt_level: 2,
            requested_engine: RunEngine::Native,
            resolved_engine: RunEngine::Native,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            executable_path: Some("tests/build/main.exe".to_string()),
            llvm_ir_hash: 44,
            object_path: Some(object_path.to_string_lossy().to_string()),
            build_graph_v2: Some(graph.clone()),
        };

        assert!(can_use_incremental_link_with_run_metadata(
            &metadata,
            44,
            &object_path,
            Some("tools/stdlib/runtime.c"),
            2,
            RunEngine::Native,
            RunEngine::Native,
            &graph,
        )
        .is_ok());

        let _ = fs::remove_file(&object_path);
    }

    #[test]
    fn cached_native_recovery_prefers_existing_object() {
        let plan = derive_cached_native_recovery_plan(true, true);
        assert_eq!(plan, Some(CachedNativeRecoveryPlan::RelinkFromObject));
    }

    #[test]
    fn cached_native_recovery_can_rebuild_object_from_ir() {
        let plan = derive_cached_native_recovery_plan(true, false);
        assert_eq!(
            plan,
            Some(CachedNativeRecoveryPlan::RebuildObjectFromCachedIr)
        );
    }

    #[test]
    fn cached_native_recovery_requires_cached_ir_or_object() {
        let plan = derive_cached_native_recovery_plan(false, false);
        assert_eq!(plan, None);
    }

    #[test]
    fn incremental_link_output_matches_full_link_output() {
        let Some(clang) = find_clang() else {
            return;
        };

        let source = "def main() -> i64 { 0 }\n";
        let llvm_ir = compile_source(source, 2).unwrap();
        let ll_path = temp_artifact("equiv-main", "ll");
        fs::write(&ll_path, llvm_ir).unwrap();

        let full_exe = temp_artifact("equiv-full", if cfg!(windows) { "exe" } else { "" });
        let inc_exe = temp_artifact("equiv-inc", if cfg!(windows) { "exe" } else { "" });
        let obj_path = temp_artifact("equiv-main", if cfg!(windows) { "obj" } else { "o" });

        let runtime_c = find_runtime_c();
        compile_native_binary(&clang, &ll_path, &full_exe, runtime_c.as_deref(), 2).unwrap();
        compile_ir_to_object(&clang, &ll_path, &obj_path, 2).unwrap();

        let mut object_paths = vec![obj_path.clone()];
        if let Some(runtime_c) = runtime_c.as_deref() {
            object_paths.push(ensure_runtime_object(&clang, runtime_c, 2).unwrap());
        }
        link_native_binary_from_objects(&clang, &object_paths, &inc_exe).unwrap();

        let full_out = Command::new(&full_exe).output().unwrap();
        let inc_out = Command::new(&inc_exe).output().unwrap();
        assert_eq!(full_out.status.code(), inc_out.status.code());
        assert_eq!(full_out.stdout, inc_out.stdout);
        assert_eq!(full_out.stderr, inc_out.stderr);

        let _ = fs::remove_file(&ll_path);
        let _ = fs::remove_file(&obj_path);
        let _ = fs::remove_file(&full_exe);
        let _ = fs::remove_file(&inc_exe);
    }

    #[test]
    fn runtime_suite_name_prefers_bench_directory() {
        let suite_path = resolve_bench_suite_path("runtime", "runtime").unwrap();
        assert!(suite_path.ends_with(Path::new("bench").join("suites").join("runtime")));
        let cases = collect_bench_cases(&suite_path).unwrap();
        assert!(!cases.is_empty());
    }

    #[test]
    fn compile_phase_timings_include_expected_keys() {
        let source = "def main() -> i64 { 0 }";
        let (_, phases) = compile_source_with_phase_timings(source, 2).unwrap();
        assert!(phases.contains_key("parse"));
        assert!(phases.contains_key("typeck"));
        assert!(phases.contains_key("mir"));
        assert!(phases.contains_key("mir_prune"));
        assert!(phases.contains_key("codegen"));
        assert!(phases.contains_key("link"));
    }

    #[test]
    fn compile_source_prunes_unreachable_functions_from_ir() {
        let source = r#"
def live() -> i64 { 1 }
def unused_xyz_dead() -> i64 { 42 }
def main() -> i64 { live() }
"#;
        let llvm_ir = compile_source(source, 2).unwrap();
        assert!(llvm_ir.contains("live"));
        assert!(llvm_ir.contains("main"));
        assert!(
            !llvm_ir.contains("unused_xyz_dead"),
            "unreachable function should be pruned from LLVM IR"
        );
    }

    #[test]
    fn compile_source_without_main_keeps_functions() {
        let source = r#"
def keep_alpha() -> i64 { 1 }
def keep_beta() -> i64 { keep_alpha() + 1 }
"#;
        let llvm_ir = compile_source(source, 2).unwrap();
        assert!(llvm_ir.contains("keep_alpha"));
        assert!(llvm_ir.contains("keep_beta"));
    }

    #[test]
    fn edit_classifier_detects_noop_for_comment_only_change() {
        let before = "def main() -> i64 {\n    0\n}\n";
        let after = "def main() -> i64 {\n    0\n}\n// comment-only change\n";
        let impact = classify_root_edit(before, after);
        assert_eq!(
            impact.class,
            EditClass::Noop,
            "{}",
            edit_class_label(impact.class)
        );
    }

    #[test]
    fn edit_classifier_detects_impl_only_for_loop_body_change() {
        let before = r#"
def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 10 {
        acc = acc + i
        i = i + 1
    }
    acc
}
"#;
        let after = r#"
def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 10 {
        acc = acc + i + 1
        i = i + 1
    }
    acc
}
"#;
        let impact = classify_root_edit(before, after);
        assert_eq!(
            impact.class,
            EditClass::ImplOnly,
            "{}",
            edit_class_label(impact.class)
        );
    }

    #[test]
    fn edit_classifier_detects_interface_change_for_signature_change() {
        let before = "def add(x: i64) -> i64 { x + 1 }\ndef main() -> i64 { add(1) }\n";
        let after = "def add(x: i64, k: i64) -> i64 { x + k }\ndef main() -> i64 { add(1, 1) }\n";
        let impact = classify_root_edit(before, after);
        assert_eq!(impact.class, EditClass::InterfaceChange);
    }

    #[test]
    fn edit_classifier_detects_interface_change_for_add_new_function() {
        let before = "def main() -> i64 { 0 }\n";
        let after = "def extra(x: i64) -> i64 { x + 1 }\ndef main() -> i64 { extra(0) }\n";
        let impact = classify_root_edit(before, after);
        assert_eq!(impact.class, EditClass::InterfaceChange);
    }

    #[test]
    fn interface_change_propagates_to_dependents() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![
                BuildGraphNodeV2 {
                    module_path: "tests/main.sg".to_string(),
                    interface_hash: 7,
                    implementation_hash: 9,
                    depends_on: vec!["tests/dep.sg".to_string()],
                    object_path: None,
                    functions: vec![],
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 11,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
                },
            ],
        };
        let before = vec![fp("tests/dep.sg", 1, 11)];
        let after = vec![fp("tests/dep.sg", 2, 11)];
        let impact = classify_edit_impact(7, 9, 7, 9, &before, &after, Some(&graph), &graph);
        assert_eq!(impact.class, EditClass::InterfaceChange);
        assert!(impact
            .impacted_modules
            .contains(&"tests/dep.sg".to_string()));
        assert!(impact
            .impacted_modules
            .contains(&"tests/main.sg".to_string()));
    }

    #[test]
    fn impl_only_change_does_not_propagate_to_dependents() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![
                BuildGraphNodeV2 {
                    module_path: "tests/main.sg".to_string(),
                    interface_hash: 7,
                    implementation_hash: 9,
                    depends_on: vec!["tests/dep.sg".to_string()],
                    object_path: None,
                    functions: vec![],
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 11,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
                },
            ],
        };
        let before = vec![fp("tests/dep.sg", 1, 11)];
        let after = vec![fp("tests/dep.sg", 1, 12)];
        let impact = classify_edit_impact(7, 9, 7, 9, &before, &after, Some(&graph), &graph);
        assert_eq!(impact.class, EditClass::ImplOnly);
        assert_eq!(impact.impacted_modules, vec!["tests/dep.sg".to_string()]);
    }

    #[test]
    fn dependency_levels_follow_topological_order() {
        let mut edges = BTreeMap::new();
        edges.insert("main".to_string(), vec!["a".to_string(), "b".to_string()]);
        edges.insert("a".to_string(), vec!["c".to_string()]);
        edges.insert("b".to_string(), vec!["d".to_string()]);
        edges.insert("c".to_string(), Vec::new());
        edges.insert("d".to_string(), Vec::new());

        let levels = module_dependency_levels(&edges);
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["c".to_string(), "d".to_string()]);
        assert_eq!(levels[1], vec!["a".to_string(), "b".to_string()]);
        assert_eq!(levels[2], vec!["main".to_string()]);
    }

    #[test]
    fn dependency_levels_keep_cycle_output_deterministic() {
        let mut edges = BTreeMap::new();
        edges.insert("a".to_string(), vec!["b".to_string()]);
        edges.insert("b".to_string(), vec!["a".to_string()]);
        edges.insert("c".to_string(), Vec::new());

        let levels = module_dependency_levels(&edges);
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0], vec!["c".to_string()]);
        assert_eq!(levels[1], vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn dependency_fingerprints_are_deterministic_with_parallel_collection() {
        let root_dir = std::env::temp_dir().join(format!("sengoo-mod-fp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root_dir);
        fs::create_dir_all(&root_dir).unwrap();

        let main_path = root_dir.join("main.sg");
        let dep_a = root_dir.join("dep_a.sg");
        let dep_b = root_dir.join("dep_b.sg");

        fs::write(
            &main_path,
            "import dep_b;\nimport dep_a;\ndef main() -> i64 {\n    0\n}\n",
        )
        .unwrap();
        fs::write(&dep_a, "def a() -> i64 {\n    1\n}\n").unwrap();
        fs::write(&dep_b, "def b() -> i64 {\n    2\n}\n").unwrap();

        let source = fs::read_to_string(&main_path).unwrap();
        let first = module_fingerprints_for_source(&main_path, &source);
        for _ in 0..5 {
            let current = module_fingerprints_for_source(&main_path, &source);
            assert_eq!(current, first);
        }

        let _ = fs::remove_dir_all(&root_dir);
    }

    #[test]
    fn incremental_fixture_retains_dependency_fingerprint_on_impl_change() {
        let root = bench_root_dir();
        let case = root.join("suites/incremental/change_impl_root.sg");
        let original = fs::read_to_string(&case).unwrap();
        let before = module_fingerprints_for_source(&case, &original);

        let mut mutated = original.clone();
        mutated.push_str("\n// test-mut\n");
        let after = module_fingerprints_for_source(&case, &mutated);
        let stats = module_invalidation_stats(&before, &after);
        assert!(
            stats.reused_modules >= 1,
            "expected at least one reused dependency module"
        );
    }

    #[test]
    fn impl_only_change_does_not_rebuild_all_modules() {
        let before = vec![
            fp("tests/mod_a.sg", 100, 1000),
            fp("tests/mod_b.sg", 200, 2000),
        ];
        let after = vec![
            // implementation changed, interface unchanged
            fp("tests/mod_a.sg", 100, 1999),
            // unchanged module should be reused
            fp("tests/mod_b.sg", 200, 2000),
        ];

        let stats = module_invalidation_stats(&before, &after);
        assert_eq!(stats.total_modules, 2);
        assert_eq!(stats.implementation_only_changed_modules, 1);
        assert_eq!(stats.reused_modules, 1);
        assert!(
            stats.rebuilt_modules < stats.total_modules,
            "impl-only change should not force all modules to rebuild"
        );
    }

    #[test]
    fn function_fingerprint_comment_only_change_keeps_hashes() {
        let before = r#"
def add(x: i64) -> i64 {
    x + 1
}
"#;
        let after = r#"
def add(x: i64) -> i64 {
    // comment-only change
    x + 1
}
"#;

        let before_fp = super::function_fingerprints_for_module("tests/main.sg", before);
        let after_fp = super::function_fingerprints_for_module("tests/main.sg", after);

        assert_eq!(before_fp.len(), 1);
        assert_eq!(after_fp.len(), 1);
        assert_eq!(before_fp[0].abi_hash, after_fp[0].abi_hash);
        assert_eq!(before_fp[0].body_hash, after_fp[0].body_hash);
    }

    #[test]
    fn function_fingerprint_signature_change_updates_abi_hash() {
        let before = "def add(x: i64) -> i64 { x + 1 }\n";
        let after = "def add(x: i64, k: i64) -> i64 { x + k }\n";

        let before_fp = super::function_fingerprints_for_module("tests/main.sg", before);
        let after_fp = super::function_fingerprints_for_module("tests/main.sg", after);

        assert_eq!(before_fp.len(), 1);
        assert_eq!(after_fp.len(), 1);
        assert_ne!(before_fp[0].abi_hash, after_fp[0].abi_hash);
    }

    #[test]
    fn function_interface_change_propagates_via_call_edges() {
        let previous_graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec![],
                object_path: None,
                functions: vec![
                    FunctionFingerprint {
                        symbol: "tests/main.sg::add".to_string(),
                        abi_hash: 11,
                        body_hash: 101,
                        calls: vec![],
                        module_imports: vec![],
                    },
                    FunctionFingerprint {
                        symbol: "tests/main.sg::main".to_string(),
                        abi_hash: 12,
                        body_hash: 102,
                        calls: vec!["tests/main.sg::add".to_string()],
                        module_imports: vec![],
                    },
                ],
            }],
        };

        let current_graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 7,
                implementation_hash: 9,
                depends_on: vec![],
                object_path: None,
                functions: vec![
                    FunctionFingerprint {
                        symbol: "tests/main.sg::add".to_string(),
                        abi_hash: 999,
                        body_hash: 101,
                        calls: vec![],
                        module_imports: vec![],
                    },
                    FunctionFingerprint {
                        symbol: "tests/main.sg::main".to_string(),
                        abi_hash: 12,
                        body_hash: 102,
                        calls: vec!["tests/main.sg::add".to_string()],
                        module_imports: vec![],
                    },
                ],
            }],
        };

        let impact =
            classify_edit_impact(7, 9, 7, 9, &[], &[], Some(&previous_graph), &current_graph);
        assert_eq!(impact.class, EditClass::InterfaceChange);
        assert!(impact
            .impacted_functions
            .contains(&"tests/main.sg::main".to_string()));
        assert!(impact
            .impacted_functions
            .contains(&"tests/main.sg::add".to_string()));
    }

    #[test]
    fn impl_only_impacted_symbols_expand_to_transitive_callers() {
        let previous = vec![
            FunctionFingerprint {
                symbol: "tests/main.sg::leaf".to_string(),
                abi_hash: 11,
                body_hash: 101,
                calls: vec![],
                module_imports: vec![],
            },
            FunctionFingerprint {
                symbol: "tests/main.sg::mid".to_string(),
                abi_hash: 12,
                body_hash: 102,
                calls: vec!["tests/main.sg::leaf".to_string()],
                module_imports: vec![],
            },
            FunctionFingerprint {
                symbol: "tests/main.sg::top".to_string(),
                abi_hash: 13,
                body_hash: 103,
                calls: vec!["tests/main.sg::mid".to_string()],
                module_imports: vec![],
            },
        ];
        let current = vec![
            FunctionFingerprint {
                symbol: "tests/main.sg::leaf".to_string(),
                abi_hash: 11,
                body_hash: 999,
                calls: vec![],
                module_imports: vec![],
            },
            FunctionFingerprint {
                symbol: "tests/main.sg::mid".to_string(),
                abi_hash: 12,
                body_hash: 102,
                calls: vec!["tests/main.sg::leaf".to_string()],
                module_imports: vec![],
            },
            FunctionFingerprint {
                symbol: "tests/main.sg::top".to_string(),
                abi_hash: 13,
                body_hash: 103,
                calls: vec!["tests/main.sg::mid".to_string()],
                module_imports: vec![],
            },
        ];

        let impacted = collect_impl_only_impacted_symbols(&previous, &current);
        assert_eq!(
            impacted,
            vec![
                "tests/main.sg::leaf".to_string(),
                "tests/main.sg::mid".to_string(),
                "tests/main.sg::top".to_string(),
            ]
        );
    }

    #[test]
    fn workset_plan_reuses_previous_artifacts_when_impl_only_does_not_touch_root() {
        let previous = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash: 1,
            root_interface_hash: 10,
            root_implementation_hash: 20,
            module_fingerprints: vec![],
            opt_level: 2,
            emit_llvm: false,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            output_path: "tests/build/main.exe".to_string(),
            llvm_ir_hash: 33,
            object_path: Some("tests/build/main.obj".to_string()),
            build_graph_v2: None,
        };
        let impact = EditImpact {
            class: EditClass::ImplOnly,
            changed_modules: vec!["tests/dep.sg".to_string()],
            impacted_modules: vec!["tests/dep.sg".to_string()],
            changed_functions: vec!["tests/dep.sg::add".to_string()],
            impacted_functions: vec!["tests/dep.sg::add".to_string()],
        };

        let plan = derive_build_workset_plan(
            Some(&previous),
            Some(&impact),
            "tests/main.sg",
            false,
            2,
            "tests/build/main.exe",
            Some("tools/stdlib/runtime.c"),
        );
        assert_eq!(plan, BuildWorksetPlan::ReusePreviousArtifacts);
    }

    #[test]
    fn workset_plan_rebuilds_root_when_impl_only_touches_root() {
        let previous = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash: 1,
            root_interface_hash: 10,
            root_implementation_hash: 20,
            module_fingerprints: vec![],
            opt_level: 2,
            emit_llvm: false,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            output_path: "tests/build/main.exe".to_string(),
            llvm_ir_hash: 33,
            object_path: Some("tests/build/main.obj".to_string()),
            build_graph_v2: None,
        };
        let impact = EditImpact {
            class: EditClass::ImplOnly,
            changed_modules: vec!["tests/main.sg".to_string()],
            impacted_modules: vec!["tests/main.sg".to_string()],
            changed_functions: vec!["tests/main.sg::main".to_string()],
            impacted_functions: vec!["tests/main.sg::main".to_string()],
        };

        let plan = derive_build_workset_plan(
            Some(&previous),
            Some(&impact),
            "tests/main.sg",
            false,
            2,
            "tests/build/main.exe",
            Some("tools/stdlib/runtime.c"),
        );
        assert_eq!(plan, BuildWorksetPlan::RebuildImpactedRoot);
    }

    #[test]
    fn codegen_workset_manifest_reuse_marks_all_modules_reusable() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![
                BuildGraphNodeV2 {
                    module_path: "tests/main.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 10,
                    depends_on: vec!["tests/dep.sg".to_string()],
                    object_path: None,
                    functions: vec![],
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 2,
                    implementation_hash: 20,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
                },
            ],
        };
        let impact = EditImpact {
            class: EditClass::ImplOnly,
            changed_modules: vec!["tests/dep.sg".to_string()],
            impacted_modules: vec!["tests/dep.sg".to_string()],
            changed_functions: vec!["tests/dep.sg::add".to_string()],
            impacted_functions: vec!["tests/dep.sg::add".to_string()],
        };

        let manifest = derive_codegen_workset_manifest(
            &graph,
            Some(&impact),
            BuildWorksetPlan::ReusePreviousArtifacts,
        );
        assert!(manifest.rebuild_modules.is_empty());
        assert_eq!(
            manifest.reuse_modules,
            vec!["tests/dep.sg".to_string(), "tests/main.sg".to_string()]
        );
    }

    #[test]
    fn codegen_workset_manifest_full_rebuild_marks_all_modules_rebuild() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![
                BuildGraphNodeV2 {
                    module_path: "tests/main.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 10,
                    depends_on: vec!["tests/dep.sg".to_string()],
                    object_path: None,
                    functions: vec![],
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 2,
                    implementation_hash: 20,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![],
                },
            ],
        };

        let manifest = derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::FullRebuild);
        assert_eq!(
            manifest.rebuild_modules,
            vec!["tests/dep.sg".to_string(), "tests/main.sg".to_string()]
        );
        assert!(manifest.reuse_modules.is_empty());
    }

    #[test]
    fn codegen_workset_manifest_rebuild_root_defaults_to_root_when_impact_absent() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![BuildGraphNodeV2 {
                module_path: "tests/main.sg".to_string(),
                interface_hash: 1,
                implementation_hash: 10,
                depends_on: vec![],
                object_path: None,
                functions: vec![],
            }],
        };

        let manifest =
            derive_codegen_workset_manifest(&graph, None, BuildWorksetPlan::RebuildImpactedRoot);
        assert_eq!(manifest.rebuild_modules, vec!["tests/main.sg".to_string()]);
        assert!(manifest.reuse_modules.is_empty());
    }

    #[test]
    fn codegen_workset_manifest_rebuild_root_tracks_symbol_frontier() {
        let graph = BuildGraphV2 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            root_module: "tests/main.sg".to_string(),
            nodes: vec![
                BuildGraphNodeV2 {
                    module_path: "tests/main.sg".to_string(),
                    interface_hash: 1,
                    implementation_hash: 10,
                    depends_on: vec!["tests/dep.sg".to_string()],
                    object_path: None,
                    functions: vec![FunctionFingerprint {
                        symbol: "tests/main.sg::main".to_string(),
                        abi_hash: 11,
                        body_hash: 101,
                        calls: vec!["tests/dep.sg::add".to_string()],
                        module_imports: vec!["tests/dep.sg".to_string()],
                    }],
                },
                BuildGraphNodeV2 {
                    module_path: "tests/dep.sg".to_string(),
                    interface_hash: 2,
                    implementation_hash: 20,
                    depends_on: vec![],
                    object_path: None,
                    functions: vec![FunctionFingerprint {
                        symbol: "tests/dep.sg::add".to_string(),
                        abi_hash: 12,
                        body_hash: 102,
                        calls: vec![],
                        module_imports: vec![],
                    }],
                },
            ],
        };
        let impact = EditImpact {
            class: EditClass::ImplOnly,
            changed_modules: vec!["tests/dep.sg".to_string()],
            impacted_modules: vec!["tests/dep.sg".to_string()],
            changed_functions: vec!["tests/dep.sg::add".to_string()],
            impacted_functions: vec!["tests/dep.sg::add".to_string()],
        };

        let manifest = derive_codegen_workset_manifest(
            &graph,
            Some(&impact),
            BuildWorksetPlan::RebuildImpactedRoot,
        );
        assert_eq!(
            manifest.changed_symbols,
            vec!["tests/dep.sg::add".to_string()]
        );
        assert_eq!(
            manifest.impacted_symbols,
            vec!["tests/dep.sg::add".to_string()]
        );
        assert_eq!(
            manifest.rebuild_symbols,
            vec!["tests/dep.sg::add".to_string()]
        );
        assert_eq!(
            manifest.reuse_symbols,
            vec!["tests/main.sg::main".to_string()]
        );
    }

    #[test]
    fn frontend_session_reuses_modules_when_source_is_unchanged() {
        let root = std::env::temp_dir().join(format!(
            "sengoo-sgc-frontend-session-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let input = root.join("main.sg");
        let source = "def main() -> i64 {\n    1\n}\n";
        fs::write(&input, source).unwrap();

        let first = collect_module_graph_snapshot(
            &input,
            source,
            None,
            None,
            FrontendProbeMode::FastNoVerify,
        );
        assert!(first.reused_modules.is_empty());
        assert!(!first.rebuilt_modules.is_empty());

        let second = collect_module_graph_snapshot(
            &input,
            source,
            None,
            Some(&first.frontend_session_store),
            FrontendProbeMode::VerifyChangedAndDependents,
        );
        assert!(second.diagnostics.is_empty());
        assert_eq!(second.rebuilt_modules.len(), 0);
        assert_eq!(
            second.reused_modules.len(),
            first.frontend_session_store.modules.len()
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn run_workset_plan_reuses_previous_artifacts_when_impl_only_does_not_touch_root() {
        let previous = RunCacheMetadata {
            source_hash: 1,
            root_interface_hash: 10,
            root_implementation_hash: 20,
            module_fingerprints: vec![],
            opt_level: 2,
            requested_engine: RunEngine::Auto,
            resolved_engine: RunEngine::Native,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            executable_path: Some("tests/build/main.exe".to_string()),
            llvm_ir_hash: 33,
            object_path: Some("tests/build/main.obj".to_string()),
            build_graph_v2: None,
        };
        let impact = EditImpact {
            class: EditClass::ImplOnly,
            changed_modules: vec!["tests/dep.sg".to_string()],
            impacted_modules: vec!["tests/dep.sg".to_string()],
            changed_functions: vec!["tests/dep.sg::add".to_string()],
            impacted_functions: vec!["tests/dep.sg::add".to_string()],
        };

        let plan = derive_run_workset_plan(
            Some(&previous),
            Some(&impact),
            "tests/main.sg",
            2,
            RunEngine::Auto,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c"),
        );
        assert_eq!(plan, BuildWorksetPlan::ReusePreviousArtifacts);
    }

    #[test]
    fn run_workset_plan_full_rebuild_when_engine_changes() {
        let previous = RunCacheMetadata {
            source_hash: 1,
            root_interface_hash: 10,
            root_implementation_hash: 20,
            module_fingerprints: vec![],
            opt_level: 2,
            requested_engine: RunEngine::Auto,
            resolved_engine: RunEngine::Native,
            runtime_c: Some("tools/stdlib/runtime.c".to_string()),
            llvm_ir_path: "tests/build/main.ll".to_string(),
            executable_path: Some("tests/build/main.exe".to_string()),
            llvm_ir_hash: 33,
            object_path: Some("tests/build/main.obj".to_string()),
            build_graph_v2: None,
        };
        let impact = EditImpact {
            class: EditClass::Noop,
            changed_modules: vec![],
            impacted_modules: vec![],
            changed_functions: vec![],
            impacted_functions: vec![],
        };

        let plan = derive_run_workset_plan(
            Some(&previous),
            Some(&impact),
            "tests/main.sg",
            2,
            RunEngine::Native,
            RunEngine::Native,
            Some("tools/stdlib/runtime.c"),
        );
        assert_eq!(plan, BuildWorksetPlan::FullRebuild);
    }
}
