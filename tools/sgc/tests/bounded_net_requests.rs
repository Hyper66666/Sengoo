//! Real large request through the public Sengoo API. No live service is used.
mod common;
use std::{fs, io::{Read, Write}, net::TcpListener, thread, time::{Duration, SystemTime, UNIX_EPOCH}};

#[test]
fn public_bounded_request_keeps_legacy_limit_and_copies_large_input() {
    let root = std::env::temp_dir().join(format!("sengoo-bounded-request-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&root).unwrap();
    let input = vec![b'x'; 1024 * 1024 + 32768];
    fs::write(root.join("request.bin"), &input).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        // Bounded even when compilation or validation fails before connect.
        let deadline = std::time::Instant::now() + Duration::from_secs(100);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(std::time::Instant::now() < deadline, "no Sengoo client connected");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut received = vec![0; input.len()];
        socket.read_exact(&mut received).unwrap();
        assert_eq!(received, input);
        socket.write_all(b"ok").unwrap();
    });
    let source = format!(r#"
import std::net;
import std::file;
import std::string;
import std::time;

def verify() -> Result<i64, i64> {{
    let bytes = ffi_buffer_new(1100000)?;
    let count = file_read_into("request.bin", bytes)?;
    let request = string_from_buffer(bytes, count)?;
    let legacy = net_stream_start("127.0.0.1", {port}, false, request.as_str(), 2000, 2000, 4096);
    if legacy.is_ok or legacy.error != 2 {{ return result_err_i64(101); }};
    let invalid = net_stream_start_bounded("127.0.0.1", {port}, false, request.as_str(), 2000, 2000, 4096, 0);
    if invalid.is_ok or invalid.error != 2 {{ return result_err_i64(102); }};
    let stream = net_stream_start_bounded("127.0.0.1", {port}, false, request.as_str(), 2000, 2000, 4096, 16777216)?;
    let output = ffi_buffer_new(16)?; let mut total = 0; let until = time_unix_ms() + 5000;
    while time_unix_ms() < until {{
        let part = stream.read_into(output);
        if part.error != 0 {{ return result_err_i64(part.error); }};
        total = total + part.count;
        if part.eof {{ if total != 2 or not stream.request_started()? {{ return result_err_i64(103); }}; return result_ok_i64(0); }};
        time_sleep_ms(1);
    }};
    result_err_i64(104)
}}
def main() -> i64 {{
    let result = verify(); if not result.is_ok {{ println(result.error); return 1; }};
    let until = time_unix_ms() + 3000;
    while sengoo_net_stream_live_workers() != 0 and time_unix_ms() < until {{ time_sleep_ms(1); }};
    if sengoo_net_stream_live_handles() != 0 or sengoo_net_stream_live_request_bytes() != 0 {{ return 2; }};
    0
}}
"#);
    let path = root.join("main.sg");
    fs::write(&path, source).unwrap();
    let output = common::source_sgc_command().current_dir(&root).arg("run").arg(&path)
        .args(["--force-rebuild", "-O1"]).output().unwrap();
    fs::write(root.join("stdout.txt"), &output.stdout).unwrap();
    fs::write(root.join("stderr.txt"), &output.stderr).unwrap();
    assert!(output.status.success(), "retained {}\n{}\n{}", root.display(), String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    server.join().unwrap();
    for name in ["request.bin", "main.sg", "stdout.txt", "stderr.txt", "main.ll", "main.exe", "main.exe.sgc-cache.json"] {
        let file = root.join(name);
        if file.is_file() { fs::remove_file(file).unwrap(); }
    }
    // Compiler caches may remain; never recursively delete a computed tree.
    if root.read_dir().unwrap().next().is_none() { fs::remove_dir(root).unwrap(); }
}
