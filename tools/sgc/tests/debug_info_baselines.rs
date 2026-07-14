mod common;

use common::source_sgc_command;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const OPT_LEVEL: &str = "0";

struct FixtureSpec {
    name: &'static str,
    user_functions: &'static [&'static str],
}

const FIXTURES: &[FixtureSpec] = &[
    FixtureSpec {
        name: "scalar_control_flow",
        user_functions: &["classify", "main"],
    },
    FixtureSpec {
        name: "struct_method",
        user_functions: &["Point_sum", "main"],
    },
    FixtureSpec {
        name: "async_main",
        user_functions: &["step", "main"],
    },
];

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sgc_debug_info_baseline_{name}_{}_{}",
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&root).expect("create temp debug-info baseline project");
        Self { root }
    }

    fn source_path(&self, fixture: &str) -> PathBuf {
        self.root.join(format!("{fixture}.sg"))
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn baseline_root() -> PathBuf {
    workspace_root().join("compiler/tests/fixtures/debug-info-baselines")
}

fn baseline_source_path(fixture: &str) -> PathBuf {
    baseline_root().join(format!("{fixture}.sg"))
}

fn baseline_ir_path(fixture: &str) -> PathBuf {
    baseline_root().join(format!("{fixture}.ll"))
}

fn baseline_hash_path(fixture: &str) -> PathBuf {
    baseline_root().join(format!("{fixture}.ll.fnv64"))
}

fn copy_fixture_source(project: &TempProject, fixture: &str) -> PathBuf {
    let source_path = baseline_source_path(fixture);
    assert!(
        source_path.is_file(),
        "missing baseline source fixture {}",
        source_path.display()
    );
    let temp_source = project.source_path(fixture);
    fs::write(
        &temp_source,
        fs::read_to_string(&source_path).expect("read baseline source fixture"),
    )
    .expect("copy baseline source fixture");
    temp_source
}

fn run_sgc_build(source_path: &Path, debug_info: bool) -> String {
    let mut command = source_sgc_command();
    command
        .arg("build")
        .arg(source_path)
        .arg("--emit-llvm")
        .arg("--force-rebuild")
        .arg("-O")
        .arg(OPT_LEVEL)
        .args(["--target", "x86_64-pc-windows-msvc"]);
    if debug_info {
        command.arg("--debug-info");
    }
    let output = command.output().expect("run sgc build");
    assert!(
        output.status.success(),
        "sgc build failed for {} (debug_info={}):\nstdout:\n{}\nstderr:\n{}",
        source_path.display(),
        debug_info,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("fixture source should have a UTF-8 stem");
    fs::read_to_string(
        source_path
            .parent()
            .expect("fixture should have a parent")
            .join("build")
            .join(format!("{stem}.ll")),
    )
    .expect("read emitted LLVM IR")
}

fn fnv1a64_hex(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn switch_terminator_has_debug_location(ir: &str) -> bool {
    ir.match_indices("switch i64 ").any(|(start, _)| {
        ir[start..]
            .lines()
            .find(|line| line.trim_start().starts_with("],"))
            .is_some_and(|closing_line| closing_line.contains("!dbg !"))
    })
}

fn assert_fixture_behavior(spec: &FixtureSpec) {
    let expected_ir_path = baseline_ir_path(spec.name);
    let expected_hash_path = baseline_hash_path(spec.name);
    assert!(
        expected_ir_path.is_file(),
        "missing baseline LLVM IR {}",
        expected_ir_path.display()
    );
    assert!(
        expected_hash_path.is_file(),
        "missing baseline hash {}",
        expected_hash_path.display()
    );

    let expected_ir = fs::read_to_string(&expected_ir_path)
        .expect("read baseline LLVM IR")
        .replace("\r\n", "\n");
    let expected_hash = fs::read_to_string(&expected_hash_path)
        .expect("read baseline hash")
        .trim()
        .to_string();

    let project = TempProject::new(spec.name);
    let source_path = copy_fixture_source(&project, spec.name);

    let no_debug_ir = run_sgc_build(&source_path, false);
    assert!(
        !no_debug_ir.contains("!DICompileUnit") && !no_debug_ir.contains("!dbg !"),
        "non-debug LLVM IR unexpectedly contains DI metadata for {}:\n{}",
        spec.name,
        no_debug_ir
    );
    assert_eq!(
        no_debug_ir, expected_ir,
        "non-debug LLVM IR drifted for fixture {}",
        spec.name
    );
    assert_eq!(
        fnv1a64_hex(&no_debug_ir),
        expected_hash,
        "non-debug LLVM IR hash drifted for fixture {}",
        spec.name
    );

    let debug_ir = run_sgc_build(&source_path, true);
    assert!(
        debug_ir.contains("!llvm.dbg.cu"),
        "debug-info LLVM IR should contain !llvm.dbg.cu for {}:\n{}",
        spec.name,
        debug_ir
    );
    assert!(
        debug_ir.contains("!DICompileUnit"),
        "debug-info LLVM IR should contain DICompileUnit for {}:\n{}",
        spec.name,
        debug_ir
    );
    assert!(
        debug_ir.contains("!DIFile("),
        "debug-info LLVM IR should contain DIFile for {}:\n{}",
        spec.name,
        debug_ir
    );
    assert!(
        debug_ir.contains("!DISubprogram"),
        "debug-info LLVM IR should contain DISubprogram for {}:\n{}",
        spec.name,
        debug_ir
    );
    assert!(
        debug_ir.contains("!dbg !"),
        "debug-info LLVM IR should contain !dbg locations for {}:\n{}",
        spec.name,
        debug_ir
    );
    for function in spec.user_functions {
        assert!(
            debug_ir.contains(&format!("!DISubprogram(name: \"{function}\"")),
            "debug-info LLVM IR should contain a DISubprogram for `{function}` in fixture {}:\n{}",
            spec.name,
            debug_ir
        );
    }
    if spec.name == "async_main" {
        assert!(
            switch_terminator_has_debug_location(&debug_ir),
            "async dispatch switch should carry a debug location:\n{debug_ir}"
        );
    }
    assert_ne!(
        debug_ir, expected_ir,
        "debug-info LLVM IR should differ from the non-debug baseline for fixture {}",
        spec.name
    );

    let roundtrip_no_debug_ir = run_sgc_build(&source_path, false);
    assert_eq!(
        roundtrip_no_debug_ir, expected_ir,
        "non-debug LLVM IR should remain baseline-stable after a debug-info build for fixture {}",
        spec.name
    );
    assert_eq!(
        fnv1a64_hex(&roundtrip_no_debug_ir),
        expected_hash,
        "non-debug LLVM IR hash should remain baseline-stable after a debug-info build for fixture {}",
        spec.name
    );
}

#[test]
fn scalar_control_flow_non_debug_ir_matches_baseline_and_debug_info_adds_di() {
    assert_fixture_behavior(&FIXTURES[0]);
}

#[test]
fn struct_method_non_debug_ir_matches_baseline_and_debug_info_adds_di() {
    assert_fixture_behavior(&FIXTURES[1]);
}

#[test]
fn async_main_non_debug_ir_matches_baseline_and_debug_info_adds_di() {
    assert_fixture_behavior(&FIXTURES[2]);
}
