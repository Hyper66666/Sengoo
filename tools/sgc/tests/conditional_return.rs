//! CLI checkpoint recovery exposed a conditional-return/Unit Phi mismatch.
use std::{fs, path::Path, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn conditional_return_merges_only_continuing_branch_values() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = std::env::temp_dir().join(format!("sengoo-conditional-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&scratch).unwrap();
    let source = scratch.join("main.sg");
    let binary = scratch.join(if cfg!(windows) {"main.exe"} else {"main"});
    fs::write(&source, r#"
def left(flag: bool) -> i64 { if flag { return 17; } else { 23 } }
def right(flag: bool) -> i64 { if flag { 31 } else { return 47; } }
def guard(flag: bool) -> i64 { if flag { return 59; }; 61 }
def main() -> i64 {
    if left(true) != 17 or left(false) != 23 { return 1; };
    if right(true) != 31 or right(false) != 47 { return 2; };
    if guard(true) != 59 or guard(false) != 61 { return 3; };
    0
}
"#).unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_sgc")).current_dir(&repo).args(["--runtime-mode","source-development","build"]).arg(&source).arg("--force-rebuild").arg("-o").arg(&binary).output().unwrap();
    assert!(build.status.success(), "{}\n{}", String::from_utf8_lossy(&build.stdout), String::from_utf8_lossy(&build.stderr));
    let run = Command::new(&binary).output().unwrap();
    assert!(run.status.success(), "{:?}: {}", run.status, String::from_utf8_lossy(&run.stderr));
    fs::write(&source, r#"
def run(a: bool, b: bool) -> bool {
    if a { true; } else { if b { return false; }; };
    true
}
def main() -> i64 { run(false,false); 0 }
"#).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_sgc")).current_dir(&repo).args(["--runtime-mode","source-development","check"]).arg(&source).output().unwrap();
    assert!(!rejected.status.success(), "bool versus () must fail before LLVM");
    let diagnostic = format!("{}{}", String::from_utf8_lossy(&rejected.stdout), String::from_utf8_lossy(&rejected.stderr));
    assert!(diagnostic.contains("类型不匹配") || diagnostic.contains("mismatch"), "{diagnostic}");
}
