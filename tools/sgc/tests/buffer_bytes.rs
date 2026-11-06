//! Raw byte queues must tolerate split UTF-8 and embedded NUL before decoding.
mod common;

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn initialized_byte_access_search_consume_and_aliases_are_checked() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-buffer-bytes-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let source = r#"
import std::ffi;
import std::string;
extern "C" { fn sengoo_buffer_live_handle_count() -> i64; }

struct BufferReader {}
impl BufferReader {
    def read(&self, out: &mut Buffer) -> Result<i64, i64> {
        out.clear();
        out.append_str("xy")
    }
}

def verify() -> Result<i64,i64> {
    let bytes = ffi_buffer_new(32)?;
    bytes.push_u8(0)?; bytes.push_u8(255)?;
    bytes.append_str("a\r\nb")?;
    let delimiter = ffi_buffer_from_bytes("\r\n")?;
    let empty = ffi_buffer_new(0)?;
    if bytes.get_u8(0)? != 0 || bytes.get_u8(1)? != 255 { return result_err_i64(100); };
    if bytes.index_of_bytes(delimiter, 0)? != 3 || bytes.index_of_bytes(delimiter, 4)? != -1 { return result_err_i64(101); };
    if bytes.index_of_bytes(empty, bytes.used_len())? != bytes.used_len() { return result_err_i64(102); };
    if bytes.index_of_bytes(bytes, 0)? != 0 { return result_err_i64(103); };
    let missing = bytes.get_u8(bytes.used_len());
    let negative = bytes.get_u8(-1);
    let invalid_start = bytes.index_of_bytes(delimiter, 33);
    let invalid_consume = bytes.consume(33);
    let negative_consume = bytes.consume(-1);
    if missing.is_ok || missing.error != 2 || negative.is_ok || invalid_start.is_ok || invalid_consume.is_ok || negative_consume.is_ok || bytes.used_len() != 6 { return result_err_i64(104); };
    if bytes.consume(2)? != 4 || bytes.get_u8(0)? != 97 || bytes.index_of_bytes(delimiter, 0)? != 1 { return result_err_i64(105); };
    if bytes.consume(0)? != 4 || bytes.consume(4)? != 0 || bytes.capacity() != 32 { return result_err_i64(106); };
    if bytes.index_of_bytes(delimiter, 0)? != -1 || bytes.index_of_bytes(empty, 0)? != 0 { return result_err_i64(107); };
    bytes.append_str("prefix-")?;
    let encoded = ffi_buffer_from_bytes("世界😀")?;
    let mut cursor = 0;
    while cursor < encoded.used_len() {
        bytes.push_u8(encoded.get_u8(cursor)?)?;
        cursor = cursor + 1;
    };
    bytes.consume(7)?;
    let complete = string_from_bytes_raw(bytes.ptr(), bytes.used_len())?;
    if complete.as_str() != "世界😀" { return result_err_i64(108); };
    let nul_needle = ffi_buffer_new(2)?; nul_needle.push_u8(0)?; nul_needle.push_u8(255)?;
    bytes.clear(); bytes.append_from(nul_needle, 0, 2)?;
    if bytes.index_of_bytes(nul_needle, 0)? != 0 { return result_err_i64(109); };
    let outside_source = bytes.append_from(nul_needle, 1, 2);
    if outside_source.is_ok || outside_source.error != 2 || bytes.used_len() != 2 { return result_err_i64(110); };
    bytes.clear(); bytes.append_str("abcd")?;
    bytes.append_from(bytes, 1, 3)?;
    let self_append = string_from_bytes_raw(bytes.ptr(), bytes.used_len())?;
    if self_append.as_str() != "abcdbcd" { return result_err_i64(111); };
    bytes.copy_range(2, 4, bytes)?;
    let self_copy = string_from_bytes_raw(bytes.ptr(), bytes.used_len())?;
    if self_copy.as_str() != "cdbc" { return result_err_i64(112); };
    let stale = ffi_buffer_from_bytes("gone")?; stale.free();
    let stale_read = stale.get_u8(0);
    let stale_search = bytes.index_of_bytes(stale, 0);
    let stale_consume = stale.consume(0);
    if stale_read.is_ok || stale_read.error != 3 || stale_search.is_ok || stale_search.error != 3 || stale_consume.is_ok || stale_consume.error != 3 { return result_err_i64(113); };
    let live_before = sengoo_buffer_live_handle_count();
    let repeated = ffi_buffer_from_bytes("abc")?;
    let destination = ffi_buffer_new(12)?;
    let mut cycle = 0;
    while cycle < 50 {
        destination.clear();
        if cycle % 2 == 0 { destination.append_copy(&repeated, 0, 3)?; (); };
        destination.append_copy(&repeated, 1, 2)?;
        if repeated.used_len() != 3 || repeated.get_u8(2)? != 99 || sengoo_buffer_live_handle_count() != live_before + 2 { return result_err_i64(114); };
        cycle = cycle + 1;
    };
    let outside_borrowed = destination.append_copy(&repeated, 3, 1);
    if outside_borrowed.is_ok || outside_borrowed.error != 2 { return result_err_i64(115); };
    let reader = BufferReader {};
    let mut read_buffer = ffi_buffer_new(8)?;
    let read_handles = sengoo_buffer_live_handle_count();
    cycle = 0;
    while cycle < 50 {
        let read = reader.read(&mut read_buffer)?;
        if read > 0 { destination.clear(); destination.append_copy(&read_buffer, 0, read)?; (); };
        if read_buffer.used_len() != 2 || destination.get_u8(0)? != 120 || sengoo_buffer_live_handle_count() != read_handles { return result_err_i64(116); };
        cycle = cycle + 1;
    };
    result_ok_i64(0)
}

def main() -> i64 {
    let checked = verify();
    if not checked.is_ok { println(checked.error); return 1; };
    checked.value
}
"#;
    let path = root.join("main.sg");
    fs::write(&path, source).unwrap();
    let output = common::source_sgc_command()
        .current_dir(&root)
        .arg("run")
        .arg(&path)
        .args(["--force-rebuild", "-O1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "buffer byte queue failed; artifacts retained {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
