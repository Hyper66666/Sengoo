mod common;

use common::source_sgc_command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

fn realworld(name: &str) -> PathBuf {
    workspace_root().join("examples/realworld").join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgc_realworld_{name}_{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn assert_sgc_check(path: &Path, module_map: Option<String>) {
    let mut command = source_sgc_command();
    command.arg("check").arg(path);
    if let Some(module_map) = module_map {
        command.env("SENGOO_MODULE_MAP", module_map);
    }
    let output = command.output().expect("run sgc check");
    assert!(
        output.status.success(),
        "sgc check failed for {}\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn realworld_sources_check_through_sgc_command() {
    assert_sgc_check(&realworld("async-channel-smoke").join("src/main.sg"), None);
    assert_sgc_check(&realworld("cli-json-audit").join("src/main.sg"), None);
    assert_sgc_check(
        &realworld("compressed-json-artifact").join("src/main.sg"),
        None,
    );
    let default_library = realworld("default-library-conformance");
    let default_library_map = format!(
        "default_library_conformance={}",
        default_library.join("src/lib.sg").display()
    );
    assert_sgc_check(
        &default_library.join("src/main.sg"),
        Some(default_library_map),
    );
    assert_sgc_check(&realworld("http-client-status").join("src/main.sg"), None);
    assert_sgc_check(&realworld("p0-foundations").join("src/main.sg"), None);

    let workspace_doc_loop = realworld("workspace-doc-loop");
    let module_map = format!(
        "workspace_doc_loop={}",
        workspace_doc_loop.join("src/lib.sg").display()
    );
    assert_sgc_check(&workspace_doc_loop.join("src/main.sg"), Some(module_map));
}

#[test]
fn default_library_conformance_runs_generic_string_keyed_map_natively() {
    let fixture = realworld("default-library-conformance");
    let module_map = format!(
        "default_library_conformance={}",
        fixture.join("src/lib.sg").display()
    );
    let output = source_sgc_command()
        .arg("run")
        .arg(fixture.join("src/main.sg"))
        .arg("--force-rebuild")
        .env("SENGOO_MODULE_MAP", module_map)
        .output()
        .expect("run default-library conformance fixture");
    assert!(
        output.status.success(),
        "default-library conformance failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn realworld_missing_import_check_reports_json_diagnostic() {
    let dir = temp_dir("missing_import");
    let source = dir.join("main.sg");
    fs::write(
        &source,
        "import definitely_missing_realworld_module;\n\ndef main() -> i64 {\n    0\n}\n",
    )
    .unwrap();

    let output = source_sgc_command()
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&source)
        .output()
        .expect("run sgc json check");
    assert!(!output.status.success(), "missing import should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let payload: Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|err| {
        panic!(
            "stderr should be machine-readable JSON ({err})\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            stderr
        )
    });
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["kind"], "compile_error");
    assert_eq!(payload["stage"], "import");
    assert_eq!(payload["input"], source.to_string_lossy().as_ref());
    assert!(payload["message"]
        .as_str()
        .unwrap_or_default()
        .contains("definitely_missing_realworld_module"));
    assert!(payload["location"]["line"].as_u64().is_some());
    assert!(payload["hint"]
        .as_str()
        .unwrap_or_default()
        .contains("import"));

    let _ = fs::remove_dir_all(dir);
}
