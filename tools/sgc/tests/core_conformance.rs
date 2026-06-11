use std::path::{Path, PathBuf};
use std::process::Command;

fn sgc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgc"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn run_core_case(tag: &str, relative_path: &str, expected_exit: i32, expected_stdout: &str) {
    let path = workspace_root().join(relative_path);
    let output = Command::new(sgc())
        .arg("run")
        .arg(&path)
        .arg("--force-rebuild")
        .output()
        .unwrap_or_else(|err| panic!("failed to run sgc for {tag} ({relative_path}): {err}"));

    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "{tag} ({relative_path}) exit mismatch\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if !expected_stdout.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.lines().any(|line| line.trim() == expected_stdout),
            "{tag} ({relative_path}) did not print expected line {expected_stdout:?}\nstdout:\n{}\nstderr:\n{}",
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn core_conformance_examples_compile_link_and_run() {
    let cases = [
        (
            "core-scalars-control",
            "examples/conformance/01_scalars_control.sg",
            9,
            "core",
        ),
        (
            "core-recursion",
            "examples/conformance/02_recursion.sg",
            13,
            "",
        ),
        ("core-struct", "examples/08_struct.sg", 30, ""),
        ("core-method", "examples/09_method_call.sg", 43, ""),
        ("core-array-read", "examples/04_array.sg", 20, ""),
        ("core-array-for", "examples/05_loop.sg", 15, ""),
        (
            "core-array-write",
            "examples/conformance/03_array_write.sg",
            42,
            "",
        ),
        ("core-closure", "examples/06_lambda.sg", 15, ""),
        (
            "core-closure-multi",
            "examples/conformance/04_closure_multi_capture.sg",
            18,
            "",
        ),
        (
            "core-enum-value",
            "examples/ergonomics/03_enum_match.sg",
            2,
            "",
        ),
        (
            "core-enum-payload",
            "examples/conformance/05_enum_payload.sg",
            42,
            "",
        ),
        (
            "core-enum-multi-payload",
            "examples/conformance/06_enum_multi_payload.sg",
            42,
            "",
        ),
        (
            "core-enum-return",
            "examples/conformance/07_enum_return.sg",
            42,
            "",
        ),
    ];

    for (tag, relative_path, expected_exit, expected_stdout) in cases {
        run_core_case(tag, relative_path, expected_exit, expected_stdout);
    }
}

#[test]
fn core_conformance_examples_are_real_workspace_paths() {
    for relative_path in [
        "examples/conformance/01_scalars_control.sg",
        "examples/conformance/02_recursion.sg",
        "examples/04_array.sg",
        "examples/05_loop.sg",
        "examples/conformance/03_array_write.sg",
        "examples/06_lambda.sg",
        "examples/conformance/04_closure_multi_capture.sg",
        "examples/ergonomics/03_enum_match.sg",
        "examples/conformance/05_enum_payload.sg",
        "examples/conformance/06_enum_multi_payload.sg",
        "examples/conformance/07_enum_return.sg",
    ] {
        assert!(
            Path::new(&workspace_root().join(relative_path)).exists(),
            "core conformance case must exist: {relative_path}"
        );
    }
}
