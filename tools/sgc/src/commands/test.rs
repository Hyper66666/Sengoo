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
pub(crate) const COVERAGE_REPORT_ENV: &str = "SENGOO_COVERAGE_REPORT";
pub(crate) const COVERAGE_SOURCE_ENV: &str = "SENGOO_COVERAGE_SOURCE";
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
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage: Option<TestCoverageJson>,
    tests: Vec<TestCaseJson<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestCoverageJson {
    format: String,
    covered_lines: usize,
    executable_lines: usize,
    percent: u32,
}

#[derive(Debug, Default)]
struct LineCoverageTotals {
    executable: BTreeSet<(PathBuf, u32)>,
    covered: BTreeSet<(PathBuf, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestParameterJson {
    name: String,
    value: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<&'a str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<TestParameterJson>>,
}

struct TestCase {
    path: PathBuf,
    source_path: PathBuf,
    name: String,
    function: Option<String>,
    parameters: Option<Vec<TestParameterJson>>,
}

pub(crate) struct TestOptions<'a> {
    pub root: &'a Path,
    pub filter: Option<&'a str>,
    pub exact: Option<&'a str>,
    pub format: TestOutputFormat,
    pub nocapture: bool,
    pub release: bool,
    pub coverage: bool,
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
        let coverage = options
            .coverage
            .then(|| collect_line_coverage(&LineCoverageTotals::default()));
        emit_report(options.format, capture_mode, 0, 0, 0, coverage, Vec::new())?;
        return Ok(());
    }

    let sgc = std::env::current_exe().into_diagnostic()?;
    let opt_level = if options.release { "2" } else { "0" };
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut json_cases = Vec::new();
    let mut coverage_totals = LineCoverageTotals::default();

