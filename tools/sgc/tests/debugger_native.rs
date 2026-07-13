use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const BREAK_MARKER: &str = "SENGOO_DEBUG_BREAK";
const STEP_MARKER: &str = "SENGOO_DEBUG_STEP";
const EXIT_MARKER: &str = "SENGOO_DEBUG_EXIT";
const PROBE_SOURCE_FILE: &str = "debugger_probe.sg";
const PROBE_SOURCE: &str = r#"def debug_probe(value: i64) -> i64 {
    let doubled = value * 2;
    let stepped = doubled + 1;
    stepped
}

def main() -> i64 {
    if debug_probe(21) == 43 { 0 } else { 1 }
}
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DebuggerFlavor {
    Lldb,
    Cdb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProbeLayout {
    break_line: usize,
    step_line: usize,
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sgc_debugger_native_{}_{}",
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&root).expect("create debugger test project");
        Self { root }
    }

    fn source_path(&self) -> PathBuf {
        self.root.join(PROBE_SOURCE_FILE)
    }

    fn executable_path(&self) -> PathBuf {
        let name = if cfg!(windows) {
            "debugger_probe.exe"
        } else {
            "debugger_probe"
        };
        self.root.join("build").join(name)
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn sgc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sgc"))
}

fn host_debugger() -> Option<(DebuggerFlavor, &'static str)> {
    #[cfg(windows)]
    {
        return Some((DebuggerFlavor::Cdb, "cdb"));
    }
    #[cfg(unix)]
    {
        return Some((DebuggerFlavor::Lldb, "lldb"));
    }
    #[allow(unreachable_code)]
    None
}

fn find_tool(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
}

fn probe_layout() -> ProbeLayout {
    ProbeLayout {
        break_line: source_line_number(PROBE_SOURCE, "let doubled ="),
        step_line: source_line_number(PROBE_SOURCE, "let stepped ="),
    }
}

fn source_line_number(source: &str, needle: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(needle))
        .map(|index| index + 1)
        .unwrap_or_else(|| panic!("missing `{needle}` in probe source"))
}

fn source_file_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|file| file.to_str())
        .unwrap_or_else(|| panic!("source path has no UTF-8 file name: {}", source.display()))
        .to_string()
}

fn lldb_arguments(executable: &Path, source: &Path, layout: &ProbeLayout) -> Vec<String> {
    let file_name = source_file_name(source);
    [
        "--batch".to_string(),
        "-o".to_string(),
        format!(
            "breakpoint set --file {file_name} --line {}",
            layout.break_line
        ),
        "-o".to_string(),
        "breakpoint list 1".to_string(),
        "-o".to_string(),
        "run".to_string(),
        "-o".to_string(),
        format!("script print(\"{BREAK_MARKER}\")"),
        "-o".to_string(),
        "frame info".to_string(),
        "-o".to_string(),
        "frame variable value".to_string(),
        "-o".to_string(),
        "next".to_string(),
        "-o".to_string(),
        format!("script print(\"{STEP_MARKER}\")"),
        "-o".to_string(),
        "frame info".to_string(),
        "-o".to_string(),
        "frame variable doubled".to_string(),
        "-o".to_string(),
        "continue".to_string(),
        "-o".to_string(),
        format!("script print(\"{EXIT_MARKER}\")"),
        "--".to_string(),
        executable.to_string_lossy().into_owned(),
    ]
    .into_iter()
    .collect()
}

fn cdb_script(source: &Path, layout: &ProbeLayout) -> String {
    format!(
        ".lines\nl+t\nl+s\n.reload /f\nbp `{source}:{line}`\nbl\ng\n.echo {BREAK_MARKER}\nl+s\ndv /t value\np\n.echo {STEP_MARKER}\nl+s\ndv /t doubled\ng\n.echo {EXIT_MARKER}\nq\n",
        source = source.display(),
        line = layout.break_line
    )
}

fn cdb_arguments(executable: &Path, script: &Path) -> Vec<String> {
    vec![
        "-lines".to_string(),
        "-cf".to_string(),
        script.to_string_lossy().into_owned(),
        executable.to_string_lossy().into_owned(),
    ]
}

fn line_has_named_value(line: &str, name: &str, decimal: i64, hex: &str) -> bool {
    if !line.contains(name) {
        return false;
    }
    let compact = line.replace(' ', "").to_ascii_lowercase();
    compact.contains(&format!("={decimal}"))
        || compact.contains(&format!("=0n{decimal}"))
        || compact.contains(&format!("=0x{hex}"))
        || (compact.contains("=0x") && compact.ends_with(hex))
}

