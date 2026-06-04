use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum TestOutputFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
struct TestReportJson<'a> {
    passed: usize,
    failed: usize,
    total: usize,
    tests: Vec<TestCaseJson<'a>>,
}

#[derive(Debug, Serialize)]
struct TestCaseJson<'a> {
    name: &'a str,
    path: String,
    ok: bool,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
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
}

pub(crate) fn cmd_test(options: TestOptions<'_>) -> Result<()> {
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

    if tests.is_empty() {
        match options.format {
            TestOutputFormat::Text => println!("no Sengoo tests found"),
            TestOutputFormat::Json => {
                let report = TestReportJson {
                    passed: 0,
                    failed: 0,
                    total: 0,
                    tests: Vec::new(),
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).into_diagnostic()?
                );
            }
        }
        return Ok(());
    }

    let sgc = std::env::current_exe().into_diagnostic()?;
    let opt_level = if options.release { "2" } else { "1" };
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut json_cases = Vec::new();

    for test in &tests {
        let started = Instant::now();
        let mut command = Command::new(&sgc);
        command
            .current_dir(options.root)
            .arg("run")
            .arg(&test.path)
            .arg("-O")
            .arg(opt_level);
        if options.nocapture {
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let display = test.path.display();
        let status = command
            .status()
            .into_diagnostic()
            .map_err(|err| miette::miette!("failed to run test {}: {}", display, err))?;
        let duration_ms = started.elapsed().as_millis();
        let ok = status.success();
        let exit_code = status.code();

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
            }
        }

        if matches!(options.format, TestOutputFormat::Json) {
            json_cases.push(TestCaseJson {
                name: &test.name,
                path: test.path.to_string_lossy().to_string(),
                ok,
                duration_ms,
                exit_code,
            });
        }
    }

    let total = passed + failed;
    match options.format {
        TestOutputFormat::Text => {
            if failed == 0 {
                println!("test result: {passed} passed");
            } else {
                println!("test result: {failed} failed, {passed} passed");
            }
        }
        TestOutputFormat::Json => {
            let report = TestReportJson {
                passed,
                failed,
                total,
                tests: json_cases,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).into_diagnostic()?
            );
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn discover_tests(root: &Path) -> Result<Vec<TestCase>> {
    let tests_dir = root.join("tests");
    if !tests_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut tests = Vec::new();
    collect_sg_tests(&tests_dir, root, &mut tests)?;
    Ok(tests)
}

fn collect_sg_tests(dir: &Path, root: &Path, tests: &mut Vec<TestCase>) -> Result<()> {
    for entry in fs::read_dir(dir).into_diagnostic().map_err(|err| {
        miette::miette!("failed to read tests directory {}: {}", dir.display(), err)
    })? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if path.is_dir() {
            collect_sg_tests(&path, root, tests)?;
            continue;
        }
        if path.extension().and_then(OsStr::to_str) != Some("sg") {
            continue;
        }
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        tests.push(TestCase { path, name });
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
