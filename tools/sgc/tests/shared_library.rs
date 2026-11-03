//! Native ABI regressions: library-only programs and deliberately bounded exports.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "sengoo-shared-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn shared_library_keeps_c_export_without_main() {
    let fixture = Fixture::new();
    let source = fixture.0.join("library.sg");
    fs::write(&source, "#[export_name = \"answer\"]\npub extern \"C\" fn compute(value: i64) -> i64 { value + 1 }\ndef private_helper() -> i64 { 999 }\n").unwrap();
    let extension = if cfg!(windows) {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let library = fixture.0.join(format!("library.{extension}"));
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .args(["--runtime-mode", "source-development", "build"])
        .arg(&source)
        .arg("--emit-shared")
        .arg("--output")
        .arg(&library)
        .output()
        .expect("run shared compiler");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(library.is_file());
    let python = std::env::var("SENGOO_PYTHON").unwrap_or_else(|_| "python".into());
    let verify = "import ctypes,sys; lib=ctypes.PyDLL(sys.argv[1]); f=lib.answer; f.argtypes=[ctypes.c_int64]; f.restype=ctypes.c_int64; assert f(41)==42\nfor name in ('compute','private_helper','sengoo_string_free_status'):\n try: getattr(lib,name)\n except AttributeError: pass\n else: raise AssertionError('unexpected export: '+name)\n";
    let status = Command::new(python)
        .args(["-I", "-c", verify])
        .arg(&library)
        .status()
        .expect("Python is required by native shared-library ABI tests; set SENGOO_PYTHON");
    assert!(status.success());
}

#[test]
fn incompatible_shared_flags_are_rejected_before_compilation() {
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .args(["build", "missing.sg", "--emit-shared", "--emit-llvm"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .args(["build", "missing.sg", "--python-export", "example"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--emit-shared"));
}
