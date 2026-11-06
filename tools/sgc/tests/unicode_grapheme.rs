//! Full pinned Unicode 17 UAX #29 extended-grapheme corpus plus editing widths.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};
fn clang() -> PathBuf {
    if let Some(value) = std::env::var_os("SENGOO_CLANG") {
        return value.into();
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(local)
                .join("SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe");
            if p.is_file() {
                return p;
            }
        }
    }
    "clang".into()
}
fn passed(o: Output, label: &str) {
    assert!(
        o.status.success(),
        "{label} ({}):\n{}\n{}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
fn unicode17_extended_graphemes_and_terminal_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = std::env::temp_dir().join(format!(
        "sengoo-grapheme-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&work).unwrap();
    let input =
        fs::read_to_string(root.join("tools/stdlib/unicode-data/17.0.0/GraphemeBreakTest.txt"))
            .unwrap();
    let mut cases = String::new();
    let mut count = 0;
    for line in input.lines() {
        let body = line.split('#').next().unwrap().trim();
        if body.is_empty() {
            continue;
        }
        let pieces = body.split_whitespace().collect::<Vec<_>>();
        let mut scalars = Vec::new();
        let mut boundaries = Vec::new();
        for pair in pieces.chunks(2) {
            if pair.len() < 2 {
                break;
            }
            boundaries.push(pair[0] == "÷");
            scalars.push(u32::from_str_radix(pair[1], 16).unwrap());
        }
        cases.push_str(&serde_json::to_string(&serde_json::json!([scalars, boundaries])).unwrap());
        cases.push('\n');
        count += 1;
    }
    assert!(count > 700, "the complete pinned corpus must run");
    let fixture = work.join("grapheme.ndjson");
    fs::write(&fixture, &cases).unwrap();
    let source = work.join("main.sg");
    fs::write(&source, r#"
import std::unicode;
import std::file;
import std::json;
def verify_case(line: &str) -> i64 {
    let document = json_parse(line);
    if !document.is_ok { return 10; }
    let scalars = document.value.root().array_get(0).value;
    let expected = document.value.root().array_get(1).value;
    let count = scalars.array_len().value; let mut index = 0;
    let mut state = unicode_grapheme_state();
    while index < count {
        let scalar = scalars.array_get(index).value.number_i64();
        if !scalar.is_ok { return 11; }
        let boundary = state.push(scalar.value);
        if !boundary.is_ok || boundary.value != expected.array_get(index).value.bool_value().value { return 12; }
        index = index + 1;
    }
    return 0;
}
def width(value: &str, expected: i64) -> bool {
    let text = string_from_str(value).value;
    let actual = unicode_display_width(&text);
    actual.is_ok && actual.value == expected;
}
def editing() -> i64 {
    if !width("中文", 4) || !width("e\u{301}", 1) || !width("😀", 2) { return 31; }
    if !width("👨‍👩‍👧‍👦", 2) || !width("🇨🇳", 2) || !width("👍🏽", 2) { return 32; }
    if !width("1️⃣", 2) || !width("©️", 2) || !width("©", 1) { return 33; }
    if !width("각", 2) || !width("क्‍ष", 1) || !width("\u{301}", 0) { return 34; }
    let text = string_from_str("a👨‍👩‍👧‍👦e\u{301}🇨🇳中").value;
    if unicode_next_grapheme(&text, 0).value != 1 { return 35; }
    if unicode_next_grapheme(&text, 1).value != 26 { return 36; }
    if unicode_previous_grapheme(&text, 26).value != 1 { return 37; }
    if unicode_next_grapheme(&text, 26).value != 29 { return 38; }
    if unicode_next_grapheme(&text, 29).value != 37 { return 39; }
    if unicode_previous_grapheme(&text, 40).value != 37 { return 40; }
    if unicode_next_grapheme(&text, 2).is_ok || unicode_previous_grapheme(&text, 27).is_ok { return 41; }
    if unicode_next_grapheme(&text, -1).is_ok || unicode_previous_grapheme(&text, 99).is_ok { return 42; }
    let with_nul = string_new(); with_nul.push_char('a'); with_nul.push_char('\0'); with_nul.push_char('b');
    if with_nul.len() != 3 || unicode_next_grapheme(&with_nul, 1).value != 2 || unicode_display_width(&with_nul).value != 2 { return 43; }
    let mut state = unicode_grapheme_state();
    if state.push(55296).is_ok || state.push(1114112).is_ok || unicode_scalar_cells(-1).is_ok { return 44; }
    return 0;
}
def main() -> i64 {
    let data = ffi_buffer_new(FIXTURE_CAPACITY);
    if !data.is_ok { return 1; }
    let read = file_read_into("FIXTURE_PATH", data.value);
    if !read.is_ok { return 2; }
    let content = string_from_buffer(data.value, read.value);
    if !content.is_ok { return 3; }
    let lines = content.value.split("\n"); let mut count = 0;
    loop {
        let next = lines.next(); if !next.is_some { break; }
        if next.value.len() > 0 {
            let code = verify_case(next.value.as_str());
            if code != 0 { print(count); return code; }
            count = count + 1;
        }
    }
    lines.free(); data.value.free();
    if count != FIXTURE_COUNT { return 4; }
    return editing();
}
"#.replace("FIXTURE_PATH", &fixture.to_string_lossy().replace('\\', "/")).replace("FIXTURE_CAPACITY", &(cases.len() + 1).to_string()).replace("FIXTURE_COUNT", &count.to_string())).unwrap();
    let compiler = clang();
    let mut paths = compiler
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let binary = work.join(if cfg!(windows) { "main.exe" } else { "main" });
    passed(
        Command::new(env!("CARGO_BIN_EXE_sgc"))
            .args(["--runtime-mode", "source-development", "build"])
            .arg(&source)
            .arg("--output")
            .arg(&binary)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .output()
            .unwrap(),
        "compile pure Sengoo Unicode graphemes",
    );
    passed(
        Command::new(&binary).output().unwrap(),
        &format!("Unicode17 official {count} cases and editing/width regressions"),
    );
}
