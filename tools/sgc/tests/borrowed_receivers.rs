//! Actual CLI owners exposed these receiver inconsistencies; keep native
//! behavior checked independently of MIR string matching.
mod common;
use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn borrowed_string_views_and_destructor_helpers_run_with_exact_field_cleanup() {
    let scratch = std::env::temp_dir().join(format!("sengoo-borrowed-receivers-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&scratch).unwrap();
    let source = scratch.join("main.sg");
    fs::write(&source, r#"
import std::string;
import std::io;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
struct Owner { marker: i64, label: String }
def touch(owner: &mut Owner) { println(owner.marker); println(owner.label.as_str()); owner.marker = owner.marker + 1; (); }
impl Drop for Owner { def drop(&mut self) { touch(self); touch(&mut self); (); } }
def shared(value: &String) -> i64 { value.as_str().len() }
def exclusive(value: &mut String) -> i64 { value.as_str().len() }
def owners() -> i64 {
    let first = string_from_str("abc").value;
    let mut second = string_from_str("xyz").value;
    let count = shared(&first) + exclusive(&mut second);
    let owned = Owner { marker: 42, label: string_from_str("drop").value };
    count
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let count = owners();
    if count != 6 or sengoo_string_live_handle_count() != before { return 1; }; 0
}
"#).unwrap();
    for level in ["0", "2"] {
        let binary = scratch.join(if cfg!(windows) { format!("main-{level}.exe") } else { format!("main-{level}") });
        let built = common::source_sgc_command().arg("build").arg(&source).args(["--force-rebuild", "-O", level, "-o"]).arg(&binary).output().unwrap();
        assert!(built.status.success(), "{}\n{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
        let run = Command::new(&binary).output().unwrap();
        assert!(run.status.success(), "{run:?}");
        assert_eq!(String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"), "42\ndrop\n43\ndrop\n");
        assert!(run.stderr.is_empty(), "{run:?}");
        fs::remove_file(binary).unwrap();
    }
    // The scratch is ours. Avoid recursive removal or deleting any artifacts
    // whose paths have not been explicitly inventoried.
    fs::remove_file(source).unwrap();
    if fs::read_dir(&scratch).unwrap().next().is_none() { fs::remove_dir(scratch).unwrap(); }
}
