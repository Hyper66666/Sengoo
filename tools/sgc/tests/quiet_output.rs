use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempSource {
    root: PathBuf,
    path: PathBuf,
}

impl TempSource {
    fn new(name: &str, source: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sengoo-quiet-output-{}-{}-{nonce}",
            name,
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create quiet-output test directory");
        let path = root.join("main.sg");
        fs::write(&path, source).expect("write quiet-output source");
        Self { root, path }
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn sgc() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sgc"));
    command.env("RUST_LOG", "off");
    command.args(["--runtime-mode", "source-development"]);
    command
}

fn combined(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn contains_instrumentation(text: &str) -> bool {
    [
        "cache miss",
        "cache hit",
        "codegen workset manifest",
        "frontend session",
        "frontend probe",
        "generic instance",
        "Running:",
        "Building:",
        "unused-command-line-argument",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

#[test]
fn successful_run_is_quiet_by_default_and_verbose_restores_instrumentation() {
    let source = TempSource::new(
        "quiet-run",
        r#"
def main() -> i64 {
    0
}
"#,
    );

    let quiet = sgc()
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .output()
        .expect("sgc run should launch");
    let quiet_text = combined(&quiet);
    assert_eq!(
        quiet.status.code(),
        Some(0),
        "quiet run should succeed\n{quiet_text}"
    );
    assert_eq!(
        String::from_utf8_lossy(&quiet.stdout).trim(),
        "exit code: 0",
        "default verbosity should print only the result line, got:\n{}",
        String::from_utf8_lossy(&quiet.stdout)
    );
    assert!(
        !contains_instrumentation(&quiet_text),
        "default verbosity must hide compiler instrumentation, got:\n{quiet_text}"
    );

    let verbose = sgc()
        .arg("--verbose")
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .output()
        .expect("sgc --verbose run should launch");
    let verbose_text = combined(&verbose);
    assert_eq!(
        verbose.status.code(),
        Some(0),
        "verbose run should succeed\n{verbose_text}"
    );
    assert!(
        verbose_text.contains("Running:"),
        "verbose mode should restore the previous run banner, got:\n{verbose_text}"
    );
    assert!(
        verbose_text.contains("cache bypassed: --force-rebuild")
            || verbose_text.contains("cache miss")
            || verbose_text.contains("codegen workset manifest")
            || verbose_text.contains("generic instance"),
        "verbose mode should restore cache/workset/generic-instance detail, got:\n{verbose_text}"
    );
    assert!(
        String::from_utf8_lossy(&verbose.stdout).contains("exit code: 0"),
        "verbose mode should still print the result line, got:\n{}",
        String::from_utf8_lossy(&verbose.stdout)
    );
}

#[test]
fn compile_errors_stay_visible_at_default_verbosity_in_english() {
    let source = TempSource::new(
        "undefined",
        r#"
def main() -> i64 {
    missing
}
"#,
    );

    let output = sgc()
        .arg("check")
        .arg(&source.path)
        .output()
        .expect("sgc check should launch");
    let text = combined(&output);
    assert_ne!(
        output.status.code(),
        Some(0),
        "undefined variable should fail"
    );
    assert!(
        text.contains("undefined variable"),
        "actionable diagnostics must stay visible at default verbosity, got:\n{text}"
    );
    assert!(
        !text.contains("未定义"),
        "diagnostics must be English, got:\n{text}"
    );
}

#[test]
fn error_format_json_keeps_schema_and_english_messages() {
    let source = TempSource::new(
        "json-error",
        r#"
enum Color { Red, Blue }

def paint(c: Color) -> i64 {
    match c {
        Color::Red => 1,
    }
}

def main() -> i64 {
    paint(Color::Red)
}
"#,
    );

    let output = sgc()
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&source.path)
        .output()
        .expect("sgc json check should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(output.status.code(), Some(0));

    let start = stderr.find('{').expect("json payload on stderr");
    let end = stderr.rfind('}').expect("json payload terminator");
    let value: Value = serde_json::from_str(&stderr[start..=end]).expect("valid json payload");

    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["ok"], false);
    assert_eq!(value["kind"], "compile_error");
    assert_eq!(value["code"], "non-exhaustive-match");
    assert_eq!(value["stage"], "typecheck");
    let message = value["message"].as_str().unwrap_or("");
    assert!(
        message.contains("non-exhaustive-match"),
        "json message should keep the stable code text, got: {message}"
    );
    assert!(
        !message.contains("未覆盖")
            && !message
                .chars()
                .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
        "json message must stay English, got: {message}"
    );
}
