mod common;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::source_sgc_command;

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sengoo_http_request_strings_{name}_{}_{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("child should be live")
    }

    fn wait_with_deadline(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child_mut()
                .try_wait()
                .expect("query child process status")
            {
                self.0.take();
                return status;
            }
            if Instant::now() >= deadline {
                let child = self.child_mut();
                let _ = child.kill();
                let _ = child.wait();
                self.0.take();
                panic!("Sengoo HTTP accessor fixture exceeded its watchdog");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn build_fixture(source: &Path, executable: &Path) {
    let output = source_sgc_command()
        .arg("build")
        .arg(source)
        .arg("--output")
        .arg(executable)
        .args(["-O", "0", "--force-rebuild"])
        .output()
        .expect("run source sgc");
    assert!(
        output.status.success(),
        "fixture build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_sgc_http_request_owned_string_accessors_are_safe() {
    let dir = TestDir::new("owned_accessors");
    let source = dir.0.join("main.sg");
    let executable = dir.0.join(if cfg!(windows) {
        "server.exe"
    } else {
        "server"
    });
    fs::write(
        &source,
        r#"import std::ffi;

import std::io;

import std::net;

import std::strconv;

import std::string;

async def main() -> i64 {
    let bound = http_server_bind("127.0.0.1", 0);
    if bound.is_err() { return 10; };
    let server = bound.value;
    let port = server.local_port();
    if port.is_err() { server.close(); return 11; };
    let port_buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let port_len = strconv_format_i64(port.value, port_buffer).unwrap_or(0);
    io_stdout_write("READY 127.0.0.1:");
    io_stdout_write_raw(port_buffer.ptr(), port_len);
    io_stdout_write("\n");
    io_stdout_flush();
    port_buffer.free();

    let outcome = await server.next_request_async(5000);
    if not outcome.is_ok { server.close(); return 12; };
    let request = outcome.value;
    let method = request.method_string();
    let path = request.path_string();
    let query = request.query_string();
    let version = request.version_string();
    let trace = request.header_string("X-Trace");
    let body_buffer = ffi_buffer_new(4).unwrap_or(Buffer { handle: 0 });
    let copied = request.body_copy(body_buffer);
    let body_matches = if copied.is_err() { false; } else { copied.value == 4 and body_buffer.used_len() == 4; };
    let matches = method.is_ok() and str_eq(method.value.as_str(), "POST") and path.is_ok() and str_eq(path.value.as_str(), "/probe") and query.is_ok() and str_eq(query.value.as_str(), "mode=owned") and version.is_ok() and str_eq(version.value.as_str(), "HTTP/1.1") and trace.is_ok() and str_eq(trace.value.as_str(), "abc") and body_matches;
    let responded = if matches { request.respond(200, "ok").unwrap_or(false); } else { request.respond(500, "mismatch").unwrap_or(false); };
    body_buffer.free();
    let closed = server.close();
    if responded and closed { 0; } else { 13; };
}
"#,
    )
    .expect("write fixture source");
    build_fixture(&source, &executable);

    let mut command = Command::new(&executable);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = ChildGuard::new(command.spawn().expect("spawn fixture"));
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .expect("capture fixture stdout");
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .expect("capture fixture stderr");
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let stdout_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut ready = String::new();
        let result = reader.read_line(&mut ready);
        let _ = ready_tx.send((ready, result));
        let mut extra = Vec::new();
        let _ = reader.read_to_end(&mut extra);
        extra
    });
    let stderr_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes);
        bytes
    });

    let (ready, ready_result) = ready_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("fixture should publish READY before deadline");
    ready_result.expect("read READY line");
    let port = ready
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("READY 127.0.0.1:")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or_else(|| panic!("invalid READY line: {ready:?}"));

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to fixture");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("set write timeout");
    stream
        .write_all(
            b"POST /probe?mode=owned HTTP/1.1\r\nHost: localhost\r\nX-Trace: abc\r\nContent-Length: 4\r\nConnection: close\r\n\r\nping",
        )
        .expect("write fixture request");
    stream.flush().expect("flush fixture request");
    let mut response = Vec::new();
    let read_result = stream.read_to_end(&mut response);
    let status = child.wait_with_deadline(Duration::from_secs(10));
    let extra_stdout = stdout_reader.join().expect("join stdout reader");
    let stderr = stderr_reader.join().expect("join stderr reader");
    let stderr = String::from_utf8_lossy(&stderr);

    assert!(
        read_result.is_ok(),
        "response read failed: {:?}; child={status:?}; stderr={stderr}",
        read_result.err()
    );
    assert!(status.success(), "child={status:?}; stderr={stderr}");
    assert!(
        extra_stdout.is_empty(),
        "unexpected stdout: {extra_stdout:?}"
    );
    let response = String::from_utf8_lossy(&response);
    assert!(
        response.starts_with("HTTP/1.1 200 OK\r\n") && response.ends_with("\r\n\r\nok"),
        "unexpected response: {response:?}; stderr={stderr}"
    );
}

