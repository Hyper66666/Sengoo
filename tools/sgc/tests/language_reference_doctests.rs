mod common;

use common::source_sgc_command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoctestMode {
    Compile,
    Run,
}

#[derive(Debug, PartialEq, Eq)]
struct ReferenceDoctest {
    mode: DoctestMode,
    source: String,
    expected_stdout: Option<String>,
    fence_line: usize,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc should live under tools/sgc")
        .to_path_buf()
}

fn parse_reference_doctests(markdown: &str) -> Result<Vec<ReferenceDoctest>, String> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut tests = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if !line.starts_with("```sg") {
            index += 1;
            continue;
        }
        let mode = match line.strip_prefix("```sg").map(str::trim) {
            Some("compile") => DoctestMode::Compile,
            Some("run") => DoctestMode::Run,
            Some(other) => {
                return Err(format!(
                    "Sengoo fence at line {} needs `compile` or `run`, found `{other}`",
                    index + 1
                ));
            }
            None => unreachable!(),
        };
        let fence_line = index + 1;
        index += 1;
        let mut source_lines = Vec::new();
        while index < lines.len() && lines[index].trim() != "```" {
            source_lines.push(lines[index]);
            index += 1;
        }
        if index == lines.len() {
            return Err(format!("unterminated Sengoo fence at line {fence_line}"));
        }
        let expected_stdout = source_lines.iter().find_map(|line| {
            line.trim()
                .strip_prefix("// doctest-stdout:")
                .map(str::trim)
                .map(str::to_string)
        });
        if mode == DoctestMode::Run && expected_stdout.is_none() {
            return Err(format!(
                "run fence at line {fence_line} needs `// doctest-stdout:`"
            ));
        }
        tests.push(ReferenceDoctest {
            mode,
            source: format!("{}\n", source_lines.join("\n")),
            expected_stdout,
            fence_line,
        });
        index += 1;
    }
    Ok(tests)
}

fn write_temp_source(index: usize, source: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sengoo-language-reference-{}-{nonce}-{index}.sg",
        std::process::id()
    ));
    fs::write(&path, source).expect("doctest source should be writable");
    path
}

fn run_sgc(mode: DoctestMode, source: &Path) -> Output {
    let command = match mode {
        DoctestMode::Compile => "check",
        DoctestMode::Run => "run",
    };
    source_sgc_command()
        .arg(command)
        .arg(source)
        .args((mode == DoctestMode::Run).then_some("--force-rebuild"))
        .current_dir(workspace_root())
        .output()
        .expect("sgc should launch")
}

#[test]
fn language_reference_sengoo_fences_compile_and_run_as_marked() {
    let reference_path = workspace_root().join("docs/language-reference.md");
    let markdown = fs::read_to_string(&reference_path).expect("language reference should exist");
    let tests = parse_reference_doctests(&markdown).expect("reference fences should be marked");
    assert!(
        tests.len() >= 5,
        "the authoritative reference should keep representative executable examples"
    );

    for (index, test) in tests.iter().enumerate() {
        let source_path = write_temp_source(index, &test.source);
        let output = run_sgc(test.mode, &source_path);
        let _ = fs::remove_file(&source_path);
        assert!(
            output.status.success(),
            "reference doctest at fence line {} failed\nstdout:\n{}\nstderr:\n{}",
            test.fence_line,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(expected) = &test.expected_stdout {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.lines().any(|line| line.trim() == expected),
                "reference run doctest at fence line {} did not emit expected line `{expected}`:\n{stdout}",
                test.fence_line,
            );
        }
    }
}

#[test]
fn reference_doctest_parser_rejects_unmarked_sengoo_fences() {
    let error = parse_reference_doctests("```sg\ndef main() {}\n```\n")
        .expect_err("unmarked Sengoo fences should fail");
    assert!(error.contains("compile") && error.contains("run"));
}
