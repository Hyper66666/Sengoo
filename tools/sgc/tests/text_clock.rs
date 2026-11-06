//! Real Sengoo decoding, independently compared with Rust's UTF-8 oracle.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn passed(output: Output, label: &str) -> Output {
    assert!(output.status.success(), "{label}: {}\n{}\n{}", output.status,
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    output
}
fn build(source: &str, directory: &Path) -> PathBuf {
    let binary = directory.join(if cfg!(windows) { format!("{source}.exe") } else { source.into() });
    let compiler = std::env::var_os("SENGOO_TEST_SGC").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_sgc")));
    passed(Command::new(compiler).current_dir(root())
        .args(["--runtime-mode", "source-development", "build"])
        .arg(root().join(format!("tools/stdlib/tests/{source}.sg")))
        .args(["-O", "0", "-o"]).arg(&binary).output().unwrap(), source);
    binary
}

#[test]
fn lossy_utf8_matches_independent_oracle_and_monotonic_clock_runs() {
    let directory = std::env::temp_dir().join(format!("sengoo-text-clock-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&directory).unwrap();
    passed(Command::new(build("lossy_monotonic", &directory)).output().unwrap(), "text/clock boundary cases");
    let mut bytes = Vec::new();
    let mut count = 0;
    let mut append = |input: &[u8]| {
        let output = String::from_utf8_lossy(input);
        bytes.push(input.len() as u8); bytes.push(output.len() as u8);
        bytes.extend_from_slice(input); bytes.extend_from_slice(output.as_bytes()); count += 1;
    };
    append(&[]);
    for first in 0..=255u8 { append(&[first]); }
    for first in 0..=255u8 { for second in 0..=255u8 { append(&[first, second]); } }
    let mut state = 0xd15ea5e5_u64;
    for index in 0..4096 {
        let mut input = Vec::new();
        for _ in 0..=(index % 64) {
            state ^= state << 13; state ^= state >> 7; state ^= state << 17;
            input.push(state as u8);
        }
        append(&input);
    }
    let vectors = directory.join("vectors.bin"); fs::write(&vectors, bytes).unwrap();
    let output = passed(Command::new(build("utf8_lossy_vectors", &directory))
        .arg(vectors).output().unwrap(), "UTF-8 differential vectors");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), count.to_string());
    assert_eq!(count, 69889);
}