#[test]
fn real_sgc_tls_router_keep_alive_streaming_composes_with_verified_ca() {
    let curl = if cfg!(windows) { "curl.exe" } else { "curl" };
    if Command::new(curl).arg("--version").output().is_err() {
        eprintln!("skip TLS composition test: curl is unavailable");
        return;
    }

    let dir = TestDir::new("tls_router_stream");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc manifest should live below the workspace");
    fs::copy(
        workspace.join("runtime/src/net/testdata/http_server_tls_cert.pem"),
        dir.0.join("cert.pem"),
    )
    .expect("copy test certificate");
    fs::copy(
        workspace.join("runtime/src/net/testdata/http_server_tls_key.pem"),
        dir.0.join("key.pem"),
    )
    .expect("copy test key");

    let source = dir.0.join("main.sg");
    let executable = dir.0.join(if cfg!(windows) {
        "tls-server.exe"
    } else {
        "tls-server"
    });
    fs::write(
        &source,
        r#"import std::ffi;

import std::file;

import std::io;

import std::net;

import std::strconv;

def stream_handler(request: &mut HttpServerRequest) -> Result<bool, i64> {
    let started = request.begin_stream(200);
    if started.is_err() {
        Result { is_ok: false, value: false, error: started.error }
    } else {
        let mut stream = started.value;
        let first = stream.write_raw(sengoo_stdlib_str_ptr("tls-"), 4);
        if first.is_err() {
            first
        } else {
            let second = stream.write_raw(sengoo_stdlib_str_ptr("stream"), 6);
            if second.is_err() {
                second
            } else {
                stream.finish()
            }
        }
    }
}

def done_handler(request: &mut HttpServerRequest) -> Result<bool, i64> {
    request.respond(200, "done")
}

async def main() -> i64 {
    let cert_len = file_len("cert.pem").unwrap_or(0);
    let key_len = file_len("key.pem").unwrap_or(0);
    if cert_len <= 0 or key_len <= 0 { return 10; };
    let cert = ffi_buffer_new(cert_len).unwrap_or(Buffer { handle: 0 });
    let key = ffi_buffer_new(key_len).unwrap_or(Buffer { handle: 0 });
    let cert_read = file_read_into("cert.pem", cert).unwrap_or(0);
    let key_read = file_read_into("key.pem", key).unwrap_or(0);
    let bound = http_server_bind_tls("127.0.0.1", 0, cert.ptr(), cert_read, key.ptr(), key_read);
    cert.free();
    key.free();
    if bound.is_err() { return 11; };
    let server = bound.value;
    if server.set_keep_alive(1).is_err() { server.close(); return 12; };

    let stream_fn: fn(&mut HttpServerRequest) -> Result<bool, i64> = stream_handler;
    let done_fn: fn(&mut HttpServerRequest) -> Result<bool, i64> = done_handler;
    let created = http_router_new();
    if created.is_err() { server.close(); return 13; };
    let mut router = created.value;
    let first_route = http_router_route(&mut router, "GET", "/stream", stream_fn);
    if first_route.is_err() { server.close(); return 13; };
    let second_route = http_router_route(&mut router, "GET", "/done", done_fn);
    if second_route.is_err() { server.close(); return 14; };

    let port = server.local_port().unwrap_or(0);
    let port_buffer = ffi_buffer_new(16).unwrap_or(Buffer { handle: 0 });
    let port_len = strconv_format_i64(port, port_buffer).unwrap_or(0);
    io_stdout_write("READY 127.0.0.1:");
    io_stdout_write_raw(port_buffer.ptr(), port_len);
    io_stdout_write("\n");
    io_stdout_flush();
    port_buffer.free();

    let first = await serve_http_timeout(&server, &router, 10000);
    if first.is_err() { server.close(); return 15; };
    let second = await serve_http_timeout(&server, &router, 10000);
    let closed = server.close();
    if second.is_err() { 16; } else if !closed { 17; } else { 0; };
}
"#,
    )
    .expect("write TLS composition fixture");
    build_fixture(&source, &executable);

    let mut command = Command::new(&executable);
    command
        .current_dir(&dir.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = ChildGuard::new(command.spawn().expect("spawn TLS fixture"));
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .expect("capture fixture stdout");
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .expect("capture fixture stderr");
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let stdout_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut ready = String::new();
        let result = reader.read_line(&mut ready);
        let _ = ready_tx.send((ready, result));
        let mut extra = Vec::new();
        let _ = reader.read_to_end(&mut extra);
        extra
    });
    let stderr_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes);
        bytes
    });

    let (ready, ready_result) = ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("TLS fixture should publish READY before deadline");
    ready_result.expect("read TLS READY line");
    let port = ready
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("READY 127.0.0.1:")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or_else(|| panic!("invalid TLS READY line: {ready:?}"));

    let resolve = format!("localhost:{port}:127.0.0.1");
    let first_url = format!("https://localhost:{port}/stream");
    let second_url = format!("https://localhost:{port}/done");
    let output = Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--http1.1",
            "--noproxy",
            "*",
            "--cacert",
            "cert.pem",
            "--resolve",
            &resolve,
            "--include",
            &first_url,
            &second_url,
        ])
        .current_dir(&dir.0)
        .output()
        .expect("run verified TLS client");
    let status = child.wait_with_deadline(Duration::from_secs(15));
    let extra_stdout = stdout_reader.join().expect("join TLS stdout reader");
    let child_stderr = stderr_reader.join().expect("join TLS stderr reader");
    let child_stderr = String::from_utf8_lossy(&child_stderr);
    let curl_stderr = String::from_utf8_lossy(&output.stderr);
    let response = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "curl failed: {curl_stderr}; child={status:?}; response={response:?}; stderr={child_stderr}"
    );
    assert!(
        status.success(),
        "TLS fixture failed: {status:?}; response={response:?}; curl_stderr={curl_stderr}; stderr={child_stderr}"
    );
    assert!(
        extra_stdout.is_empty(),
        "unexpected stdout: {extra_stdout:?}"
    );
    assert_eq!(
        response.matches("HTTP/1.1 200 OK").count(),
        2,
        "both routed responses should succeed: {response:?}"
    );
    assert!(
        response
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
            && response.contains("tls-stream")
            && response.contains("done"),
        "verified TLS router/stream response is incomplete: {response:?}"
    );
}