    for test in &tests {
        let started = Instant::now();
        let assert_report_path = create_assert_report_path()?;
        let coverage_report_path = options
            .coverage
            .then(create_coverage_report_path)
            .transpose()?;
        let mut command = Command::new(&sgc);
        command
            .current_dir(options.root)
            .arg("run")
            .arg(&test.path)
            .arg("-O")
            .arg(opt_level)
            .env(ASSERT_REPORT_ENV, &assert_report_path);
        if let Some(report_path) = coverage_report_path.as_deref() {
            fs::write(report_path, []).into_diagnostic()?;
            command
                .arg("--force-rebuild")
                .env(COVERAGE_REPORT_ENV, report_path)
                .env(COVERAGE_SOURCE_ENV, &test.source_path);
        }
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
        let (mut assertion, assertion_transport) = if ok {
            let _ = fs::remove_file(&assert_report_path);
            (None, None)
        } else {
            read_assertion_envelope(&assert_report_path)
        };
        if test.function.is_some() && test.path != test.source_path {
            if let Some(AssertionEnvelopeRead::Valid(envelope)) = assertion.as_mut() {
                envelope.file = Some(test.source_path.to_string_lossy().replace('\\', "/"));
            }
        }
        if let Some(report_path) = coverage_report_path.as_deref() {
            collect_runtime_line_coverage(&test.source_path, report_path, &mut coverage_totals)?;
            let _ = fs::remove_file(report_path);
        }

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
                function: test.function.as_deref(),
                path: test.path.to_string_lossy().to_string(),
                ok,
                duration_ms,
                exit_code,
                stdout: if ok { None } else { stdout },
                stderr: if ok { None } else { stderr },
                assertion: assertion_json,
                assertion_transport: assertion_transport.clone(),
                parameters: test.parameters.clone(),
            });
        }
    }

    let exit_status = if failed > 0 { 1 } else { 0 };
    let coverage = options
        .coverage
        .then(|| collect_line_coverage(&coverage_totals));
    emit_report(
        options.format,
        capture_mode,
        exit_status,
        passed,
        failed,
        coverage,
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
    coverage: Option<TestCoverageJson>,
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
            if let Some(coverage) = coverage {
                println!(
                    "coverage: {} / {} executable lines ({}%)",
                    coverage.covered_lines, coverage.executable_lines, coverage.percent
                );
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
                coverage,
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

fn collect_line_coverage(totals: &LineCoverageTotals) -> TestCoverageJson {
    let executable_lines = totals.executable.len();
    let covered_lines = totals.covered.intersection(&totals.executable).count();
    let percent = if executable_lines == 0 {
        100
    } else {
        ((covered_lines * 100) / executable_lines) as u32
    };
    TestCoverageJson {
        format: "line".to_string(),
        covered_lines,
        executable_lines,
        percent,
    }
}

fn collect_runtime_line_coverage(
    source_path: &Path,
    report_path: &Path,
    totals: &mut LineCoverageTotals,
) -> Result<()> {
    let report = fs::read_to_string(report_path)
        .into_diagnostic()
        .map_err(|err| {
            miette::miette!(
                "failed to read coverage report {}: {}",
                report_path.display(),
                err
            )
        })?;
    let source = fs::canonicalize(source_path).unwrap_or_else(|_| source_path.to_path_buf());
    for record in report.lines() {
        let Some((kind, line)) = record.trim().split_once(':') else {
            continue;
        };
        let Ok(line) = line.parse::<u32>() else {
            continue;
        };
        if line == 0 {
            continue;
        }
        let key = (source.clone(), line);
        match kind {
            "E" => {
                totals.executable.insert(key);
            }
            "H" => {
                totals.executable.insert(key.clone());
                totals.covered.insert(key);
            }
            _ => {}
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
        tests.push(TestCase {
            source_path: path.clone(),
            path,
            name,
            function: None,
            parameters: None,
        });
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
        if !push_function_test_cases(root, &path, seen, tests)? {
            push_test_case(root, path, seen, tests)?;
        }
    }
    Ok(())
}

fn push_function_test_cases(
    root: &Path,
    source_path: &Path,
    seen: &mut BTreeSet<String>,
    tests: &mut Vec<TestCase>,
) -> Result<bool> {
    let source = fs::read_to_string(source_path)
        .into_diagnostic()
        .map_err(|err| {
            miette::miette!(
                "failed to read test source {}: {}",
                source_path.display(),
                err
            )
        })?;
    let discovered = discover_test_functions(&source);
    if discovered.is_empty() {
        return Ok(false);
    }
    if source_defines_main(&source) {
        return Ok(false);
    }

    let harness_dir = root.join("target").join("sgc-test-harness");
    fs::create_dir_all(&harness_dir)
        .into_diagnostic()
        .map_err(|err| {
            miette::miette!(
                "failed to create test harness directory {}: {}",
                harness_dir.display(),
                err
            )
        })?;

    let fixtures = discover_test_fixtures(&source);
    let relative = source_path
        .strip_prefix(root)
        .unwrap_or(source_path)
        .to_string_lossy()
        .replace('\\', "/");
    for function in discovered {
        for invocation in function.invocations() {
            let name = invocation.name(&relative, &function.name);
            if !seen.insert(name.clone()) {
                continue;
            }
            let harness_name = sanitize_harness_name(&name);
            let harness_path = harness_dir.join(format!("{harness_name}.sg"));
            let body = build_function_test_harness_body(&function, &invocation, fixtures);
            let harness_source = strip_test_attributes(&source);
            fs::write(&harness_path, format!("{harness_source}{body}"))
                .into_diagnostic()
                .map_err(|err| {
                    miette::miette!(
                        "failed to write test harness {}: {}",
                        harness_path.display(),
                        err
                    )
                })?;
            tests.push(TestCase {
                path: harness_path,
                source_path: source_path.to_path_buf(),
                name,
                function: Some(function.name.clone()),
                parameters: invocation.parameters(),
            });
        }
    }

    Ok(true)
}

fn build_function_test_harness_body(
    function: &DiscoveredTestFunction,
    invocation: &TestInvocation,
    fixtures: TestFixtures,
) -> String {
    let setup = if fixtures.setup { "    setup();\n" } else { "" };
    let teardown = if fixtures.teardown {
        "    teardown();\n"
    } else {
        ""
    };
    let call = invocation.call(&function.name);
    match function.return_kind {
        TestFunctionReturnKind::Bool => format!(
            "\n\ndef main() -> i64 {{\n{setup}    let __sgc_test_ok = {call};\n{teardown}    if __sgc_test_ok {{ 0 }} else {{ 1 }}\n}}\n",
        ),
        TestFunctionReturnKind::Unit => {
            format!("\n\ndef main() -> i64 {{\n{setup}    {call};\n{teardown}    0\n}}\n",)
        }
        TestFunctionReturnKind::I64 => {
            format!(
                "\n\ndef main() -> i64 {{\n{setup}    let __sgc_test_status = {call};\n{teardown}    __sgc_test_status\n}}\n",
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestFunctionReturnKind {
    I64,
    Bool,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredTestFunction {
    name: String,
    return_kind: TestFunctionReturnKind,
    cases: Vec<DiscoveredTestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredTestCase {
    label: String,
    argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestInvocation {
    Plain,
    Case(DiscoveredTestCase),
}

impl DiscoveredTestFunction {
    fn invocations(&self) -> Vec<TestInvocation> {
        if self.cases.is_empty() {
            vec![TestInvocation::Plain]
        } else {
            self.cases
                .iter()
                .cloned()
                .map(TestInvocation::Case)
                .collect()
        }
    }
}

impl TestInvocation {
    fn name(&self, relative: &str, function_name: &str) -> String {
        match self {
            Self::Plain => format!("{relative}::{function_name}"),
            Self::Case(case) => format!("{relative}::{function_name}[{}]", case.label),
        }
    }

    fn call(&self, function_name: &str) -> String {
        match self {
            Self::Plain => format!("{function_name}()"),
            Self::Case(case) => format!("{function_name}({})", case.argument),
        }
    }

    fn parameters(&self) -> Option<Vec<TestParameterJson>> {
        match self {
            Self::Plain => None,
            Self::Case(case) => Some(vec![
                TestParameterJson {
                    name: "case".to_string(),
                    value: case.label.clone(),
                },
                TestParameterJson {
                    name: "arg0".to_string(),
                    value: case.argument.clone(),
                },
            ]),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TestFixtures {
    setup: bool,
    teardown: bool,
}

fn discover_test_functions(source: &str) -> Vec<DiscoveredTestFunction> {
    let mut discovered = Vec::new();
    let mut pending_test_attribute = false;
    let mut pending_cases = Vec::new();
    for line in source.lines() {
        if is_test_attribute_line(line) {
            pending_test_attribute = true;
            continue;
        }
        if let Some(case) = parse_test_case_attribute_line(line) {
            pending_cases.push(case);
            continue;
        }
        if let Some(mut function) =
            parse_test_function_line(line, pending_test_attribute || !pending_cases.is_empty())
        {
            function.cases = std::mem::take(&mut pending_cases);
            discovered.push(function);
            pending_test_attribute = false;
            continue;
        }
        if !line.trim().is_empty() && !line.trim_start().starts_with("//") {
            pending_test_attribute = false;
            pending_cases.clear();
        }
    }
    discovered
}

fn parse_test_function_line(
    line: &str,
    has_test_attribute: bool,
) -> Option<DiscoveredTestFunction> {
    let (name, after_name) = parse_def_name_and_tail(line)?;
    if !has_test_attribute && !name.starts_with("test_") {
        return None;
    }
    if !after_name.starts_with('(') {
        return None;
    }
    Some(DiscoveredTestFunction {
        name: name.to_string(),
        return_kind: parse_test_return_kind(after_name),
        cases: Vec::new(),
    })
}

fn parse_test_case_attribute_line(line: &str) -> Option<DiscoveredTestCase> {
    let trimmed = line.trim().trim_start_matches('\u{feff}');
    let inner = trimmed.strip_prefix("#[case(")?.strip_suffix(")]")?;
    let label_start = inner.trim_start().strip_prefix('"')?;
    let label_end = label_start.find('"')?;
    let label = &label_start[..label_end];
    let after_label = label_start[label_end + 1..].trim_start();
    let argument = after_label.strip_prefix(',')?.trim();
    if argument.is_empty() {
        return None;
    }
    Some(DiscoveredTestCase {
        label: label.to_string(),
        argument: argument.to_string(),
    })
}

fn parse_def_name_and_tail(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start().trim_start_matches('\u{feff}');
    let rest = trimmed.strip_prefix("def ")?;
    let name_end = rest
        .char_indices()
        .find_map(|(index, ch)| (!is_ident_char(ch)).then_some(index))
        .unwrap_or(rest.len());
    Some((&rest[..name_end], rest[name_end..].trim_start()))
}

fn discover_test_fixtures(source: &str) -> TestFixtures {
    let mut fixtures = TestFixtures::default();
    for line in source.lines() {
        let Some((name, tail)) = parse_def_name_and_tail(line) else {
            continue;
        };
        if !tail.starts_with('(') {
            continue;
        }
        match name {
            "setup" => fixtures.setup = true,
            "teardown" => fixtures.teardown = true,
            _ => {}
        }
    }
    fixtures
}

fn is_test_attribute_line(line: &str) -> bool {
    line.trim().trim_start_matches('\u{feff}') == "#[test]"
}

pub(crate) fn strip_test_attributes(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    for line in source.lines() {
        if is_test_attribute_line(line) || parse_test_case_attribute_line(line).is_some() {
            // Preserve source line numbers for debugger and coverage probes.
            stripped.push('\n');
            continue;
        }
        stripped.push_str(line);
        stripped.push('\n');
    }
    stripped
}

fn parse_test_return_kind(after_name: &str) -> TestFunctionReturnKind {
    let Some(close_paren) = after_name.find(')') else {
        return TestFunctionReturnKind::I64;
    };
    let tail = after_name[close_paren + 1..].trim_start();
    if let Some(return_ty) = tail.strip_prefix("->") {
        let return_ty = return_ty.trim_start();
        if return_ty.starts_with("bool") {
            TestFunctionReturnKind::Bool
        } else if return_ty.starts_with("()") || return_ty.starts_with("unit") {
            TestFunctionReturnKind::Unit
        } else {
            TestFunctionReturnKind::I64
        }
    } else {
        TestFunctionReturnKind::Unit
    }
}

fn source_defines_main(source: &str) -> bool {
    source.lines().any(|line| {
        parse_def_name_and_tail(line)
            .is_some_and(|(name, tail)| name == "main" && tail.starts_with('('))
    })
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn sanitize_harness_name(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
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

fn create_coverage_report_path() -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .into_diagnostic()?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "sengoo_coverage_report_{}_{}.txt",
        std::process::id(),
        stamp
    )))
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
    fn discover_tests_expands_def_test_functions_into_harnesses() {
        let root = temp_dir("discover_functions");
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/functions.sg"),
            "def helper() -> i64 { 0 }\n\
             def test_adds() -> i64 { 0 }\n\
             def test_predicate() -> bool { true }\n",
        )
        .unwrap();
        fs::write(
            root.join("tests/legacy.sg"),
            "def main() -> i64 { 0 }\ndef test_ignored_with_main() -> i64 { 0 }\n",
        )
        .unwrap();

        let tests = discover_tests(&root).expect("discover tests");
        let names = tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"tests/functions.sg::test_adds"));
        assert!(names.contains(&"tests/functions.sg::test_predicate"));
        assert!(names.contains(&"tests/legacy.sg"));
        assert_eq!(tests.len(), 3);
        for test in tests
            .iter()
            .filter(|test| test.name.starts_with("tests/functions.sg::"))
        {
            let harness = fs::read_to_string(&test.path).expect("harness source");
            assert!(
                harness.contains("def main() -> i64"),
                "function test harness should synthesize main:\n{harness}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_tests_expands_test_attributes_into_harnesses() {
        let root = temp_dir("discover_attribute_functions");
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/functions.sg"),
            "#[test]\n\
             def adds() -> i64 { 0 }\n\
             \n\
             #[test]\n\
             def predicate() -> bool { true }\n\
             def helper() -> i64 { 0 }\n",
        )
        .unwrap();

        let tests = discover_tests(&root).expect("discover tests");
        let names = tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"tests/functions.sg::adds"));
        assert!(names.contains(&"tests/functions.sg::predicate"));
        assert_eq!(tests.len(), 2);
        for test in tests {
            let harness = fs::read_to_string(&test.path).expect("harness source");
            assert!(
                !harness.contains("#[test]"),
                "generated harness should strip sgc-only test attributes:\n{harness}"
            );
            assert!(
                harness.contains("def main() -> i64"),
                "attribute test harness should synthesize main:\n{harness}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_tests_wraps_function_tests_with_fixtures() {
        let root = temp_dir("discover_fixtures");
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/functions.sg"),
            "def setup() { }\n\
             def teardown() { }\n\
             def test_status() -> i64 { 0 }\n\
             #[test]\n\
             def predicate() -> bool { true }\n\
             #[test]\n\
             def unit_case() { }\n",
        )
        .unwrap();

        let tests = discover_tests(&root).expect("discover tests");
        assert_eq!(tests.len(), 3);
        for test in tests {
            let harness = fs::read_to_string(&test.path).expect("harness source");
            assert!(
                harness.contains("setup();"),
                "function test harness should call setup before the case:\n{harness}"
            );
            assert!(
                harness.contains("teardown();"),
                "function test harness should call teardown after the case:\n{harness}"
            );
            let setup_at = harness.find("setup();").expect("setup call");
            let teardown_at = harness.find("teardown();").expect("teardown call");
            assert!(
                setup_at < teardown_at,
                "setup should be emitted before teardown:\n{harness}"
            );
            assert!(
                !harness.contains("#[test]"),
                "fixture harness should strip sgc-only test attributes:\n{harness}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_tests_expands_parameterized_cases_into_harnesses() {
        let root = temp_dir("discover_parameterized_cases");
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/functions.sg"),
            "#[case(\"zero\", 0)]\n\
             #[case(\"one\", 1)]\n\
             def accepts_value(value: i64) -> bool { value >= 0 }\n",
        )
        .unwrap();

        let tests = discover_tests(&root).expect("discover tests");
        let names = tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"tests/functions.sg::accepts_value[zero]"));
        assert!(names.contains(&"tests/functions.sg::accepts_value[one]"));
        assert_eq!(tests.len(), 2);
        for test in tests {
            let harness = fs::read_to_string(&test.path).expect("harness source");
            assert!(
                !harness.contains("#[case("),
                "generated harness should strip sgc-only case attributes:\n{harness}"
            );
            if test.name.ends_with("[zero]") {
                assert!(harness.contains("accepts_value(0)"));
                assert_eq!(
                    test.parameters,
                    Some(vec![
                        TestParameterJson {
                            name: "case".to_string(),
                            value: "zero".to_string(),
                        },
                        TestParameterJson {
                            name: "arg0".to_string(),
                            value: "0".to_string(),
                        },
                    ])
                );
            }
            if test.name.ends_with("[one]") {
                assert!(harness.contains("accepts_value(1)"));
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_test_functions_parses_return_kinds() {
        let found = discover_test_functions(
            "def test_i64() -> i64 { 0 }\n\
             def test_bool() -> bool { true }\n\
             def test_unit() { }\n\
             def helper() -> i64 { 0 }\n",
        );
        assert_eq!(
            found,
            vec![
                DiscoveredTestFunction {
                    name: "test_i64".to_string(),
                    return_kind: TestFunctionReturnKind::I64,
                    cases: Vec::new(),
                },
                DiscoveredTestFunction {
                    name: "test_bool".to_string(),
                    return_kind: TestFunctionReturnKind::Bool,
                    cases: Vec::new(),
                },
                DiscoveredTestFunction {
                    name: "test_unit".to_string(),
                    return_kind: TestFunctionReturnKind::Unit,
                    cases: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn discover_test_functions_parses_parameterized_cases() {
        let found = discover_test_functions(
            "#[case(\"small\", 1)]\n\
             #[case(\"large\", 42)]\n\
             def accepts_value(value: i64) -> bool { value > 0 }\n",
        );
        assert_eq!(
            found,
            vec![DiscoveredTestFunction {
                name: "accepts_value".to_string(),
                return_kind: TestFunctionReturnKind::Bool,
                cases: vec![
                    DiscoveredTestCase {
                        label: "small".to_string(),
                        argument: "1".to_string(),
                    },
                    DiscoveredTestCase {
                        label: "large".to_string(),
                        argument: "42".to_string(),
                    },
                ],
            }]
        );
    }

    #[test]
    fn discover_test_functions_accepts_test_attribute_on_non_prefixed_names() {
        let found = discover_test_functions(
            "#[test]\n\
             def adds() -> i64 { 0 }\n\
             #[test]\n\
             def predicate() -> bool { true }\n\
             def helper() -> i64 { 0 }\n",
        );
        assert_eq!(
            found,
            vec![
                DiscoveredTestFunction {
                    name: "adds".to_string(),
                    return_kind: TestFunctionReturnKind::I64,
                    cases: Vec::new(),
                },
                DiscoveredTestFunction {
                    name: "predicate".to_string(),
                    return_kind: TestFunctionReturnKind::Bool,
                    cases: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn discover_test_functions_accepts_utf8_bom_before_test_attribute() {
        let found = discover_test_functions("\u{feff}#[test]\ndef adds() -> i64 { 0 }\n");
        assert_eq!(
            found,
            vec![DiscoveredTestFunction {
                name: "adds".to_string(),
                return_kind: TestFunctionReturnKind::I64,
                cases: Vec::new(),
            }]
        );
        assert_eq!(
            strip_test_attributes("\u{feff}#[test]\ndef adds() -> i64 { 0 }\n"),
            "\ndef adds() -> i64 { 0 }\n"
        );
    }

    #[test]
    fn discover_test_fixtures_detects_setup_and_teardown() {
        assert_eq!(
            discover_test_fixtures(
                "\u{feff}def setup() { }\ndef teardown() { }\ndef test_case() { }\n"
            ),
            TestFixtures {
                setup: true,
                teardown: true,
            }
        );
        assert_eq!(
            discover_test_fixtures("def setup_flag() { }\ndef teardown_later() { }\n"),
            TestFixtures::default()
        );
    }

    #[test]
    fn line_coverage_aggregates_runtime_records_and_deduplicates_test_cases() {
        let root = temp_dir("coverage_sources");
        let source = root.join("tests").join("cases.sg");
        let first = root.join("first.coverage");
        let second = root.join("second.coverage");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "def accepts(value: i64) -> bool { value > 0 }\n").unwrap();
        fs::write(&first, "E:1\nE:2\nH:1\nH:1\n").unwrap();
        fs::write(&second, "E:1\nE:2\nH:2\n").unwrap();

        let mut totals = LineCoverageTotals::default();
        collect_runtime_line_coverage(&source, &first, &mut totals).unwrap();
        collect_runtime_line_coverage(&source, &second, &mut totals).unwrap();
        let coverage = collect_line_coverage(&totals);
        assert_eq!(
            coverage,
            TestCoverageJson {
                format: "line".to_string(),
                covered_lines: 2,
                executable_lines: 2,
                percent: 100,
            }
        );
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
            coverage: None,
            tests: vec![TestCaseJson {
                name: "tests/basic.sg",
                function: None,
                path: "tests/basic.sg".to_string(),
                ok: true,
                duration_ms: 3,
                exit_code: Some(0),
                stdout: None,
                stderr: None,
                assertion: None,
                assertion_transport: None,
                parameters: None,
            }],
        })
        .unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"capture\":\"capture\""));
        assert!(json.contains("\"exit_status\""));
        assert!(
            !json.contains("\"coverage\""),
            "reserved coverage field should be omitted until --coverage is implemented: {json}"
        );
        assert!(
            !json.contains("\"parameters\""),
            "reserved parameters field should be omitted for normal tests: {json}"
        );
    }

    #[test]
    fn json_report_schema_accepts_future_coverage_and_parameters() {
        let json = serde_json::to_string(&TestReportJson {
            schema_version: 1,
            exit_status: 0,
            capture: "capture",
            passed: 1,
            failed: 0,
            total: 1,
            coverage: Some(TestCoverageJson {
                format: "line".to_string(),
                covered_lines: 3,
                executable_lines: 4,
                percent: 75,
            }),
            tests: vec![TestCaseJson {
                name: "tests/table.sg",
                function: None,
                path: "tests/table.sg".to_string(),
                ok: true,
                duration_ms: 5,
                exit_code: Some(0),
                stdout: None,
                stderr: None,
                assertion: None,
                assertion_transport: None,
                parameters: Some(vec![TestParameterJson {
                    name: "case".to_string(),
                    value: "happy-path".to_string(),
                }]),
            }],
        })
        .unwrap();
        assert!(json.contains("\"coverage\""));
        assert!(json.contains("\"covered_lines\":3"));
        assert!(json.contains("\"parameters\""));
        assert!(json.contains("\"happy-path\""));
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
