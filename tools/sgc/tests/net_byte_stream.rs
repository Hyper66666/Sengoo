//! Native source-level proof for caller-nonblocking raw network workers.
mod common;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn sengoo_stream_reads_raw_fragments_commits_buffer_and_cancels_without_async_frames() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-net-byte-stream-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let mut children = Vec::new();
        for index in 0..4 {
            let (mut socket, _) = listener.accept().unwrap();
            children.push(thread::spawn(move || {
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = [0u8; 4096];
                let count = socket.read(&mut request).unwrap();
                assert!(String::from_utf8_lossy(&request[..count])
                    .contains("Authorization: Bearer fixture"));
                for fragment in [
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n".as_slice(),
                    b"hel",
                    b"lo",
                ] {
                    if socket.write_all(fragment).is_err() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                if index == 0 {
                    thread::sleep(Duration::from_millis(50));
                }
            }));
        }
        for child in children {
            child.join().unwrap();
        }
    });
    let source = format!(
        r##"
import std::net;
import std::ffi;
import std::time;
import std::string;

def drain(stream: &NetByteStream) -> Result<i64,i64> {{
    let buffer = ffi_buffer_new(7)?;
    let mut count = 0;
    let until = time_unix_ms() + 5000;
    while time_unix_ms() < until {{
        let event = stream.read_into(buffer);
        if event.error != 0 {{ return result_err_i64(event.error); }};
        if event.eof {{ return result_ok_i64(count); }};
        if event.ready {{
            if buffer.used_len() != event.count {{ return result_err_i64(101); }};
            let copied = string_from_buffer(buffer, event.count)?;
            if copied.len() != event.count {{ return result_err_i64(102); }};
            count = count + event.count;
        }};
        time_sleep_ms(2);
    }};
    result_err_i64(103)
}}

def verify() -> Result<i64,i64> {{
    // A hash inside ordinary application data must not make attribute scanning
    // strip async modifiers from std::net's unrelated public helpers.
    let attribute_scan_trigger = "#";
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture\r\n\r\n";
    let first = net_stream_start("127.0.0.1", {port}, false, request, 2000, 2000, 4096)?;
    let second = net_stream_start("127.0.0.1", {port}, false, request, 2000, 2000, 4096)?;
    let third = net_stream_start("127.0.0.1", {port}, false, request, 2000, 2000, 4096)?;
    let fourth = net_stream_start("127.0.0.1", {port}, false, request, 2000, 2000, 4096)?;
    let expected = 43;
    if drain(&second)? != expected || drain(&third)? != expected || drain(&first)? != expected || drain(&fourth)? != expected {{ return result_err_i64(104); }};
    if !first.request_started()? || !second.request_started()? {{ return result_err_i64(106); }};
    first.cancel();
    let buffer = ffi_buffer_new(8)?;
    let canceled = first.read_into(buffer);
    if canceled.error != 19 || canceled.eof || !canceled.ready {{ return result_err_i64(105); }};
    if !first.request_started()? {{ return result_err_i64(107); }};
    first.close();
    let closed = first.request_started();
    if closed.is_ok || closed.error != 3 {{ return result_err_i64(108); }};
    result_ok_i64(0)
}}
def main() -> i64 {{
    let result = verify();
    if !result.is_ok {{ println(result.error); return 1; }};
    let until = time_unix_ms() + 3000;
    while sengoo_net_stream_live_workers() != 0 && time_unix_ms() < until {{ time_sleep_ms(2); }};
    if sengoo_net_stream_live_workers() == 0 && sengoo_net_stream_live_handles() == 0 {{ 0 }} else {{ 2 }}
}}
"##
    );
    let path = root.join("main.sg");
    fs::write(&path, source).unwrap();
    let output = common::source_sgc_command()
        .current_dir(&root)
        .arg("run")
        .arg(&path)
        .args(["--force-rebuild", "-O1"])
        .output()
        .unwrap();
    fs::write(root.join("stdout.txt"), &output.stdout).unwrap();
    fs::write(root.join("stderr.txt"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "Sengoo byte stream failed; retained {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
