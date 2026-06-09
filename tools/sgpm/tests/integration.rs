use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use tar::Archive;

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

#[derive(Debug)]
struct HttpRequestCapture {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn spawn_one_request_server() -> (
    String,
    mpsc::Receiver<HttpRequestCapture>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut buf).unwrap();
            if read == 0 {
                return;
            }
            data.extend_from_slice(&buf[..read]);
            if let Some(pos) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let headers_text = String::from_utf8_lossy(&data[..header_end]);
        let mut lines = headers_text.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let path = request_parts.next().unwrap_or_default().to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
            .collect::<Vec<_>>();
        let content_length = headers
            .iter()
            .find(|(name, _)| name == "content-length")
            .and_then(|(_, value)| value.parse::<usize>().ok())
            .unwrap_or(0);
        while data.len().saturating_sub(header_end) < content_length {
            let read = stream.read(&mut buf).unwrap();
            if read == 0 {
                break;
            }
            data.extend_from_slice(&buf[..read]);
        }
        let body = data[header_end..].to_vec();
        let _ = tx.send(HttpRequestCapture {
            method,
            path,
            headers,
            body,
        });
        let response = b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
        stream.write_all(response).unwrap();
    });

    (format!("http://{}", addr), rx, handle)
}

fn read_http_request(stream: &mut TcpStream) -> HttpRequestCapture {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut buf).unwrap();
        if read == 0 {
            panic!("connection closed before headers");
        }
        data.extend_from_slice(&buf[..read]);
        if let Some(pos) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let headers_text = String::from_utf8_lossy(&data[..header_end]);
    let mut lines = headers_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
        .collect::<Vec<_>>();
    let content_length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    while data.len().saturating_sub(header_end) < content_length {
        let read = stream.read(&mut buf).unwrap();
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buf[..read]);
    }
    let body = data[header_end..].to_vec();
    HttpRequestCapture {
        method,
        path,
        headers,
        body,
    }
}

fn spawn_remote_package_server(
    package: &str,
    version: &str,
    checksum: &str,
    archive: Vec<u8>,
) -> (String, mpsc::Receiver<Vec<String>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let package = package.to_string();
    let version = version.to_string();
    let checksum = checksum.to_string();
    let handle = thread::spawn(move || {
        let mut seen_paths = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            seen_paths.push(request.path.clone());
            if request.path == format!("/api/v1/packages/{}", package) {
                let body = format!(
                    r#"{{"versions":[{{"version":"{}","checksum":"{}"}}]}}"#,
                    version, checksum
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).unwrap();
            } else if request.path == format!("/api/v1/packages/{}/{}/download", package, version) {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    archive.len()
                );
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(&archive).unwrap();
            } else {
                let response =
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream.write_all(response).unwrap();
            }
        }
        let _ = tx.send(seen_paths);
    });

    (format!("http://{}", addr), rx, handle)
}

