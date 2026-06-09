use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) const MODULE_MAP_ENV: &str = "SENGOO_MODULE_MAP";
pub(crate) const ASSERT_REPORT_ENV: &str = "SENGOO_ASSERT_REPORT";
const MAX_ASSERT_ENVELOPE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum TestOutputFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
struct TestReportJson<'a> {
    schema_version: u32,
    exit_status: i32,
    capture: &'static str,
    passed: usize,
    failed: usize,
    total: usize,
    tests: Vec<TestCaseJson<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AssertionEnvelope {
    pub schema_version: u32,
    pub kind: String,
    pub helper: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AssertionEnvelopeRead {
    Valid(AssertionEnvelope),
    Missing,
}

#[derive(Debug, Serialize)]
struct TestCaseJson<'a> {
    name: &'a str,
    path: String,
    ok: bool,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assertion: Option<AssertionEnvelope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assertion_transport: Option<String>,
}

struct TestCase {
    path: PathBuf,
    name: String,
}

pub(crate) struct TestOptions<'a> {
    pub root: &'a Path,
    pub filter: Option<&'a str>,
    pub exact: Option<&'a str>,
    pub format: TestOutputFormat,
    pub nocapture: bool,
    pub release: bool,
    pub locked: bool,
    pub manifest_path: Option<&'a Path>,
}

pub(crate) fn cmd_test(options: TestOptions<'_>) -> Result<()> {
    if options.locked {
        ensure_lockfile_current(options.root, options.manifest_path)?;
    }

    let mut tests = discover_tests(options.root)?;
    tests.sort_by(|a, b| a.path.cmp(&b.path));

    if let Some(exact) = options.exact {
        tests.retain(|test| test.name == exact);
    }
    if let Some(filter) = options.filter {
        tests.retain(|test| {
            test.name.contains(filter) || test.path.to_string_lossy().contains(filter)
        });
    }

    let capture_mode = if options.nocapture {
        "inherit"
    } else {
        "capture"
    };

    if tests.is_empty() {
        emit_report(options.format, capture_mode, 0, 0, 0, Vec::new())?;
        return Ok(());
    }

    let sgc = std::env::current_exe().into_diagnostic()?;
    let opt_level = if options.release { "2" } else { "0" };
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut json_cases = Vec::new();

    for test in &tests {
        let started = Instant::now();
        let assert_report_path = create_assert_report_path()?;
        let mut command = Command::new(&sgc);
        command
            .current_dir(options.root)
            .arg("run")
            .arg(&test.path)
            .arg("-O")
            .arg(opt_level)
            .env(ASSERT_REPORT_ENV, &assert_report_path);
        apply_module_map_env(&mut command, std::env::var_os(MODULE_MAP_ENV).as_deref());

        let display = test.path.display();
        let (ok, exit_code, stdout, stderr) = if options.nocapture {
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            let status = command
                .status()
                .into_diagnostic()
                .map_err(|err| miette::miette!("failed to run test {}: {}", display, err))?;
            (status.success(), status.code(), None, None)
        } else {
            let output = command
                .output()
                .into_diagnostic()
                .map_err(|err| miette::miette!("failed to run test {}: {}", display, err))?;
            let ok = output.status.success();
            let exit_code = output.status.code();
            let stdout = lossy_output(&output.stdout);
            let stderr = lossy_output(&output.stderr);
            (ok, exit_code, stdout, stderr)
        };
        let duration_ms = started.elapsed().as_millis();
        let (assertion, assertion_transport) = if ok {
            let _ = fs::remove_file(&assert_report_path);
            (None, None)
        } else {
            read_assertion_envelope(&assert_report_path)
        };

        if ok {
            passed += 1;
            if matches!(options.format, TestOutputFormat::Text) {
                println!("test ok {}", display);
            }
        } else {
            failed += 1;
            if matches!(options.format, TestOutputFormat::Text) {
                println!(
                    "test FAILED {} (exit status: {})",
                    display,
                    exit_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                );
                if let Some(AssertionEnvelopeRead::Valid(envelope)) = assertion.as_ref() {
                    println!("assertion: {}", envelope.message);
                }
                if let Some(diagnostic) = assertion_transport.as_ref() {
                    println!("assertion transport: {diagnostic}");
                }
                emit_captured_stream("stdout", stdout.as_deref());
                emit_captured_stream("stderr", stderr.as_deref());
            }
        }

        let assertion_json = assertion.as_ref().and_then(|read| match read {
            AssertionEnvelopeRead::Valid(envelope) => Some(envelope.clone()),
            _ => None,
        });
        if matches!(options.format, TestOutputFormat::Json) {
            json_cases.push(TestCaseJson {
                name: &test.name,
                path: test.path.to_string_lossy().to_string(),
                ok,
                duration_ms,
                exit_code,
                stdout: if ok { None } else { stdout },
                stderr: if ok { None } else { stderr },
                assertion: assertion_json,
                assertion_transport: assertion_transport.clone(),
            });
        }
    }

    let exit_status = if failed > 0 { 1 } else { 0 };
    emit_report(
        options.format,
        capture_mode,
        exit_status,
        passed,
        failed,
        json_cases,
    )?;

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn emit_report(
    format: TestOutputFormat,
    capture: &'static str,
    exit_status: i32,
    passed: usize,
    failed: usize,
    tests: Vec<TestCaseJson<'_>>,
) -> Result<()> {
    let total = passed + failed;
    match format {
        TestOutputFormat::Text => {
            if total == 0 {
                println!("no Sengoo tests found");
            } else if failed == 0 {
                println!("test result: {passed} passed");
            } else {
                println!("test result: {failed} failed, {passed} passed");
            }
        }
        TestOutputFormat::Json => {
            let report = TestReportJson {
                schema_version: 1,
                exit_status,
                capture,
                passed,
                failed,
                total,
                tests,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).into_diagnostic()?
            );
        }
    }
    Ok(())
}

