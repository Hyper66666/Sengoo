use crate::compile_to_ir;
use std::fs;
use std::path::Path;

fn load_stdlib_surface(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
    let stdlib_root = workspace_root.join("tools").join("stdlib");
    modules
        .iter()
        .map(|module| {
            let stdlib_path = stdlib_root.join(module);
            fs::read_to_string(&stdlib_path).unwrap_or_else(|err| {
                panic!(
                    "failed to read stdlib surface {}: {err}",
                    stdlib_path.display()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compile_with_stdlib_modules(modules: &[&str], program: &str) -> String {
    let source = format!("{}\n\n{}", load_stdlib_surface(modules), program);
    compile_to_ir(&source)
        .unwrap_or_else(|err| panic!("stdlib breadth program should compile: {err}"))
}

#[test]
fn assert_module_compiles() {
    let ir = compile_with_stdlib_modules(
        &["assert.sg"],
        r#"
def main() -> i64 {
    if assert_eq_i64(2 + 2, 4) { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("assert_eq_i64"));
    assert!(ir.contains("sengoo_assert_failure_v1"));
}

#[test]
fn regex_module_compiles() {
    let ir = compile_with_stdlib_modules(
        &["option.sg", "result.sg", "ffi.sg", "status.sg", "regex.sg"],
        r#"
def main() -> i64 {
    let pattern = regex_compile("^hello.*$");
    if pattern.is_ok {
        let re = pattern.value;
        let matched = re.is_match("hello world");
        re.drop();
        if matched.is_ok && matched.value { 1 } else { 0 }
    } else {
        0
    }
}
"#,
    );
    assert!(ir.contains("regex_compile"));
}

#[test]
fn log_and_time_modules_compile() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "log.sg",
            "time.sg",
        ],
        r#"
def main() -> i64 {
    let level = log_set_level(2);
    let now = time_unix_ms();
    if level.is_ok && now > 0 { 1 } else { 0 }
}
"#,
    );
    assert!(ir.contains("log_set_level"));
    assert!(ir.contains("time_unix_ms"));
}

#[test]
fn config_hash_encoding_modules_compile() {
    let ir = compile_with_stdlib_modules(
        &[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "status.sg",
            "config.sg",
            "hash.sg",
            "encoding.sg",
        ],
        r#"
def main() -> i64 {
    let doc = ini_parse("name=sengoo");
    if doc.is_ok {
        doc.value.drop();
        1
    } else {
        0
    }
}
"#,
    );
    assert!(ir.contains("ini_parse"));
}