#[test]
fn version_flag_prints_package_version() {
    let dir = temp_dir("version");
    let output = run_sgpm(&["--version"], &dir);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output should include package version:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

fn write_pkg(root: &Path, name: &str, deps: &[(&str, &str)]) {
    write_pkg_version(root, name, "0.1.0", deps);
}

fn write_pkg_version(root: &Path, name: &str, version: &str, deps: &[(&str, &str)]) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
    let mut text = format!(
        "[package]\nname = '{}'\nversion = '{}'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n",
        name, version
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

fn write_lib_pkg(root: &Path, name: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.sg"),
        "def imported_value() -> i64 { 42 }\n",
    )
    .unwrap();
    fs::write(
        root.join("Sengoo.toml"),
        format!(
            "[package]\nname = '{}'\nversion = '0.1.0'\nedition = '2026'\n\n[lib]\npath = 'src/lib.sg'\n",
            name
        ),
    )
    .unwrap();
}

fn write_bin_and_lib_pkg(root: &Path, name: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
    fs::write(
        root.join("src/lib.sg"),
        "def imported_value() -> i64 { 42 }\n",
    )
    .unwrap();
    fs::write(
        root.join("Sengoo.toml"),
        format!(
            "[package]\nname = '{}'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n\n[lib]\npath = 'src/lib.sg'\n",
            name
        ),
    )
    .unwrap();
}

fn git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_git_pkg(root: &Path, name: &str) {
    write_pkg(root, name, &[]);
    git(&["init"], root);
    git(&["add", "."], root);
    git(
        &[
            "-c",
            "user.name=sgpm test",
            "-c",
            "user.email=sgpm@example.invalid",
            "commit",
            "-m",
            "initial",
        ],
        root,
    );
}

fn git_head(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("resolve git head");
    assert!(
        output.status.success(),
        "git rev-parse failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgpm crate should live under tools/sgpm")
        .to_path_buf()
}

fn realworld_fixture(name: &str) -> PathBuf {
    workspace_root().join("examples/realworld").join(name)
}

fn copy_dir_filtered(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_name_text = file_name.to_string_lossy();
        if file_name_text == "target" || file_name_text == "build" {
            continue;
        }
        let destination_path = destination.join(file_name);
        if source_path.is_dir() {
            copy_dir_filtered(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

#[cfg(windows)]
fn fake_sgc(dir: &Path) -> PathBuf {
    let script = dir.join("sgc.cmd");
    fs::write(
        &script,
        "@echo off\r\necho %CD% :: %* :: modules=%SENGOO_MODULE_MAP% >> \"%SGPM_RECORD%\"\r\nexit /b 0\r\n",
    )
    .unwrap();
    script
}

#[cfg(windows)]
fn fake_sgfmt(dir: &Path) -> PathBuf {
    let script = dir.join("sgfmt.cmd");
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
        "#!/bin/sh\nprintf '%s :: %s :: modules=%s\\n' \"$PWD\" \"$*\" \"$SENGOO_MODULE_MAP\" >> \"$SGPM_RECORD\"\nexit 0\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

#[cfg(not(windows))]
fn fake_sgfmt(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let script = dir.join("sgfmt");
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
fn sgpm_new_lib_creates_library_layout() {
    let dir = temp_dir("new_lib");
    let project = dir.join("demo_lib");
    let output = run_sgpm(
        &[
            "new",
            "demo_lib",
            "--lib",
            "--path",
            project.to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("Sengoo.toml").exists());
    assert!(project.join("src/lib.sg").exists());
    assert!(!project.join("src/main.sg").exists());
    let manifest = fs::read_to_string(project.join("Sengoo.toml")).unwrap();
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("path = \"src/lib.sg\""));
    assert!(!manifest.contains("[bin]"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_init_initializes_existing_directory_with_derived_name() {
    let dir = temp_dir("init");
    let project = dir.join("demo_pkg");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("README.md"), "# existing project\n").unwrap();

    let output = run_sgpm(&["init"], &project);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("Sengoo.toml").exists());
    assert!(project.join("src/main.sg").exists());
    assert!(project.join("tests").exists());
    assert_eq!(
        fs::read_to_string(project.join("README.md")).unwrap(),
        "# existing project\n"
    );
    let manifest = fs::read_to_string(project.join("Sengoo.toml")).unwrap();
    assert!(manifest.contains("name = \"demo_pkg\""));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_init_lib_initializes_existing_directory_as_library() {
    let dir = temp_dir("init_lib");
    let project = dir.join("demo_lib");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("README.md"), "# existing library\n").unwrap();

    let output = run_sgpm(&["init", "--lib"], &project);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("src/lib.sg").exists());
    assert!(!project.join("src/main.sg").exists());
    let manifest = fs::read_to_string(project.join("Sengoo.toml")).unwrap();
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("name = \"demo_lib\""));
    assert_eq!(
        fs::read_to_string(project.join("README.md")).unwrap(),
        "# existing library\n"
    );
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
fn tree_resolves_local_git_dependency_into_cache() {
    let dir = temp_dir("tree_git_dep");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    let dep_pos = stdout.find("dep v0.1.0").expect("dep in tree");
    let app_pos = stdout.find("app v0.1.0").expect("app in tree");
    assert!(dep_pos < app_pos, "tree output:\n{}", stdout);
    assert!(
        stdout.contains("target/sgpm/git"),
        "git dependency should resolve through the root package cache:\n{}",
        stdout
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn git_dependency_repairs_incomplete_checkout_cache() {
    let dir = temp_dir("tree_git_dep_repair");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
    )
    .unwrap();

    let manifest = app.join("Sengoo.toml");
    let initial = run_sgpm(
        &["tree", "--manifest-path", manifest.to_str().unwrap()],
        &dir,
    );
    assert!(
        initial.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&initial.stdout),
        String::from_utf8_lossy(&initial.stderr)
    );

    let git_cache = app.join("target/sgpm/git");
    let checkout = fs::read_dir(&git_cache)
        .unwrap()
        .next()
        .expect("git checkout cache")
        .unwrap()
        .path();
    fs::remove_dir_all(checkout.join(".git")).unwrap();

    let repaired = run_sgpm(
        &["tree", "--manifest-path", manifest.to_str().unwrap()],
        &dir,
    );
    assert!(
        repaired.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&repaired.stdout),
        String::from_utf8_lossy(&repaired.stderr)
    );
    assert!(checkout.join(".git").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn registry_dependency_resolves_highest_matching_local_version() {
    let dir = temp_dir("registry_dep_highest");
    let registry = dir.join("registry");
    write_pkg_version(&registry.join("foo/1.0.0"), "foo", "1.0.0", &[]);
    write_pkg_version(&registry.join("foo/1.2.0"), "foo", "1.2.0", &[]);
    write_pkg_version(&registry.join("foo/2.0.0"), "foo", "2.0.0", &[]);
    fs::create_dir_all(registry.join("foo/.1.3.0.sgpm-publish-test")).unwrap();
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.local]\npath = '../registry'\n\n[dependencies]\nfoo = { version = '>=1.0.0, <2.0.0', registry = 'local' }\n",
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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    let foo_pos = stdout.find("foo v1.2.0").expect("foo in tree");
    let app_pos = stdout.find("app v0.1.0").expect("app in tree");
    assert!(foo_pos < app_pos, "tree output:\n{stdout}");
    assert!(
        stdout.contains("registry/foo/1.2.0"),
        "tree should point at selected registry package:\n{stdout}"
    );
    assert!(
        !stdout.contains("foo v2.0.0"),
        "tree should honor the semver upper bound:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn registry_dependency_fetches_highest_matching_remote_version() {
    let dir = temp_dir("registry_dep_remote");
    let remote_pkg = dir.join("remote/foo");
    write_pkg_version(&remote_pkg, "foo", "1.2.0", &[]);
    let package_output = run_sgpm(
        &[
            "publish",
            "--dry-run",
            "--manifest-path",
            remote_pkg.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        package_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&package_output.stdout),
        String::from_utf8_lossy(&package_output.stderr)
    );
    let archive_path = remote_pkg.join("target/package/foo-1.2.0.tar.gz");
    let archive = fs::read(&archive_path).unwrap();
    let checksum_text =
        fs::read_to_string(remote_pkg.join("target/package/foo-1.2.0.tar.gz.sha256")).unwrap();
    let checksum = checksum_text.split_whitespace().next().unwrap().to_string();
    let (server_url, paths_rx, handle) =
        spawn_remote_package_server("foo", "1.2.0", &checksum, archive);

    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        format!(
            "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.default]\nurl = '{}'\n\n[dependencies]\nfoo = '>=1.0.0, <2.0.0'\n",
            server_url
        ),
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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(stdout.contains("foo v1.2.0"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("target/sgpm/registry/default/foo/1.2.0"),
        "remote package should resolve through the registry cache:\n{stdout}"
    );
    let paths = paths_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("remote registry should receive index and download requests");
    handle.join().unwrap();
    assert_eq!(
        paths,
        vec![
            "/api/v1/packages/foo".to_string(),
            "/api/v1/packages/foo/1.2.0/download".to_string()
        ]
    );
    assert!(app
        .join("target/sgpm/registry/default/foo/1.2.0/Sengoo.toml")
        .exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn remote_registry_dependency_repairs_incomplete_package_cache() {
    let dir = temp_dir("registry_dep_remote_repair");
    let remote_pkg = dir.join("remote/foo");
    write_pkg_version(&remote_pkg, "foo", "1.2.0", &[]);
    let package_output = run_sgpm(
        &[
            "publish",
            "--dry-run",
            "--manifest-path",
            remote_pkg.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        package_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&package_output.stdout),
        String::from_utf8_lossy(&package_output.stderr)
    );
    let archive_path = remote_pkg.join("target/package/foo-1.2.0.tar.gz");
    let archive = fs::read(&archive_path).unwrap();
    let checksum_text =
        fs::read_to_string(remote_pkg.join("target/package/foo-1.2.0.tar.gz.sha256")).unwrap();
    let checksum = checksum_text.split_whitespace().next().unwrap().to_string();
    let (server_url, paths_rx, handle) =
        spawn_remote_package_server("foo", "1.2.0", &checksum, archive);

    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        format!(
            "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.default]\nurl = '{}'\n\n[dependencies]\nfoo = '>=1.0.0, <2.0.0'\n",
            server_url
        ),
    )
    .unwrap();
    let cached_package = app.join("target/sgpm/registry/default/foo/1.2.0");
    fs::create_dir_all(&cached_package).unwrap();
    fs::copy(
        remote_pkg.join("Sengoo.toml"),
        cached_package.join("Sengoo.toml"),
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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let paths = paths_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("remote registry should receive index and repair download requests");
    handle.join().unwrap();
    assert_eq!(
        paths,
        vec![
            "/api/v1/packages/foo".to_string(),
            "/api/v1/packages/foo/1.2.0/download".to_string()
        ]
    );
    assert!(cached_package.join("src/main.sg").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn metadata_json_reports_resolved_package_graph() {
    let dir = temp_dir("metadata_json");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let output = run_sgpm(
        &[
            "metadata",
            "--format",
            "json",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("metadata should be valid json");
    assert_eq!(json["root"], "app");
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["packages"][0]["name"], "dep");
    assert_eq!(json["packages"][1]["name"], "app");
    assert_eq!(json["dependencies"][0]["alias"], "dep");
    assert!(
        json["packages"][0]["id"]
            .as_str()
            .unwrap_or_default()
            .contains("dep@"),
        "package identity should include id:\n{json:#}"
    );
    assert!(
        json["packages"][0]["manifest"]
            .as_str()
            .unwrap_or_default()
            .ends_with("dep/Sengoo.toml"),
        "dependency package should include manifest path:\n{json:#}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn metadata_json_reports_all_workspace_members() {
    let dir = temp_dir("metadata_workspace_json");
    let workspace = dir.join("workspace");
    write_pkg(&workspace.join("packages/app"), "app", &[]);
    write_pkg(&workspace.join("packages/cli"), "cli", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "metadata",
            "--workspace",
            "--format",
            "json",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("metadata should be valid json");
    assert_eq!(json["workspace"], true);
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["roots"][0], "app");
    assert_eq!(json["roots"][1], "cli");
    assert_eq!(json["packages"][0]["name"], "app");
    assert_eq!(json["packages"][1]["name"], "cli");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_tree_resolves_named_member_with_inherited_registry() {
    let dir = temp_dir("workspace_registry");
    let registry = dir.join("registry");
    write_pkg_version(&registry.join("foo/1.0.0"), "foo", "1.0.0", &[]);
    write_pkg_version(&registry.join("foo/1.2.0"), "foo", "1.2.0", &[]);

    let workspace = dir.join("workspace");
    let app = workspace.join("packages/app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\nfoo = { version = '>=1.0.0, <2.0.0', registry = 'local' }\n",
    )
    .unwrap();
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n\n[registries.local]\npath = '../registry'\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
            "--package",
            "app",
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    let foo_pos = stdout.find("foo v1.2.0").expect("foo in tree");
    let app_pos = stdout.find("app v0.1.0").expect("app in tree");
    assert!(foo_pos < app_pos, "tree output:\n{stdout}");
    assert!(
        stdout.contains("registry/foo/1.2.0"),
        "workspace registry should be inherited by the selected member:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_manifest_requires_package_when_multiple_members() {
    let dir = temp_dir("workspace_multiple_members");
    let workspace = dir.join("workspace");
    write_pkg(&workspace.join("packages/app"), "app", &[]);
    write_pkg(&workspace.join("packages/cli"), "cli", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("workspace"), "stderr:\n{stderr}");
    assert!(stderr.contains("--package"), "stderr:\n{stderr}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_rejects_duplicate_member_package_names() {
    let dir = temp_dir("workspace_duplicate_member_names");
    let workspace = dir.join("workspace");
    write_pkg(&workspace.join("packages/one"), "duplicate", &[]);
    write_pkg(&workspace.join("packages/two"), "duplicate", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--workspace",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("workspace has multiple member packages named 'duplicate'"),
        "stderr:\n{stderr}"
    );
    assert!(!workspace.join("Sengoo.lock").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_check_runs_all_members_with_workspace_flag() {
    let dir = temp_dir("workspace_check_all");
    let workspace = dir.join("workspace");
    write_pkg(&workspace.join("packages/app"), "app", &[]);
    write_pkg(&workspace.join("packages/cli"), "cli", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "check",
            "--workspace",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    let app_pos = log.find("/packages/app :: check").expect("app check");
    let cli_pos = log.find("/packages/cli :: check").expect("cli check");
    assert!(app_pos < cli_pos, "workspace check log:\n{log}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_update_writes_single_lockfile_for_all_members() {
    let dir = temp_dir("workspace_update_all");
    let workspace = dir.join("workspace");
    let app = workspace.join("packages/app");
    let cli = workspace.join("packages/cli");
    write_pkg(&app, "app", &[]);
    write_pkg(&cli, "cli", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--workspace",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!app.join("Sengoo.lock").exists());
    assert!(!cli.join("Sengoo.lock").exists());
    let lockfile_path = workspace.join("Sengoo.lock");
    assert!(
        lockfile_path.exists(),
        "missing {}",
        lockfile_path.display()
    );
    let lockfile = fs::read_to_string(lockfile_path)
        .unwrap()
        .replace('\\', "/");
    assert!(lockfile.contains("workspace = true"), "{lockfile}");
    assert!(
        lockfile.contains("members = [\"app\", \"cli\"]"),
        "{lockfile}"
    );
    assert!(
        lockfile.contains("manifest = \"packages/app/Sengoo.toml\""),
        "{lockfile}"
    );
    assert!(
        lockfile.contains("manifest = \"packages/cli/Sengoo.toml\""),
        "{lockfile}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_update_check_uses_root_lockfile() {
    let dir = temp_dir("workspace_update_check");
    let workspace = dir.join("workspace");
    let app = workspace.join("packages/app");
    let cli = workspace.join("packages/cli");
    write_pkg(&app, "app", &[]);
    write_pkg(&cli, "cli", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let update = run_sgpm(
        &[
            "update",
            "--workspace",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        update.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&update.stdout),
        String::from_utf8_lossy(&update.stderr)
    );

    let check = run_sgpm(
        &[
            "update",
            "--workspace",
            "--check",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        check.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let stdout = String::from_utf8_lossy(&check.stdout).replace('\\', "/");
    assert!(
        stdout.contains("workspace/Sengoo.lock"),
        "workspace check should report root lockfile:\n{stdout}"
    );

    fs::write(workspace.join("Sengoo.lock"), "stale workspace lockfile\n").unwrap();
    let stale = run_sgpm(
        &[
            "update",
            "--workspace",
            "--check",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(!stale.status.success());
    let stderr = String::from_utf8_lossy(&stale.stderr);
    assert!(
        stderr.contains("sgpm update --workspace"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_flag_rejects_package_manifest() {
    let dir = temp_dir("workspace_flag_package_manifest");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);

    let output = run_sgpm(
        &[
            "tree",
            "--workspace",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--workspace requires a workspace manifest"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_flag_rejects_package_selection() {
    let dir = temp_dir("workspace_flag_with_package");
    let workspace = dir.join("workspace");
    write_pkg(&workspace.join("packages/app"), "app", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "tree",
            "--workspace",
            "--package",
            "app",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--workspace cannot be combined with --package"),
        "stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_writes_lockfile_for_local_registry_dependency() {
    let dir = temp_dir("registry_lockfile");
    let registry = dir.join("registry");
    write_pkg_version(&registry.join("foo/1.0.0"), "foo", "1.0.0", &[]);
    write_pkg_version(&registry.join("foo/1.2.0"), "foo", "1.2.0", &[]);
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.local]\npath = '../registry'\n\n[dependencies]\nfoo = { version = '>=1.0.0, <2.0.0', registry = 'local' }\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lockfile = fs::read_to_string(app.join("Sengoo.lock"))
        .unwrap()
        .replace('\\', "/");
    assert!(
        lockfile.contains("version = \"2\"") || lockfile.contains("version = 2"),
        "{lockfile}"
    );
    assert!(lockfile.contains("name = \"foo\""), "{lockfile}");
    assert!(lockfile.contains("version = \"1.2.0\""), "{lockfile}");
    assert!(
        lockfile.contains("source.kind = \"registry\""),
        "lockfile should record structured registry source:\n{lockfile}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn registry_dependency_multiversion_keeps_both_selected_versions() {
    let dir = temp_dir("registry_multiversion");
    let registry = dir.join("registry");
    write_pkg_version(&registry.join("foo/1.5.0"), "foo", "1.5.0", &[]);
    write_pkg_version(&registry.join("foo/2.1.0"), "foo", "2.1.0", &[]);
    let a = dir.join("a");
    let b = dir.join("b");
    let app = dir.join("app");
    write_pkg(&a, "a", &[]);
    fs::write(
        a.join("Sengoo.toml"),
        "[package]\nname = 'a'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\nfoo = { version = '>=1.0.0, <2.0.0', registry = 'local' }\n",
    )
    .unwrap();
    write_pkg(&b, "b", &[]);
    fs::write(
        b.join("Sengoo.toml"),
        "[package]\nname = 'b'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\nfoo = { version = '>=2.0.0, <3.0.0', registry = 'local' }\n",
    )
    .unwrap();
    write_pkg(&app, "app", &[("a", "../a"), ("b", "../b")]);
    let mut root_manifest = fs::read_to_string(app.join("Sengoo.toml")).unwrap();
    root_manifest.push_str("\n[registries.local]\npath = '../registry'\n");
    fs::write(app.join("Sengoo.toml"), root_manifest).unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lockfile = fs::read_to_string(app.join("Sengoo.lock"))
        .unwrap()
        .replace('\\', "/");
    assert!(lockfile.contains("version = 2"), "{lockfile}");
    assert!(lockfile.contains("name = \"foo\""), "{lockfile}");
    assert!(lockfile.contains("version = \"1.5.0\""), "{lockfile}");
    assert!(lockfile.contains("version = \"2.1.0\""), "{lockfile}");

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
fn sgpm_build_checks_library_dependency_before_building_app() {
    let dir = temp_dir("build_library_dep");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_lib_pkg(&dep, "dep");
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
    let dep_check = log.find("/dep :: check").expect("dep library check");
    let app_build = log.find("/app :: build").expect("app build");
    assert!(dep_check < app_build, "build log:\n{log}");
    assert!(
        log[app_build..].contains("modules=dep="),
        "app build should receive dependency module map:\n{log}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_doc_invokes_sgc_doc_for_library_entry() {
    let dir = temp_dir("doc_library_entry");
    let app = dir.join("app");
    write_bin_and_lib_pkg(&app, "app");

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "doc",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm doc");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    assert!(log.contains("/app :: doc"), "doc log:\n{log}");
    assert!(
        log.contains("/src/lib.sg --output"),
        "doc should prefer the package library entry:\n{log}"
    );
    assert!(log.contains("/target/doc"), "doc output log:\n{log}");

    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(
        stdout.contains("documented app ->") && stdout.contains("/target/doc"),
        "stdout:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_check_exposes_dependency_library_module_map() {
    let dir = temp_dir("check_module_map");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_lib_pkg(&dep, "dep");
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "check",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    let entry = dep.join("src/lib.sg").to_string_lossy().replace('\\', "/");
    assert!(
        log.contains(&format!("modules=dep={entry}")),
        "dependency library should be exposed through SENGOO_MODULE_MAP:\n{log}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_check_maps_dual_target_dependency_to_library_entry() {
    let dir = temp_dir("check_dual_target_module_map");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_bin_and_lib_pkg(&dep, "dep");
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "check",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm check");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    let entry = dep.join("src/lib.sg").to_string_lossy().replace('\\', "/");
    assert!(
        log.contains(&format!("modules=dep={entry}")),
        "dual-target dependency should expose its library entry:\n{log}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_package_commands_expose_selected_package_own_library_module() {
    let dir = temp_dir("check_own_lib_module_map");
    let app = dir.join("app");
    write_bin_and_lib_pkg(&app, "app");

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let check_output = Command::new(sgpm())
        .args([
            "check",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", &fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm check");

    assert!(
        check_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );
    let log = fs::read_to_string(&record).unwrap().replace('\\', "/");
    let entry = app.join("src/lib.sg").to_string_lossy().replace('\\', "/");
    assert!(
        log.contains(&format!("modules=app={entry}")),
        "selected package check should expose its own library entry:\n{log}"
    );

    let build_output = Command::new(sgpm())
        .args([
            "build",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", &fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm build");

    assert!(
        build_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    assert!(
        log.contains(&format!("modules=app={entry}")),
        "selected package build should expose its own library entry:\n{log}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_run_rejects_library_package_with_actionable_diagnostic() {
    let dir = temp_dir("run_library");
    let lib = dir.join("libpkg");
    write_lib_pkg(&lib, "libpkg");

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "run",
            "--manifest-path",
            lib.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot run library package 'libpkg'") && stderr.contains("add [bin]"),
        "stderr:\n{stderr}"
    );
    assert!(
        !record.exists(),
        "library run should fail before invoking delegated tools"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_test_release_invokes_sgc_run_with_o2() {
    let dir = temp_dir("test_release");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::create_dir_all(app.join("tests")).unwrap();
    fs::write(app.join("tests/basic.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "test",
            "--release",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm test");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    assert!(
        log.contains(":: test") && log.contains("--release"),
        "test log should delegate to sgc test with release profile:\n{}",
        log
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_test_debug_invokes_sgc_run_with_o0() {
    let dir = temp_dir("test_debug");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::create_dir_all(app.join("tests")).unwrap();
    fs::write(app.join("tests/basic.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "test",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm test");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    assert!(
        log.contains(":: test") && log.contains("Sengoo.toml"),
        "test log should delegate to sgc test with manifest path:\n{}",
        log
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_test_exposes_library_package_to_its_tests() {
    let dir = temp_dir("test_library_module_map");
    let lib = dir.join("libpkg");
    write_lib_pkg(&lib, "libpkg");
    fs::create_dir_all(lib.join("tests")).unwrap();
    fs::write(
        lib.join("tests/public_api.sg"),
        "import libpkg;\ndef main() -> i64 { imported_value() }\n",
    )
    .unwrap();

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "test",
            "--manifest-path",
            lib.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm test");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    let entry = lib.join("src/lib.sg").to_string_lossy().replace('\\', "/");
    assert!(
        log.contains(&format!("modules=libpkg={entry}")),
        "library tests should receive their own public module mapping:\n{log}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sgpm_fmt_formats_src_and_tests_files() {
    let dir = temp_dir("fmt_src_and_tests");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::create_dir_all(app.join("tests")).unwrap();
    fs::write(app.join("tests/basic.sg"), "def main() -> i64 { 0 }\n").unwrap();

    let record = dir.join("record.txt");
    let fake_sgc = fake_sgc(&dir);
    let fake_sgfmt = fake_sgfmt(&dir);
    let output = Command::new(sgpm())
        .args([
            "fmt",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake_sgc)
        .env("SGPM_SGFMT", fake_sgfmt)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm fmt");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(record).unwrap().replace('\\', "/");
    assert!(
        log.contains("/src/main.sg --write"),
        "fmt should format package sources:\n{log}"
    );
    assert!(
        log.contains("/tests/basic.sg --write"),
        "fmt should format package tests:\n{log}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("formatted 2 Sengoo source file(s)"),
        "stdout should count src and tests files:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejects_version_dep_without_default_registry() {
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
        stderr.contains("requires registry 'default'")
            && stderr.contains("[registries.default] is configured"),
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

#[test]
fn publish_dry_run_creates_archive_and_checksum() {
    let dir = temp_dir("publish_dry_run");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::create_dir_all(app.join("tests")).unwrap();
    fs::create_dir_all(app.join("target/debug")).unwrap();
    fs::write(app.join("tests/basic.sg"), "def main() -> i64 { 0 }\n").unwrap();
    fs::write(app.join("target/debug/app"), "build output").unwrap();

    let output = run_sgpm(
        &[
            "publish",
            "--dry-run",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let archive_path = app.join("target/package/app-0.1.0.tar.gz");
    let checksum_path = app.join("target/package/app-0.1.0.tar.gz.sha256");
    assert!(archive_path.exists(), "missing {}", archive_path.display());
    assert!(
        checksum_path.exists(),
        "missing {}",
        checksum_path.display()
    );

    let checksum = fs::read_to_string(checksum_path).unwrap();
    assert!(
        checksum.contains("app-0.1.0.tar.gz"),
        "checksum should name archive:\n{}",
        checksum
    );
    assert_eq!(
        checksum.split_whitespace().next().unwrap_or("").len(),
        64,
        "checksum should start with sha256 hex:\n{}",
        checksum
    );

    let archive_file = fs::File::open(archive_path).unwrap();
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    let mut entries = archive
        .entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    entries.sort();
    assert!(entries.contains(&"Sengoo.toml".to_string()), "{entries:?}");
    assert!(entries.contains(&"src/main.sg".to_string()), "{entries:?}");
    assert!(
        entries.contains(&"tests/basic.sg".to_string()),
        "{entries:?}"
    );
    assert!(
        entries.iter().all(|entry| !entry.starts_with("target/")),
        "archive must not include build artifacts: {entries:?}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_dry_run_rejects_missing_package_entry() {
    let dir = temp_dir("publish_missing_entry");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::remove_file(app.join("src/main.sg")).unwrap();

    let output = run_sgpm(
        &[
            "publish",
            "--dry-run",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("package 'app' [bin] entry does not exist"),
        "stderr:\n{stderr}"
    );
    assert!(
        !app.join("target/package/app-0.1.0.tar.gz").exists(),
        "broken package should fail before creating an archive"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_dry_run_custom_output_does_not_package_itself() {
    let dir = temp_dir("publish_custom_output");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);

    let output = run_sgpm(
        &[
            "publish",
            "--dry-run",
            "--output",
            "dist",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let archive_path = app.join("dist/app-0.1.0.tar.gz");
    assert!(archive_path.exists(), "missing {}", archive_path.display());

    let archive_file = fs::File::open(archive_path).unwrap();
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    assert!(
        entries.iter().all(|entry| !entry.starts_with("dist/")),
        "archive must not include generated package artifacts: {entries:?}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_to_local_registry_writes_resolvable_package() {
    let dir = temp_dir("publish_local_registry");
    let registry = dir.join("registry");
    let libpkg = dir.join("libpkg");
    write_pkg_version(&libpkg, "libpkg", "0.2.0", &[]);
    fs::create_dir_all(libpkg.join("target/debug")).unwrap();
    fs::write(libpkg.join("target/debug/libpkg"), "build output").unwrap();
    let mut manifest = fs::read_to_string(libpkg.join("Sengoo.toml")).unwrap();
    manifest.push_str("\n[registries.local]\npath = '../registry'\n");
    fs::write(libpkg.join("Sengoo.toml"), manifest).unwrap();

    let output = run_sgpm(
        &[
            "publish",
            "--registry",
            "local",
            "--manifest-path",
            libpkg.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(
        stdout.contains("published libpkg v0.2.0 to local registry"),
        "stdout:\n{stdout}"
    );
    assert!(registry.join("libpkg/0.2.0/Sengoo.toml").exists());
    assert!(registry.join("libpkg/0.2.0/src/main.sg").exists());
    assert!(
        !registry.join("libpkg/0.2.0/target").exists(),
        "published package should not contain build artifacts"
    );

    let consumer = dir.join("consumer");
    write_pkg(&consumer, "consumer", &[]);
    fs::write(
        consumer.join("Sengoo.toml"),
        "[package]\nname = 'consumer'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.local]\npath = '../registry'\n\n[dependencies]\nlibpkg = { version = '0.2.0', registry = 'local' }\n",
    )
    .unwrap();
    let output = run_sgpm(
        &[
            "tree",
            "--manifest-path",
            consumer.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(stdout.contains("libpkg v0.2.0"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("registry/libpkg/0.2.0"),
        "tree should resolve through the published registry package:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_to_local_registry_rejects_unsafe_package_name() {
    let dir = temp_dir("publish_local_registry_unsafe_name");
    let registry = dir.join("registry");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = '../escape'\nversion = '0.1.0'\nedition = '2026'\n\n[registries.local]\npath = '../registry'\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "publish",
            "--registry",
            "local",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("package name '../escape'"),
        "stderr:\n{stderr}"
    );
    assert!(!registry.exists());
    assert!(!dir.join("escape").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_to_local_registry_refuses_existing_version() {
    let dir = temp_dir("publish_local_registry_exists");
    let libpkg = dir.join("libpkg");
    write_pkg_version(&libpkg, "libpkg", "0.2.0", &[]);
    let mut manifest = fs::read_to_string(libpkg.join("Sengoo.toml")).unwrap();
    manifest.push_str("\n[registries.local]\npath = '../registry'\n");
    fs::write(libpkg.join("Sengoo.toml"), manifest).unwrap();

    let first = run_sgpm(
        &[
            "publish",
            "--registry",
            "local",
            "--manifest-path",
            libpkg.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        first.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );

    let second = run_sgpm(
        &[
            "publish",
            "--registry",
            "local",
            "--manifest-path",
            libpkg.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("already exists"), "stderr:\n{stderr}");
    assert!(stderr.contains("libpkg"), "stderr:\n{stderr}");
    assert!(stderr.contains("0.2.0"), "stderr:\n{stderr}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn publish_to_default_remote_registry_uploads_package_archive() {
    let dir = temp_dir("publish_remote_registry");
    let app = dir.join("remotepkg");
    write_pkg_version(&app, "remotepkg", "0.3.0", &[]);
    let (server_url, rx, handle) = spawn_one_request_server();
    let mut manifest = fs::read_to_string(app.join("Sengoo.toml")).unwrap();
    manifest.push_str(&format!(
        "\n[registries.default]\nurl = '{}'\ntoken_env = 'SGPM_TEST_REMOTE_TOKEN'\n",
        server_url
    ));
    fs::write(app.join("Sengoo.toml"), manifest).unwrap();

    let output = Command::new(sgpm())
        .args([
            "publish",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_TEST_REMOTE_TOKEN", "secret-token")
        .output()
        .expect("run sgpm publish");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let request = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("remote registry should receive publish request");
    handle.join().unwrap();

    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/v1/packages/remotepkg/0.3.0");
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| { name == "authorization" && value == "Bearer secret-token" }),
        "headers: {:?}",
        request.headers
    );
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| { name == "x-sengoo-checksum" && value.len() == 64 }),
        "headers: {:?}",
        request.headers
    );
    assert!(request.body.starts_with(&[0x1f, 0x8b]));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("published remotepkg v0.3.0 to remote registry"),
        "stdout:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn workspace_publish_uses_inherited_registry() {
    let dir = temp_dir("workspace_publish_registry");
    let workspace = dir.join("workspace");
    let app = workspace.join("packages/app");
    write_pkg_version(&app, "app", "0.2.0", &[]);
    fs::write(
        workspace.join("Sengoo.toml"),
        "[workspace]\nmembers = ['packages/*']\n\n[registries.local]\npath = '../registry'\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "publish",
            "--registry",
            "local",
            "--manifest-path",
            workspace.join("Sengoo.toml").to_str().unwrap(),
            "--package",
            "app",
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(
        stdout.contains("published app v0.2.0 to local registry"),
        "stdout:\n{stdout}"
    );
    assert!(dir.join("registry/app/0.2.0/Sengoo.toml").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_writes_lockfile_for_path_dependency_graph() {
    let dir = temp_dir("update_lockfile");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let lockfile = app.join("Sengoo.lock");
    assert!(lockfile.exists(), "missing {}", lockfile.display());
    let text = fs::read_to_string(lockfile).unwrap().replace('\\', "/");
    assert!(text.contains("version = 2"), "{text}");
    assert!(text.contains("root = \"app\""), "{text}");
    assert!(text.contains("name = \"dep\""), "{text}");
    assert!(text.contains("source.kind = \"path\""), "{text}");
    assert!(text.contains("source.path = \"../dep\""), "{text}");
    assert!(text.contains("manifest = \"../dep/Sengoo.toml\""), "{text}");
    assert!(text.contains("name = \"app\""), "{text}");
    assert!(text.contains("source.path = \".\""), "{text}");
    assert!(text.contains("[[dependency]]"), "{text}");
    assert!(text.contains("alias = \"dep\""), "{text}");

    let dep_pos = text.find("name = \"dep\"").expect("dep package");
    let app_pos = text.find("name = \"app\"").expect("app package");
    assert!(
        dep_pos < app_pos,
        "lockfile should preserve dependency-first graph order:\n{text}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_writes_lockfile_for_git_dependency_with_resolved_rev() {
    let dir = temp_dir("update_git_lockfile");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    let dep_url = dep_repo.to_string_lossy().replace('\\', "/");
    fs::write(
        app.join("Sengoo.toml"),
        format!(
            "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = {{ git = '{}' }}\n",
            dep_url
        ),
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&dep_repo)
        .output()
        .expect("resolve dep git commit");
    assert!(commit.status.success());
    let commit = String::from_utf8_lossy(&commit.stdout).trim().to_string();

    let lockfile = fs::read_to_string(app.join("Sengoo.lock"))
        .unwrap()
        .replace('\\', "/");
    assert!(
        lockfile.contains("source.kind = \"git\""),
        "lockfile should record git source:\n{}",
        lockfile
    );
    assert!(
        lockfile.contains(&format!("source.url = \"{}\"", dep_url)),
        "lockfile should record git url:\n{}",
        lockfile
    );
    assert!(
        lockfile.contains(&format!("source.rev = \"{}\"", commit)),
        "lockfile should record resolved commit:\n{}",
        lockfile
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_refresh_reclones_git_dependency_cache() {
    let dir = temp_dir("update_git_refresh");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let initial_commit = git_head(&dep_repo);
    let initial_lockfile = fs::read_to_string(app.join("Sengoo.lock")).unwrap();
    assert!(
        initial_lockfile.contains(&initial_commit),
        "initial lockfile should record first git commit:\n{}",
        initial_lockfile
    );

    fs::write(dep_repo.join("src/main.sg"), "def main() -> i64 { 1 }\n").unwrap();
    git(&["add", "."], &dep_repo);
    git(
        &[
            "-c",
            "user.name=sgpm test",
            "-c",
            "user.email=sgpm@example.invalid",
            "commit",
            "-m",
            "second",
        ],
        &dep_repo,
    );
    let updated_commit = git_head(&dep_repo);
    assert_ne!(initial_commit, updated_commit);

    let output = run_sgpm(
        &[
            "update",
            "--refresh",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let refreshed_lockfile = fs::read_to_string(app.join("Sengoo.lock")).unwrap();
    assert!(
        refreshed_lockfile.contains(&updated_commit),
        "refreshed lockfile should record latest git commit:\n{}",
        refreshed_lockfile
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_refresh_preserves_valid_git_cache_when_new_checkout_is_broken() {
    let dir = temp_dir("update_git_refresh_broken");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
    )
    .unwrap();
    let manifest = app.join("Sengoo.toml");

    let initial = run_sgpm(
        &["tree", "--manifest-path", manifest.to_str().unwrap()],
        &dir,
    );
    assert!(
        initial.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&initial.stdout),
        String::from_utf8_lossy(&initial.stderr)
    );

    fs::remove_file(dep_repo.join("src/main.sg")).unwrap();
    git(&["add", "-A"], &dep_repo);
    git(
        &[
            "-c",
            "user.name=sgpm test",
            "-c",
            "user.email=sgpm@example.invalid",
            "commit",
            "-m",
            "broken",
        ],
        &dep_repo,
    );

    let refresh = run_sgpm(
        &[
            "update",
            "--refresh",
            "--manifest-path",
            manifest.to_str().unwrap(),
        ],
        &dir,
    );
    assert!(!refresh.status.success());

    let cached = run_sgpm(
        &["tree", "--manifest-path", manifest.to_str().unwrap()],
        &dir,
    );
    assert!(
        cached.status.success(),
        "failed refresh should preserve the previous valid cache\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&cached.stdout),
        String::from_utf8_lossy(&cached.stderr)
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_list_prints_git_dependency_checkouts() {
    let dir = temp_dir("cache_list_git");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
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
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_sgpm(
        &[
            "cache",
            "list",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(stdout.contains("git dep-"), "stdout:\n{stdout}");
    assert!(
        stdout.contains("target/sgpm/git"),
        "cache list should print the git cache path:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_clean_git_removes_git_dependency_checkouts() {
    let dir = temp_dir("cache_clean_git");
    let dep_repo = dir.join("dep_repo");
    let app = dir.join("app");
    write_git_pkg(&dep_repo, "dep");
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[dependencies]\ndep = { git = '../dep_repo' }\n",
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
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let git_cache = app.join("target/sgpm/git");
    assert!(git_cache.exists(), "missing {}", git_cache.display());

    let output = run_sgpm(
        &[
            "cache",
            "clean",
            "--git",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !git_cache.exists(),
        "{} should be removed",
        git_cache.display()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("removed git cache"), "stdout:\n{stdout}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_list_prints_remote_registry_packages() {
    let dir = temp_dir("cache_list_registry");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    let registry_cache = app.join("target/sgpm/registry/default/foo/1.2.0");
    fs::create_dir_all(&registry_cache).unwrap();

    let output = run_sgpm(
        &[
            "cache",
            "list",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace('\\', "/");
    assert!(
        stdout.contains("registry default/foo/1.2.0"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("target/sgpm/registry/default/foo/1.2.0"),
        "cache list should print the registry cache path:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cache_clean_registry_removes_downloaded_packages() {
    let dir = temp_dir("cache_clean_registry");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    let registry_cache = app.join("target/sgpm/registry");
    fs::create_dir_all(registry_cache.join("default/foo/1.2.0")).unwrap();

    let output = run_sgpm(
        &[
            "cache",
            "clean",
            "--registry",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !registry_cache.exists(),
        "{} should be removed",
        registry_cache.display()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("removed registry cache"),
        "stdout:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_check_reports_stale_lockfile_without_writing() {
    let dir = temp_dir("update_check_stale");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);
    fs::write(app.join("Sengoo.lock"), "stale lockfile\n").unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--check",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Sengoo.lock is out of date") && stderr.contains("sgpm update"),
        "stderr:\n{}",
        stderr
    );
    let lockfile = fs::read_to_string(app.join("Sengoo.lock")).unwrap();
    assert_eq!(lockfile, "stale lockfile\n");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_check_reports_missing_lockfile() {
    let dir = temp_dir("update_check_missing");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);

    let output = run_sgpm(
        &[
            "update",
            "--check",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Sengoo.lock is out of date") && stderr.contains("sgpm update"),
        "stderr:\n{}",
        stderr
    );
    assert!(!app.join("Sengoo.lock").exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn realworld_locked_project_loop_keeps_lockfiles_current() {
    let dir = temp_dir("realworld_locked_loop");
    let record = dir.join("record.txt");
    let fake_sgc = fake_sgc(&dir);
    let fake_sgfmt = fake_sgfmt(&dir);

    for fixture in ["cli-json-audit", "http-client-status", "workspace-doc-loop"] {
        let package = dir.join(fixture);
        copy_dir_filtered(&realworld_fixture(fixture), &package);

        let update = run_sgpm(&["update"], &package);
        assert!(
            update.status.success(),
            "{} update stdout:\n{}\nstderr:\n{}",
            fixture,
            String::from_utf8_lossy(&update.stdout),
            String::from_utf8_lossy(&update.stderr)
        );

        let lock_path = package.join("Sengoo.lock");
        let before = fs::read_to_string(&lock_path).expect("lockfile should exist after update");

        for args in [
            vec!["check", "--locked"],
            vec!["test", "--locked"],
            vec!["fmt", "--check", "--locked"],
            vec!["doc", "--locked"],
            vec!["build", "--locked"],
        ] {
            let output = Command::new(sgpm())
                .args(&args)
                .current_dir(&package)
                .env("SGPM_SGC", &fake_sgc)
                .env("SGPM_SGFMT", &fake_sgfmt)
                .env("SGPM_RECORD", &record)
                .output()
                .expect("run sgpm locked command");
            assert!(
                output.status.success(),
                "{} sgpm {} stdout:\n{}\nstderr:\n{}",
                fixture,
                args.join(" "),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let after = fs::read_to_string(&lock_path).expect("lockfile should remain readable");
            assert_eq!(
                after,
                before,
                "{} sgpm {} should not rewrite Sengoo.lock",
                fixture,
                args.join(" ")
            );
        }
    }

    let log = fs::read_to_string(&record)
        .unwrap_or_default()
        .replace('\\', "/");
    assert!(
        log.contains("src/main.sg"),
        "expected realworld src delegation:\n{log}"
    );
    assert!(
        log.contains("tests/"),
        "expected realworld test delegation:\n{log}"
    );
    assert!(
        log.contains("--check"),
        "expected fmt --check delegation:\n{log}"
    );
    assert!(log.contains("doc"), "expected doc delegation:\n{log}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn build_locked_rejects_stale_lockfile_before_invoking_sgc() {
    let dir = temp_dir("build_locked_stale");
    let app = dir.join("app");
    write_pkg(&app, "app", &[]);
    fs::write(app.join("Sengoo.lock"), "stale lockfile\n").unwrap();

    let record = dir.join("record.txt");
    let fake = fake_sgc(&dir);
    let output = Command::new(sgpm())
        .args([
            "build",
            "--locked",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ])
        .current_dir(&dir)
        .env("SGPM_SGC", fake)
        .env("SGPM_RECORD", &record)
        .output()
        .expect("run sgpm build");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Sengoo.lock is out of date") && stderr.contains("sgpm update"),
        "stderr:\n{}",
        stderr
    );
    assert!(
        !record.exists(),
        "locked build should fail before invoking sgc"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn alias_update_writes_lockfile_v2_with_dependency_edge() {
    let dir = temp_dir("alias_lockfile");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "actual_name", &[]);
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n\n[dependencies]\nmy_alias = { package = 'actual_name', path = '../dep' }\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let lockfile = fs::read_to_string(app.join("Sengoo.lock"))
        .unwrap()
        .replace('\\', "/");
    assert!(lockfile.contains("version = 2"), "{lockfile}");
    assert!(lockfile.contains("alias = \"my_alias\""), "{lockfile}");
    assert!(lockfile.contains("name = \"actual_name\""), "{lockfile}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn update_migrates_compatible_v1_lockfile_to_v2() {
    let dir = temp_dir("lockfile_v1_migration");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "dep", &[]);
    write_pkg(&app, "app", &[("dep", "../dep")]);
    fs::write(
        app.join("Sengoo.lock"),
        "version = 1\nroot = \"app\"\n\n[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\nsource = \"path+../dep\"\nmanifest = \"../dep/Sengoo.toml\"\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\nsource = \"path+.\"\nmanifest = \"Sengoo.toml\"\ndependencies = [\"dep\"]\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "update",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lockfile = fs::read_to_string(app.join("Sengoo.lock"))
        .unwrap()
        .replace('\\', "/");
    assert!(lockfile.contains("version = 2"), "{lockfile}");
    assert!(lockfile.contains("[[dependency]]"), "{lockfile}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn lockfile_incompatible_v1_graph_fails_locked_check_with_update_hint() {
    let dir = temp_dir("lockfile_incompatible_v1");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "actual_name", &[]);
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n\n[dependencies]\nmy_alias = { package = 'actual_name', path = '../dep' }\n",
    )
    .unwrap();
    fs::write(app.join("Sengoo.lock"), "version = 1\nroot = \"app\"\n").unwrap();

    let output = run_sgpm(
        &[
            "check",
            "--locked",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot represent the current dependency graph"),
        "stderr:\n{stderr}"
    );
    assert!(stderr.contains("sgpm update"), "stderr:\n{stderr}");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn metadata_json_alias_lists_dependency_edges_separately() {
    let dir = temp_dir("metadata_alias_json");
    let dep = dir.join("dep");
    let app = dir.join("app");
    write_pkg(&dep, "actual_name", &[]);
    write_pkg(&app, "app", &[]);
    fs::write(
        app.join("Sengoo.toml"),
        "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n\n[dependencies]\nmy_alias = { package = 'actual_name', path = '../dep' }\n",
    )
    .unwrap();

    let output = run_sgpm(
        &[
            "metadata",
            "--format",
            "json",
            "--manifest-path",
            app.join("Sengoo.toml").to_str().unwrap(),
        ],
        &dir,
    );
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("metadata should be valid json");
    assert_eq!(json["dependencies"][0]["alias"], "my_alias");
    assert!(
        json["dependencies"][0]["to"]
            .as_str()
            .unwrap_or_default()
            .contains("actual_name@"),
        "dependency edge should use lockfile identity:\n{json:#}"
    );

    let _ = fs::remove_dir_all(dir);
}