fn output_after_marker<'a>(output: &'a str, marker: &str) -> Result<&'a str, String> {
    output
        .split_once(marker)
        .map(|(_, rest)| rest)
        .ok_or_else(|| format!("debugger did not reach marker `{marker}`:\n{output}"))
}

fn contains_source_location(output: &str, source: &Path, line: usize) -> bool {
    let normalized_output = output.replace('\\', "/").to_ascii_lowercase();
    let file_name = source_file_name(source)
        .replace('\\', "/")
        .to_ascii_lowercase();
    let full_path = source
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    [
        format!("{file_name}:{line}"),
        format!("{file_name}({line})"),
        format!("{file_name} @ {line}"),
        format!("{full_path}:{line}"),
        format!("{full_path}({line})"),
        format!("{full_path} @ {line}"),
    ]
    .into_iter()
    .any(|pattern| normalized_output.contains(&pattern))
}

fn validate_debugger_output(
    output: &str,
    source: &Path,
    layout: &ProbeLayout,
) -> Result<(), String> {
    if !output.contains(BREAK_MARKER) {
        return Err(format!(
            "debugger did not reach the breakpoint marker:\n{output}"
        ));
    }
    if !output.contains(STEP_MARKER) {
        return Err(format!(
            "debugger did not complete the step command:\n{output}"
        ));
    }
    if !output.contains(EXIT_MARKER) {
        return Err(format!(
            "debugger did not resume to the post-continue exit marker:\n{output}"
        ));
    }
    let break_segment = output_after_marker(output, BREAK_MARKER)?
        .split(STEP_MARKER)
        .next()
        .unwrap_or_default();
    if !contains_source_location(break_segment, source, layout.break_line) {
        return Err(format!(
            "debugger did not bind/hit {}:{} at the breakpoint:\n{output}",
            source.display(),
            layout.break_line
        ));
    }
    let step_segment = output_after_marker(output, STEP_MARKER)?
        .split(EXIT_MARKER)
        .next()
        .unwrap_or_default();
    if !contains_source_location(step_segment, source, layout.step_line) {
        return Err(format!(
            "debugger did not advance to {}:{} after next:\n{output}",
            source.display(),
            layout.step_line
        ));
    }
    if !output
        .lines()
        .any(|line| line_has_named_value(line, "value", 21, "15"))
    {
        return Err(format!(
            "debugger did not report parameter `value` as 21:\n{output}"
        ));
    }
    if !output
        .lines()
        .any(|line| line_has_named_value(line, "doubled", 42, "2a"))
    {
        return Err(format!(
            "debugger did not report local `doubled` as 42 after stepping:\n{output}"
        ));
    }
    Ok(())
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn skip(reason: &str) {
    eprintln!("SKIP debugger_native::native_debugger_breaks_steps_and_reads_local: {reason}");
}

#[test]
fn lldb_batch_commands_break_step_and_read_values() {
    let _flavor = DebuggerFlavor::Lldb;
    let layout = probe_layout();
    let args = lldb_arguments(
        Path::new("/tmp/debugger_probe"),
        Path::new("/tmp/debugger_probe.sg"),
        &layout,
    );
    let joined = args.join("\n");

    assert!(joined.contains(&format!(
        "breakpoint set --file debugger_probe.sg --line {}",
        layout.break_line
    )));
    assert!(joined.contains("breakpoint list 1"));
    assert!(joined.contains("frame info"));
    assert!(joined.contains("frame variable value"));
    assert_eq!(args.iter().filter(|arg| arg.as_str() == "next").count(), 1);
    assert!(joined.contains("frame variable doubled"));
    assert!(joined.contains(EXIT_MARKER));
    assert_eq!(args.last().map(String::as_str), Some("/tmp/debugger_probe"));
}

#[test]
fn cdb_script_breaks_steps_and_reads_typed_values() {
    let _flavor = DebuggerFlavor::Cdb;
    let layout = probe_layout();
    let script = cdb_script(Path::new(r"C:\probe\debugger_probe.sg"), &layout);

    assert!(script.contains(&format!(
        "bp `C:\\probe\\debugger_probe.sg:{}`",
        layout.break_line
    )));
    assert!(script.contains("l+t"));
    assert!(script.contains("l+s"));
    assert!(script.contains("bl"));
    assert!(script.contains("dv /t value"));
    assert_eq!(script.lines().filter(|line| *line == "p").count(), 1);
    assert!(script.contains("dv /t doubled"));
    assert!(script.contains(EXIT_MARKER));
    assert!(script.contains("q"));
}

#[test]
fn cdb_batch_arguments_use_the_generated_script_and_executable() {
    let args = cdb_arguments(
        Path::new(r"C:\probe\debugger_probe.exe"),
        Path::new(r"C:\probe\debugger_probe.cdb"),
    );

    assert_eq!(args[0], "-lines");
    assert_eq!(args[1], "-cf");
    assert!(args[2].ends_with("debugger_probe.cdb"));
    assert!(args[3].ends_with("debugger_probe.exe"));
}

#[test]
fn debugger_output_requires_break_step_and_expected_values() {
    let layout = probe_layout();
    let lldb = r#"
Breakpoint 1: where = debugger_probe`debug_probe + 16 at debugger_probe.sg:2:5, address = 0x0000000140001234
SENGOO_DEBUG_BREAK
(lldb) frame info
frame #0: 0x0000000140001234 debugger_probe`debug_probe + 16 at debugger_probe.sg:2:5
(long long) value = 21
SENGOO_DEBUG_STEP
(lldb) frame info
frame #0: 0x0000000140001240 debugger_probe`debug_probe + 32 at debugger_probe.sg:3:5
(long long) doubled = 42
Process 123 exited with status = 0 (0x00000000)
SENGOO_DEBUG_EXIT
"#;
    assert!(validate_debugger_output(lldb, Path::new("/tmp/debugger_probe.sg"), &layout).is_ok());

    let cdb = r#"
0:000> bp `C:\probe\debugger_probe.sg:2`
SENGOO_DEBUG_BREAK
C:\probe\debugger_probe.sg(2)
long long value = 0x0000000000000015
SENGOO_DEBUG_STEP
C:\probe\debugger_probe.sg(3)
long long doubled = 0x000000000000002a
SENGOO_DEBUG_EXIT
"#;
    assert!(
        validate_debugger_output(cdb, Path::new(r"C:\probe\debugger_probe.sg"), &layout).is_ok()
    );

    assert!(validate_debugger_output(
        "value = 21\ndoubled = 42",
        Path::new("/tmp/debugger_probe.sg"),
        &layout
    )
    .is_err());
    assert!(validate_debugger_output(
        "SENGOO_DEBUG_BREAK\ndebugger_probe.sg:2\nvalue = 21\nSENGOO_DEBUG_STEP\ndebugger_probe.sg:2\ndoubled = 42\nSENGOO_DEBUG_EXIT",
        Path::new("/tmp/debugger_probe.sg"),
        &layout
    )
    .is_err());
    assert!(validate_debugger_output(
        "SENGOO_DEBUG_BREAK\ndebugger_probe.sg:2\nvalue = 21\nSENGOO_DEBUG_STEP\ndebugger_probe.sg:3\ndoubled = 42",
        Path::new("/tmp/debugger_probe.sg"),
        &layout
    )
    .is_err());
}

#[test]
fn native_debugger_breaks_steps_and_reads_local() {
    let layout = probe_layout();
    let Some((flavor, debugger_name)) = host_debugger() else {
        skip("host platform has no supported LLDB/CDB driver");
        return;
    };
    let Some(debugger) = find_tool(debugger_name) else {
        skip(&format!(
            "required debugger `{debugger_name}` was not found on PATH"
        ));
        return;
    };
    if find_tool(if cfg!(windows) { "clang.exe" } else { "clang" }).is_none() {
        skip("native clang toolchain was not found on PATH");
        return;
    }

    let project = TempProject::new();
    let source = project.source_path();
    fs::write(&source, PROBE_SOURCE).expect("write debugger probe source");

    let build = Command::new(sgc())
        .arg("build")
        .arg(&source)
        .args(["-O", "0", "--debug-info", "--force-rebuild"])
        .output()
        .expect("run sgc debug build");
    assert!(
        build.status.success(),
        "sgc debug build failed:\n{}",
        combined_output(&build)
    );

    let executable = project.executable_path();
    assert!(
        executable.is_file(),
        "sgc did not create expected debug executable {}\nbuild output:\n{}",
        executable.display(),
        combined_output(&build)
    );

    let output = match flavor {
        DebuggerFlavor::Lldb => Command::new(&debugger)
            .args(lldb_arguments(&executable, &source, &layout))
            .output()
            .expect("run LLDB batch session"),
        DebuggerFlavor::Cdb => {
            let script = project.root.join("debugger_probe.cdb");
            fs::write(&script, cdb_script(&source, &layout)).expect("write CDB command script");
            Command::new(&debugger)
                .args(cdb_arguments(&executable, &script))
                .output()
                .expect("run CDB batch session")
        }
    };
    let transcript = combined_output(&output);
    assert!(
        output.status.success(),
        "{debugger_name} batch session failed:\n{transcript}"
    );
    validate_debugger_output(&transcript, &source, &layout)
        .unwrap_or_else(|error| panic!("{debugger_name} validation failed: {error}"));
}