fn ensure_lockfile_current(root: &Path, manifest_path: Option<&Path>) -> Result<()> {
    let manifest = manifest_path
        .map(Path::to_path_buf)
        .or_else(|| find_manifest(root))
        .ok_or_else(|| {
            miette::miette!(
                "--locked requires a package manifest; pass --manifest-path or run from a package root"
            )
        })?;

    let sgpm = which::which("sgpm").map_err(|_| {
        miette::miette!("--locked requires `sgpm` on PATH to verify Sengoo.lock freshness")
    })?;
    let status = Command::new(sgpm)
        .arg("update")
        .arg("--check")
        .arg("--manifest-path")
        .arg(&manifest)
        .status()
        .into_diagnostic()
        .map_err(|err| miette::miette!("failed to run sgpm update --check: {}", err))?;
    if status.success() {
        return Ok(());
    }
    miette::bail!(
        "Sengoo.lock is stale for {}; run `sgpm update --manifest-path {}`",
        root.display(),
        manifest.display()
    );
}

fn find_manifest(root: &Path) -> Option<PathBuf> {
    let manifest = root.join("Sengoo.toml");
    manifest.is_file().then_some(manifest)
}

fn discover_tests(root: &Path) -> Result<Vec<TestCase>> {
    let mut seen = BTreeSet::new();
    let mut tests = Vec::new();

    for path in manifest_declared_tests(root)? {
        push_test_case(root, path, &mut seen, &mut tests)?;
    }

    let tests_dir = root.join("tests");
    if tests_dir.is_dir() {
        collect_sg_tests(&tests_dir, root, &mut seen, &mut tests)?;
    }

    Ok(tests)
}

fn manifest_declared_tests(root: &Path) -> Result<Vec<PathBuf>> {
    let manifest = match find_manifest(root) {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };
    let source = fs::read_to_string(&manifest)
        .into_diagnostic()
        .map_err(|err| {
            miette::miette!("failed to read manifest {}: {}", manifest.display(), err)
        })?;
    let raw: ManifestForTests = toml::from_str(&source).into_diagnostic().map_err(|err| {
        miette::miette!("failed to parse manifest {}: {}", manifest.display(), err)
    })?;
    if let Some(version) = raw.sengoo_schema {
        if version != 1 {
            miette::bail!(
                "unsupported Sengoo.toml schema version {}; expected 1",
                version
            );
        }
    }
    Ok(raw
        .test
        .into_iter()
        .map(|entry| root.join(entry.path))
        .collect())
}

