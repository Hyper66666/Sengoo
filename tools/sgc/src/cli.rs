use clap::{Parser as ClapParser, Subcommand};
use miette::Result;
use std::path::Path;

use crate::{
    cmd_bench_compile, cmd_bench_incremental, cmd_bench_reflection, cmd_bench_run, cmd_build,
    cmd_check, cmd_daemon, cmd_doc, cmd_dump_ast, cmd_repl, cmd_run, cmd_test,
    current_error_format, dispatch_build_via_daemon, dispatch_run_via_daemon,
    frontend_trace_enabled, parse_frontend_jobs_arg,
    portable_backends::{build_bytecode, build_wasm, run_bytecode, run_wasm},
    propagate_run_exit_code, reflection_options_from_cli, resolve_daemon_addr, resolve_test_root,
    set_error_format, ContractChecksMode, DaemonDispatchOutcome, ErrorFormat, FrontendJobs,
    ReflectionMode, RunEngine, TestOptions, TestOutputFormat, DEFAULT_DAEMON_ADDR,
};

pub(crate) const SGC_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("SENGOO_BUILD_HASH"),
    ")"
);

/// Sengoo command-line compiler.
#[derive(ClapParser, Debug)]
#[command(name = "sgc")]
#[command(author = "Sengoo Team")]
#[command(version = SGC_VERSION)]
#[command(about = "Sengoo language compiler", long_about = None)]
pub(crate) struct Cli {
    /// Error output format.
    #[arg(long = "error-format", global = true, value_enum, default_value_t = ErrorFormat::Text)]
    error_format: ErrorFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
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

        /// Runtime contract checks (`auto` enables in O0/O1).
        #[arg(long = "contract-checks", value_enum, default_value_t = ContractChecksMode::Auto)]
        contract_checks: ContractChecksMode,

        /// Emit LLVM IR instead of a native executable.
        #[arg(long)]
        emit_llvm: bool,

        /// Ignore cached build artifacts and rebuild.
        #[arg(long)]
        force_rebuild: bool,

        /// Enable manual low-memory pipeline (trades incremental features for lower RSS).
        #[arg(long = "low-memory")]
        low_memory: bool,

        /// Frontend scheduler workers (`auto` by default, `1` for serial deterministic mode).
        #[arg(long = "frontend-jobs", default_value = "auto", value_parser = parse_frontend_jobs_arg)]
        frontend_jobs: FrontendJobs,

        /// Emit deterministic frontend planner trace lines.
        #[arg(long = "frontend-trace")]
        frontend_trace: bool,

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

        /// Build target: native, wasm, bytecode, or a reference native triple.
        #[arg(long)]
        target: Option<String>,

        /// Write schema-version-1 compile phase timings to PATH.
        #[arg(long = "timings-json")]
        timings_json: Option<String>,

