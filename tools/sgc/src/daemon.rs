use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::{
    cmd_build, cmd_run, frontend_jobs_label, frontend_trace_enabled, parse_frontend_jobs_arg,
    reflection_options_from_cli, FrontendJobs, ReflectionMode, RunEngine, DAEMON_CONNECT_TIMEOUT,
    DAEMON_PROTOCOL_VERSION, DEFAULT_DAEMON_ADDR,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DaemonRequest {
    pub(crate) protocol_version: u32,
    pub(crate) client_version: String,
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
        #[serde(default)]
        low_memory: bool,
        #[serde(default = "default_frontend_jobs_wire")]
        frontend_jobs: String,
        #[serde(default)]
        frontend_trace: bool,
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
        #[serde(default)]
        low_memory: bool,
        #[serde(default = "default_frontend_jobs_wire")]
        frontend_jobs: String,
        #[serde(default)]
        frontend_trace: bool,
        reflect: ReflectionMode,
        reflect_module: Vec<String>,
        reflect_symbol: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DaemonResponse {
    pub(crate) protocol_version: u32,
    pub(crate) server_version: String,
    pub(crate) ok: bool,
    pub(crate) recoverable: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DaemonDispatchOutcome {
    Handled,
    Fallback,
}

pub(crate) fn resolve_daemon_addr(explicit: Option<&str>) -> String {
    explicit
        .map(str::to_string)
        .or_else(|| std::env::var("SENGOO_DAEMON_ADDR").ok())
        .unwrap_or_else(|| DEFAULT_DAEMON_ADDR.to_string())
}

fn parse_frontend_jobs_wire(raw: &str) -> FrontendJobs {
    parse_frontend_jobs_arg(raw).unwrap_or(FrontendJobs::Auto)
}

fn default_frontend_jobs_wire() -> String {
    "auto".to_string()
}

pub(super) fn daemon_request_build(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
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
            low_memory,
            frontend_jobs: frontend_jobs_label(frontend_jobs),
            frontend_trace,
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
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
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
            low_memory,
            frontend_jobs: frontend_jobs_label(frontend_jobs),
            frontend_trace,
            reflect,
            reflect_module: reflect_module.to_vec(),
            reflect_symbol: reflect_symbol.to_vec(),
        },
    }
}

pub(crate) async fn dispatch_build_via_daemon(
    addr: &str,
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
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
        low_memory,
        frontend_jobs,
        frontend_trace_enabled(frontend_trace),
        reflect,
        reflect_module,
        reflect_symbol,
    );
    dispatch_daemon_request(addr, &request, "build").await
}

pub(crate) async fn dispatch_run_via_daemon(
    addr: &str,
    input: &str,
    opt_level: u8,
    engine: RunEngine,
    force_rebuild: bool,
    args: &[String],
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
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
        low_memory,
        frontend_jobs,
        frontend_trace_enabled(frontend_trace),
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

pub(super) async fn send_daemon_request(
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

    let mut reader = TokioBufReader::new(read_half);
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

pub(crate) async fn cmd_daemon(addr: &str) -> Result<()> {
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

pub(super) async fn handle_daemon_client(stream: TcpStream) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = TokioBufReader::new(read_half);
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
            low_memory,
            frontend_jobs,
            frontend_trace,
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
                low_memory,
                parse_frontend_jobs_wire(&frontend_jobs),
                frontend_trace_enabled(frontend_trace),
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
            low_memory,
            frontend_jobs,
            frontend_trace,
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
                low_memory,
                parse_frontend_jobs_wire(&frontend_jobs),
                frontend_trace_enabled(frontend_trace),
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
