use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn sgpm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgpm"))
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("sgpm_integration_{}_{}", name, stamp));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_sgpm(args: &[&str], cwd: &Path) -> Output {
    Command::new(sgpm())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run sgpm")
}

fn write_pkg(root: &Path, name: &str, deps: &[(&str, &str)]) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
    let mut text = format!(
        "[package]\nname = '{}'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n",
        name
    );
    if !deps.is_empty() {
        text.push_str("\n[dependencies]\n");
        for (dep_name, dep_path) in deps {
            text.push_str(&format!(
                "{} = {{ path = '{}' }}\n",
                dep_name,
                dep_path.replace('\\', "\\\\")
            ));
        }
    }
    fs::write(root.join("Sengoo.toml"), text).unwrap();
}

#[cfg(windows)]
fn fake_sgc(dir: &Path) -> PathBuf {
    let script = dir.join("sgc.cmd");
    fs::write(
        &script,
        "@echo off\r\necho %CD% :: %* >> \"%SGPM_RECORD%\"\r\nexit /b 0\r\n",
    )
    .unwrap();
    script
}

#[cfg(not(windows))]
fn fake_sgc(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let script = dir.join("sgc");
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s :: %s\\n' \"$PWD\" \"$*\" >> \"$SGPM_RECORD\"\nexit 0\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

#[test]
fn parses_minimal_manifest() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello");
    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            fixture.join("Sengoo.toml").to_str().unwrap(),
        ],
        &fixture,
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello v0.1.0"), "stdout:\n{}", stdout);
}

#[test]
fn resolves_topological_order_three_packages() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dep_chain/a");
    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            fixture.join("Sengoo.toml").to_str().unwrap(),
        ],
        &fixture,
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let c_pos = stdout.find("c v0.1.0").expect("c in tree");
    let b_pos = stdout.find("b v0.1.0").expect("b in tree");
    let a_pos = stdout.find("a v0.1.0").expect("a in tree");
    assert!(c_pos < b_pos && b_pos < a_pos, "tree output:\n{}", stdout);
}

#[test]
fn rejects_cyclic_path_deps() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cycle/a");
    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            fixture.join("Sengoo.toml").to_str().unwrap(),
        ],
        &fixture,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cyclic path dependency"),
        "stderr:\n{}",
        stderr
    );
}

#[test]
fn sgpm_new_creates_expected_layout() {
    let dir = temp_dir("new");
    let project = dir.join("demo");
    let output = run_sgpm(
        &["new", "demo_pkg", "--path", project.to_str().unwrap()],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("Sengoo.toml").exists());
    assert!(project.join("src/main.sg").exists());
    assert!(project.join("tests").exists());
    let manifest = fs::read_to_string(project.join("Sengoo.toml")).unwrap();
    assert!(manifest.contains("name = \"demo_pkg\""));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn tree_orders_path_dependencies_before_root() {
    let dir = temp_dir("tree");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let dep_pos = stdout.find("dep v0.1.0").expect("dep in tree");
    let app_pos = stdout.find("app v0.1.0").expect("app in tree");
    assert!(dep_pos < app_pos, "tree output:\n{}", stdout);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_build_invokes_sgc_in_topo_order() {
    let dir = temp_dir("build_order");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "build",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm build");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    let dep_pos = log.find("/dep :: build").expect("dep build");
    let app_pos = log.find("/app :: build").expect("app build");
    assert!(dep_pos < app_pos, "build log:\n{}", log);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejects_remote_dep_without_registry() {
    let dir = temp_dir("registry_dep");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\n[dependencies]\nfoo = '1.0.0'\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("registry version") && stderr.contains("supports path dependencies"),
        "stderr:\n{}",
        stderr
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn clean_removes_root_target_dir_only() {
    let dir = temp_dir("clean");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);
    fs::create_dir_all(app.join("target/debug")).unwrap();
    fs::create_dir_all(dep.join("target/debug")).unwrap();
    fs::write(app.join("target/debug/app"), "").unwrap();
    fs::write(dep.join("target/debug/dep"), "").unwrap();

    let output = run_sgpm(
        &[
            "clean",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(output.status.success());
    assert!(!app.join("target").exists());
    assert!(dep.join("target").exists());
    let _ = fs::remove_dir_all(dir);
}