        /// Emit native debug metadata and pass -g to clang object compilation.
        #[arg(short = 'g', long = "debug-info")]
        debug_info: bool,
    },

    /// Run a Sengoo source file.
    Run {
        /// Input source file.
        input: String,

        /// Optimization level (0-3).
        #[arg(short = 'O', long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=3))]
        opt_level: u8,

        /// Runtime contract checks (`auto` enables in O0/O1).
        #[arg(long = "contract-checks", value_enum, default_value_t = ContractChecksMode::Auto)]
        contract_checks: ContractChecksMode,

        /// Runtime engine policy.
        #[arg(long, value_enum, default_value_t = RunEngine::Auto)]
        engine: RunEngine,

        /// Execute the supported primitive numeric subset with Cranelift JIT.
        #[arg(long = "cranelift-fast-jit", conflicts_with = "daemon")]
        cranelift_fast_jit: bool,

        /// Ignore cached run artifacts and rebuild.
        #[arg(long)]
        force_rebuild: bool,

        /// Enable manual low-memory pipeline (trades incremental features for lower RSS).
        #[arg(long = "low-memory")]
        low_memory: bool,

        /// Frontend scheduler workers (`auto` by default, `1` for serial deterministic mode).
        #[arg(long = "frontend-jobs", default_value = "auto", value_parser = parse_frontend_jobs_arg)]
        frontend_jobs: FrontendJobs,

        /// Emit deterministic frontend planner trace lines.
        #[arg(long = "frontend-trace")]
        frontend_trace: bool,

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

        /// Emit native debug metadata and pass -g to clang object compilation.
        #[arg(short = 'g', long = "debug-info")]
        debug_info: bool,

        /// Execution target: native or bytecode.
        #[arg(long)]
        target: Option<String>,

        /// Arguments passed to program (reserved).
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Type-check/compile without generating final output.
    Check {
        /// Input source file.
        input: String,
    },

    /// Discover and run `tests/**/*.sg` in a package directory.
    Test {
        /// Package directory (defaults to current directory).
        #[arg(value_name = "PATH")]
        path: Option<String>,

        /// Keep only tests whose name/path contains TEXT.
        #[arg(long)]
        filter: Option<String>,

        /// Run only the test whose name exactly matches NAME.
        #[arg(long)]
        exact: Option<String>,

        /// Output format.
        #[arg(long, value_enum, default_value_t = TestOutputFormat::Text)]
        format: TestOutputFormat,

        /// Forward child stdout/stderr instead of capturing them.
        #[arg(long)]
        nocapture: bool,

        /// Use release optimization for test runs.
        #[arg(long)]
        release: bool,

        /// Emit a line-coverage summary for discovered test source files.
        #[arg(long)]
        coverage: bool,

        /// Manifest path used to locate the package root.
        #[arg(long = "manifest-path")]
        manifest_path: Option<String>,

        /// Reserved for lockfile-aware package runs.
        #[arg(long)]
        locked: bool,
    },

    /// Generate rustdoc-like API documentation.
    Doc {
        /// Input source file.
        input: String,

        /// Documentation output directory.
        #[arg(short, long, default_value = "target/doc")]
        output: String,
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
pub(crate) enum BenchCommands {
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

pub(crate) async fn run() -> Result<()> {
    let cli = Cli::parse();
    set_error_format(cli.error_format);
    dispatch(cli.command).await
}

async fn dispatch(command: Commands) -> Result<()> {
    let result = match command {
        Commands::Build {
            input,
            output,
            opt_level,
            contract_checks,
            emit_llvm,
            force_rebuild,
            low_memory,
            frontend_jobs,
            frontend_trace,
            daemon,
            daemon_addr,
            reflect,
            reflect_module,
            reflect_symbol,
            target,
            timings_json,
            debug_info,
        } => {
            if matches!(target.as_deref(), Some("wasm" | "bytecode")) {
                if daemon {
                    miette::bail!("portable targets do not support daemon dispatch");
                }
                if emit_llvm {
                    miette::bail!("--emit-llvm cannot be combined with a portable target");
                }
                if debug_info {
                    miette::bail!("portable targets do not support native debug metadata");
                }
                return match target.as_deref() {
                    Some("wasm") => build_wasm(&input, output.as_deref(), opt_level).map(|_| ()),
                    Some("bytecode") => {
                        build_bytecode(&input, output.as_deref(), opt_level).map(|_| ())
                    }
                    _ => unreachable!(),
                };
            }
            let native_target = match target.as_deref() {
                Some("native") => None,
                other => other,
            };
            if daemon {
                let addr = resolve_daemon_addr(daemon_addr.as_deref());
                let outcome = dispatch_build_via_daemon(
                    &addr,
                    &input,
                    output.as_deref(),
                    opt_level,
                    contract_checks,
                    emit_llvm,
                    force_rebuild,
                    low_memory,
                    frontend_jobs,
                    frontend_trace,
                    reflect,
                    &reflect_module,
                    &reflect_symbol,
                    debug_info,
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
                contract_checks,
                emit_llvm,
                force_rebuild,
                low_memory,
                frontend_jobs,
                frontend_trace_enabled(frontend_trace),
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
                native_target,
                timings_json.as_deref(),
                debug_info,
            )
            .await
        }
        Commands::Run {
            input,
            opt_level,
            contract_checks,
            engine,
            cranelift_fast_jit,
            force_rebuild,
            low_memory,
            frontend_jobs,
            frontend_trace,
            daemon,
            daemon_addr,
            reflect,
            reflect_module,
            reflect_symbol,
            debug_info,
            target,
            args,
        } => {
            if let Some(target) = target.as_deref() {
                match target {
                    "bytecode" => {
                        if daemon {
                            miette::bail!("bytecode execution does not support daemon dispatch");
                        }
                        if !args.is_empty() {
                            miette::bail!(
                                "bytecode execution does not support program arguments yet"
                            );
                        }
                        if debug_info {
                            miette::bail!(
                                "bytecode execution does not support native debug metadata"
                            );
                        }
                        let exit_code = i32::try_from(run_bytecode(&input, opt_level)?)
                            .map_err(|_| miette::miette!("bytecode main result is not an i32"))?;
                        return propagate_run_exit_code(exit_code);
                    }
                    "native" => {}
                    "wasm" => {
                        if daemon {
                            miette::bail!("wasm execution does not support daemon dispatch");
                        }
                        if !args.is_empty() {
                            miette::bail!("wasm execution does not support program arguments yet");
                        }
                        if debug_info {
                            miette::bail!("wasm execution does not support native debug metadata");
                        }
                        let exit_code = i32::try_from(run_wasm(&input, opt_level)?)
                            .map_err(|_| miette::miette!("wasm main result is not an i32"))?;
                        return propagate_run_exit_code(exit_code);
                    }
                    other => {
                        miette::bail!(
                            "unsupported run target `{other}`; expected `native`, `bytecode`, or `wasm`"
                        );
                    }
                }
            }
            if cranelift_fast_jit {
                if !args.is_empty() {
                    return Err(miette::miette!(
                        "cranelift fast-jit does not support program arguments"
                    ));
                }
                let source = std::fs::read_to_string(&input).map_err(|error| {
                    miette::miette!("failed to read Cranelift input `{input}`: {error}")
                })?;
                let value =
                    crate::cranelift_fast_jit::run_with_cranelift_fast_jit(&source, opt_level)?;
                println!("{value}");
                return Ok(());
            }
            if daemon {
                let addr = resolve_daemon_addr(daemon_addr.as_deref());
                let outcome = dispatch_run_via_daemon(
                    &addr,
                    &input,
                    opt_level,
                    contract_checks,
                    engine,
                    force_rebuild,
                    &args,
                    low_memory,
                    frontend_jobs,
                    frontend_trace,
                    reflect,
                    &reflect_module,
                    &reflect_symbol,
                    debug_info,
                )
                .await?;
                if matches!(outcome, DaemonDispatchOutcome::Handled) {
                    return Ok(());
                }
            }
            cmd_run(
                &input,
                opt_level,
                contract_checks,
                engine,
                force_rebuild,
                &args,
                low_memory,
                frontend_jobs,
                frontend_trace_enabled(frontend_trace),
                reflection_options_from_cli(reflect, &reflect_module, &reflect_symbol),
                debug_info,
            )
            .await
        }
        Commands::Check { input } => cmd_check(&input).await,
        Commands::Test {
            path,
            filter,
            exact,
            format,
            nocapture,
            release,
            coverage,
            manifest_path,
            locked,
        } => {
            let manifest = manifest_path.as_deref().map(Path::new);
            let root = resolve_test_root(path.as_deref().map(Path::new), manifest)?;
            cmd_test(TestOptions {
                root: &root,
                filter: filter.as_deref(),
                exact: exact.as_deref(),
                format,
                nocapture,
                release,
                coverage,
                locked,
                manifest_path: manifest,
            })
        }
        Commands::Doc { input, output } => cmd_doc(&input, &output).await,
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
    };

    if let Err(err) = result {
        if current_error_format() == ErrorFormat::Json && err.to_string() == "compile failed" {
            std::process::exit(1);
        }
        return Err(err);
    }

    Ok(())
}
