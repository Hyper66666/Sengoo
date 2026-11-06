//! Exercise the actual C JSON runtime through compiled Sengoo, including
//! binary input which cannot be represented as a valid source string.
mod common;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn json_unicode_scalars_surrogates_nul_and_invalid_bytes_round_trip() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-json-unicode-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let mut source = String::from("import std::json;\nimport std::file;\nimport std::string;\nimport std::ffi;\n\ndef verify() -> Result<i64,i64> {\n");
    let valid = [
        (r#""\u4e16\u754c\ud83d\ude00\u00e9""#, "世界😀é"),
        (
            r#""\u0001\u001f\u007f\u0080\u07ff\u0800\uffff\ud800\udc00\udbff\udfff""#,
            "\u{1}\u{1f}\u{7f}\u{80}\u{7ff}\u{800}\u{ffff}\u{10000}\u{10ffff}",
        ),
        ("\"中文 emoji 🧪\"", "中文 emoji 🧪"),
        (r#""\"\\\/\b\f\n\r\t""#, "\"\\/\x08\x0c\n\r\t"),
    ];
    for (index, (json, expected)) in valid.iter().enumerate() {
        fs::write(root.join(format!("valid-{index}.json")), json).unwrap();
        // Compare serialized JSON using two runtime parses rather than source
        // escape syntax (the expected file contains the exact decoded UTF-8).
        fs::write(
            root.join(format!("expected-{index}.txt")),
            expected.as_bytes(),
        )
        .unwrap();
        source.push_str(&format!(
            r#"
    {{
        let input = ffi_buffer_new(4096)?;
        let size = file_read_into("valid-{index}.json", input)?;
        let doc = json_parse_buffer(input, size)?;
        let text = doc.root().string_value()?;
        let wanted = ffi_buffer_new(4096)?;
        let wanted_size = file_read_into("expected-{index}.txt", wanted)?;
        let wanted_text = string_from_bytes_raw(wanted.ptr(), wanted_size)?;
        if text != wanted_text {{ return result_err_i64(100 + {index}); }};
        let encoded = doc.serialize_string()?;
        let restored = json_parse(encoded.as_str())?;
        let restored_text = restored.root().string_value()?;
        if restored_text != wanted_text {{ return result_err_i64(110 + {index}); }};
    }};
"#
        ));
    }
    source.push_str(r#"
    {
        let doc = json_parse("{\"a\\u0000b\":\"x\\u0000y\",\"a\":1}")?;
        let encoded = doc.serialize_string()?;
        if !str_contains(encoded.as_str(), "x\\u0000y") || !str_contains(encoded.as_str(), "a\\u0000b") { return result_err_i64(120); };
        if doc.root().object_get("a")?.number_i64()? != 1 { return result_err_i64(121); };
        let scalar = json_parse("\"x\\u0000y\"")?;
        let text = scalar.root().string_value()?;
        if text.len() != 3 { return result_err_i64(122); };
    };
"#);
    let invalid: Vec<Vec<u8>> = vec![
        br#""\ud800""#.to_vec(),
        br#""\udc00""#.to_vec(),
        br#""\ud800\u0041""#.to_vec(),
        br#""\ud800\ud800""#.to_vec(),
        br#""\u12g4""#.to_vec(),
        br#""\u123""#.to_vec(),
        br#""\x20""#.to_vec(),
        br#""\U00000041""#.to_vec(),
        b"\"\xc0\x80\"".to_vec(),
        b"\"\xed\xa0\x80\"".to_vec(),
        b"\"\xf4\x90\x80\x80\"".to_vec(),
        b"\"\xe4\xb8\"".to_vec(),
        b"\"\x80\"".to_vec(),
        b"\"\x01\"".to_vec(),
    ];
    for (index, bytes) in invalid.iter().enumerate() {
        fs::write(root.join(format!("invalid-{index}.json")), bytes).unwrap();
        source.push_str(&format!(r#"
    {{ let bytes = ffi_buffer_new(64)?; let size = file_read_into("invalid-{index}.json", bytes)?;
       let rejected = json_parse_buffer(bytes, size);
       if rejected.is_ok || rejected.error != 10 || json_last_error_offset() < 0 {{ return result_err_i64(200 + {index}); }}; }};
"#));
    }
    // Recursive node allocations previously invalidated cached parent pointers.
    let wide = format!(
        "{{\"items\":[{}]}}",
        (0..300)
            .map(|i| format!("{{\"n\":{i},\"text\":\"\\u4e16\"}}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    fs::write(root.join("wide.json"), wide).unwrap();
    source.push_str(r#"
    let wide_bytes = ffi_buffer_new(32768)?;
    let wide_size = file_read_into("wide.json", wide_bytes)?;
    let wide = json_parse_buffer(wide_bytes, wide_size)?;
    let items = wide.root().object_get("items")?;
    if items.array_len()? != 300 || items.array_get(299)?.object_get("n")?.number_i64()? != 299 { return result_err_i64(300); };
    result_ok_i64(0)
}
def main() -> i64 { let result = verify(); if result.is_ok { 0 } else { println(result.error); 1 } }
"#);
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
        "JSON Unicode failed; artifacts retained {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn json_copy_is_independent_exact_and_transactional_on_limits() {
    let root = std::env::temp_dir().join(format!(
        "sengoo-json-copy-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let mut source = String::from(
        r#"
import std::json;
import std::string;

def verify() -> Result<i64,i64> {
    let destination = json_doc_object()?;
    destination.root().object_set("keep", destination.new_i64(7)?)?;
    let mut stale = JsonValue { doc_handle: 0, node_id: 0 };
    {
        let source = json_parse("{\"maximum\":9223372036854775807,\"minimum\":-9223372036854775808,\"text\":\"\\u4e16\\u754c\\ud83d\\ude00\",\"nul\":\"a\\u0000b\",\"a\\u0000b\":true,\"items\":[null,false,1.25]}")?;
        stale = source.root();
        let copied = destination.copy(source.root())?;
        destination.root().object_set("copied", copied)?;
    };
    let copied = destination.root().object_get("copied")?;
    if copied.object_get("maximum")?.number_i64()? != 9223372036854775807 { return result_err_i64(401); };
    if copied.object_get("minimum")?.number_i64()? != -9223372036854775808 { return result_err_i64(402); };
    if copied.object_get("text")?.string_value()?.as_str() != "世界😀" { return result_err_i64(403); };
    if copied.object_get("nul")?.string_value()?.len() != 3 || copied.object_has("a") { return result_err_i64(404); };
    if copied.object_get("items")?.array_len()? != 3 { return result_err_i64(405); };
    let same_document = destination.copy(copied)?;
    destination.root().object_set("again", same_document)?;
    let before = destination.serialize_string()?;
    if !str_contains(before.as_str(), "a\\u0000b") || !str_contains(before.as_str(), "9223372036854775807") { return result_err_i64(406); };
    let stale_copy = destination.copy(stale);
    if stale_copy.is_ok || stale_copy.error != 3 { return result_err_i64(407); };
    let missing = destination.copy(JsonValue { doc_handle: destination.handle, node_id: 999999 });
    if missing.is_ok || missing.error != 3 { return result_err_i64(408); };
    {
        let deep = json_doc_object()?;
        let mut current = deep.new_array()?;
        let first = current;
        let mut depth = 0;
        while depth < 70 { let next = deep.new_array()?; current.array_push(next)?; current = next; depth = depth + 1; };
        let rejected = destination.copy(first);
        if rejected.is_ok || rejected.error != 10 { return result_err_i64(409); };
    };
    {
        let cyclic = json_doc_object()?;
        let array = cyclic.new_array()?;
        array.array_push(array)?;
        let rejected = destination.copy(array);
        if rejected.is_ok || rejected.error != 2 { return result_err_i64(410); };
    };
    if destination.serialize_string()? != before { return result_err_i64(411); };
    {
        let crowded = json_doc_object()?;
        crowded.root().object_set("keep", crowded.new_i64(7)?)?;
        let mut nodes = 0;
        while nodes < 4093 { crowded.new_null()?; nodes = nodes + 1; };
        let source = json_parse("{\"a\":1}")?;
        let rejected = crowded.copy(source.root());
        if rejected.is_ok || rejected.error != 10 { return result_err_i64(412); };
        let final_slot = crowded.new_i64(9)?;
        crowded.root().object_set("last", final_slot)?;
        if crowded.root().object_get("last")?.number_i64()? != 9 || crowded.root().object_get("keep")?.number_i64()? != 7 { return result_err_i64(413); };
    };
    {
        let large = json_doc_object()?;
        let array = large.new_array()?;
"#,
    );
    source.push_str(&format!(
        "        let part = large.new_string(\"{}\")?;\n",
        "x".repeat(1024)
    ));
    source.push_str(
        r#"
        let mut entries = 0;
        while entries < 1025 { array.array_push(part)?; entries = entries + 1; };
        let rejected = destination.copy(array);
        if rejected.is_ok || rejected.error != 10 { return result_err_i64(414); };
    };
    if destination.serialize_string()? != before { return result_err_i64(415); };
    result_ok_i64(0)
}
def main() -> i64 { let result = verify(); if result.is_ok { 0 } else { println(result.error); 1 } }
"#,
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
        "JSON copy failed; artifacts retained {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