#[derive(Debug, serde::Deserialize)]
struct ManifestForTests {
    #[serde(default, rename = "sengoo-schema")]
    sengoo_schema: Option<u32>,
    #[serde(default)]
    test: Vec<ManifestTestTarget>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestTestTarget {
    path: PathBuf,
}

fn push_test_case(
    root: &Path,
    path: PathBuf,
    seen: &mut BTreeSet<String>,
    tests: &mut Vec<TestCase>,
) -> Result<()> {
    if !path.is_file() {
        miette::bail!("declared test path does not exist: {}", path.display());
    }
    let name = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    if seen.insert(name.clone()) {
        tests.push(TestCase { path, name });
    }
    Ok(())
}

fn collect_sg_tests(
    dir: &Path,
    root: &Path,
    seen: &mut BTreeSet<String>,
    tests: &mut Vec<TestCase>,
) -> Result<()> {
    for entry in fs::read_dir(dir).into_diagnostic().map_err(|err| {
        miette::miette!("failed to read tests directory {}: {}", dir.display(), err)
    })? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if path.is_dir() {
            collect_sg_tests(&path, root, seen, tests)?;
            continue;
        }
        if path.extension().and_then(OsStr::to_str) != Some("sg") {
            continue;
        }
        push_test_case(root, path, seen, tests)?;
    }
    Ok(())
}

pub(crate) fn resolve_test_root(
    path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(manifest_path) = manifest_path {
        let manifest =
            fs::canonicalize(manifest_path).unwrap_or_else(|_| manifest_path.to_path_buf());
        let parent = manifest
            .parent()
            .ok_or_else(|| miette::miette!("invalid manifest path: {}", manifest.display()))?;
        return Ok(parent.to_path_buf());
    }
    if let Some(path) = path {
        let candidate = PathBuf::from(path);
        if candidate.is_dir() {
            return Ok(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
        if candidate.is_file() {
            let parent = candidate
                .parent()
                .ok_or_else(|| miette::miette!("invalid test path: {}", candidate.display()))?;
            return Ok(fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf()));
        }
        return Ok(candidate);
    }
    std::env::current_dir().into_diagnostic()
}

fn lossy_output(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn create_assert_report_path() -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .into_diagnostic()?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!("sengoo_assert_report_{stamp}.json")))
}

pub(crate) fn read_assertion_envelope(
    report_path: &Path,
) -> (Option<AssertionEnvelopeRead>, Option<String>) {
    let metadata = match fs::metadata(report_path) {
        Ok(metadata) => metadata,
        Err(_) => return (Some(AssertionEnvelopeRead::Missing), None),
    };
    if metadata.len() as usize > MAX_ASSERT_ENVELOPE_BYTES {
        let _ = fs::remove_file(report_path);
        return (
            None,
            Some("assertion envelope exceeded 64 KiB limit".to_string()),
        );
    }

    let bytes = match fs::read(report_path) {
        Ok(bytes) => bytes,
        Err(_) => return (Some(AssertionEnvelopeRead::Missing), None),
    };
    let _ = fs::remove_file(report_path);

    if bytes.is_empty() {
        return (Some(AssertionEnvelopeRead::Missing), None);
    }

    let line = match std::str::from_utf8(bytes.split(|byte| *byte == b'\n').next().unwrap_or(&[])) {
        Ok(line) => line.trim(),
        Err(_) => {
            return (
                None,
                Some("assertion envelope is not valid UTF-8".to_string()),
            );
        }
    };

    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => {
            return (
                None,
                Some("assertion envelope is not valid JSON".to_string()),
            );
        }
    };

    let schema_version = value
        .get("schema_version")
        .and_then(|field| field.as_u64())
        .unwrap_or(0) as u32;
    if schema_version != 1 {
        return (
            None,
            Some(format!(
                "unsupported assertion envelope schema_version {schema_version}"
            )),
        );
    }

    match serde_json::from_value::<AssertionEnvelope>(value) {
        Ok(envelope) if envelope.kind == "assertion_failure" && !envelope.helper.is_empty() => {
            (Some(AssertionEnvelopeRead::Valid(envelope)), None)
        }
        Ok(_) => (
            None,
            Some("assertion envelope failed validation".to_string()),
        ),
        Err(_) => (
            None,
            Some("assertion envelope failed validation".to_string()),
        ),
    }
}

fn emit_captured_stream(label: &str, content: Option<&str>) {
    let Some(content) = content.filter(|value| !value.is_empty()) else {
        return;
    };
    println!("--- {label} ---");
    print!("{content}");
    if !content.ends_with('\n') {
        println!();
    }
}

pub(crate) fn apply_module_map_env(command: &mut Command, module_map: Option<&OsStr>) {
    command.env_remove(MODULE_MAP_ENV);
    if let Some(value) = module_map {
        if !value.is_empty() {
            command.env(MODULE_MAP_ENV, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sgc_test_{name}_{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_tests_includes_manifest_targets_and_tree() {
        let root = temp_dir("discover");
        fs::write(
            root.join("Sengoo.toml"),
            "sengoo-schema = 1\n[[test]]\npath = \"tests/custom.sg\"\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("tests/custom.sg"), "def main() -> i64 { 0 }\n").unwrap();
        fs::write(root.join("tests/basic.sg"), "def main() -> i64 { 0 }\n").unwrap();

        let tests = discover_tests(&root).expect("discover tests");
        assert_eq!(tests.len(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn json_report_has_schema_fields() {
        let json = serde_json::to_string(&TestReportJson {
            schema_version: 1,
            exit_status: 0,
            capture: "capture",
            passed: 1,
            failed: 0,
            total: 1,
            tests: vec![TestCaseJson {
                name: "tests/basic.sg",
                path: "tests/basic.sg".to_string(),
                ok: true,
                duration_ms: 3,
                exit_code: Some(0),
                stdout: None,
                stderr: None,
                assertion: None,
                assertion_transport: None,
            }],
        })
        .unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"capture\":\"capture\""));
        assert!(json.contains("\"exit_status\""));
    }

    #[test]
    fn lossy_output_preserves_non_empty_streams() {
        assert_eq!(lossy_output(b"hello"), Some("hello".to_string()));
        assert_eq!(lossy_output(b""), None);
    }

    #[test]
    fn read_assertion_envelope_accepts_schema_v1() {
        let dir = temp_dir("assert_envelope_valid");
        let path = dir.join("report.json");
        fs::write(
            &path,
            r#"{"schema_version":1,"kind":"assertion_failure","helper":"assert_eq_i64","message":"expected 7, got 9","file":"tests/smoke.sg","line":12,"expected":"7","actual":"9"}"#,
        )
        .unwrap();
        let (read, diagnostic) = read_assertion_envelope(&path);
        assert_eq!(diagnostic, None);
        assert_eq!(
            read,
            Some(AssertionEnvelopeRead::Valid(AssertionEnvelope {
                schema_version: 1,
                kind: "assertion_failure".to_string(),
                helper: "assert_eq_i64".to_string(),
                message: "expected 7, got 9".to_string(),
                file: Some("tests/smoke.sg".to_string()),
                line: Some(12),
                expected: Some("7".to_string()),
                actual: Some("9".to_string()),
            }))
        );
        assert!(!path.exists(), "report file should be removed after read");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_assertion_envelope_missing_file_is_not_fatal() {
        let dir = temp_dir("assert_envelope_missing");
        let path = dir.join("missing.json");
        let (read, diagnostic) = read_assertion_envelope(&path);
        assert_eq!(read, Some(AssertionEnvelopeRead::Missing));
        assert_eq!(diagnostic, None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_assertion_envelope_rejects_malformed_json() {
        let dir = temp_dir("assert_envelope_malformed");
        let path = dir.join("report.json");
        fs::write(&path, "{not-json").unwrap();
        let (read, diagnostic) = read_assertion_envelope(&path);
        assert_eq!(read, None);
        assert_eq!(
            diagnostic,
            Some("assertion envelope is not valid JSON".to_string())
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_assertion_envelope_rejects_unsupported_schema_version() {
        let dir = temp_dir("assert_envelope_version");
        let path = dir.join("report.json");
        fs::write(
            &path,
            r#"{"schema_version":2,"kind":"assertion_failure","helper":"assert","message":"fail"}"#,
        )
        .unwrap();
        let (read, diagnostic) = read_assertion_envelope(&path);
        assert_eq!(read, None);
        assert_eq!(
            diagnostic,
            Some("unsupported assertion envelope schema_version 2".to_string())
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_assertion_envelope_rejects_oversized_payload() {
        let dir = temp_dir("assert_envelope_oversized");
        let path = dir.join("report.json");
        let mut file = fs::File::create(&path).unwrap();
        write!(file, "{{\"schema_version\":1,\"kind\":\"assertion_failure\",\"helper\":\"assert\",\"message\":\"")
            .unwrap();
        write!(file, "{}", "x".repeat(MAX_ASSERT_ENVELOPE_BYTES)).unwrap();
        write!(file, "\"}}").unwrap();
        let (read, diagnostic) = read_assertion_envelope(&path);
        assert_eq!(read, None);
        assert_eq!(
            diagnostic,
            Some("assertion envelope exceeded 64 KiB limit".to_string())
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn create_assert_report_path_returns_absolute_path() {
        let path = create_assert_report_path().expect("assert report path");
        assert!(path.is_absolute());
    }
}
