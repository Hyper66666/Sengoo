use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const BREAK_MARKER: &str = "SENGOO_DEBUG_BREAK";
const STEP_MARKER: &str = "SENGOO_DEBUG_STEP";
const COMPOSITE_MARKER: &str = "SENGOO_DEBUG_COMPOSITES";
const CALL_MARKER: &str = "SENGOO_DEBUG_CALL";
const CALL_STEP_MARKER: &str = "SENGOO_DEBUG_CALL_STEP";
const CALL_BODY_MARKER: &str = "SENGOO_DEBUG_CALL_BODY";
const CLOSURE_MARKER: &str = "SENGOO_DEBUG_CLOSURE";
const CLOSURE_STEP_MARKER: &str = "SENGOO_DEBUG_CLOSURE_STEP";
const EXIT_MARKER: &str = "SENGOO_DEBUG_EXIT_ZERO";
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
const COMPOSITE_PROBE_SOURCE_FILE: &str = "debugger_composite_probe.sg";
const COMPOSITE_PROBE_SOURCE: &str = r#"import std::collections;

struct Pair {
    left: i64,
    enabled: bool,
}

enum Choice { Empty, Value(i64) }

def scalar_helper(value: i64) -> i64 {
    let adjusted = value + 1;
    adjusted
}

def inspect_composites(value: i64) -> i64 {
    let pair = Pair { left: value, enabled: true };
    let picked = Choice::Value(7);
    let text = string_from_str("hi").unwrap_or(String { handle: 0 });
    let values = vec_new_i64();
    values.push(3);
    let observed = pair.left + text.len() + values.len();
    if pair.enabled { observed } else { 0 }
}

def call_surface(value: i64) -> i64 {
    let called = scalar_helper(value);
    called
}

def closure_surface(value: i64) -> i64 {
    let add = |extra| value + extra;
    let closed = add(2);
    let result = closed + 1;
    result
}

def main() -> i64 {
    let composites = inspect_composites(21);
    let called = call_surface(21);
    let closed = closure_surface(21);
    if composites == 24 && called == 22 && closed == 24 { 0 } else { 1 }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompositeProbeLayout {
    composite_line: usize,
    call_entry_line: usize,
    helper_entry_line: usize,
    helper_line: usize,
    closure_call_line: usize,
    closure_step_line: usize,
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

    fn composite_source_path(&self) -> PathBuf {
        self.root.join(COMPOSITE_PROBE_SOURCE_FILE)
    }

    #[cfg(unix)]
    fn composite_executable_path(&self) -> PathBuf {
        let name = if cfg!(windows) {
            "debugger_composite_probe.exe"
        } else {
            "debugger_composite_probe"
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

fn composite_lldb_arguments(
    executable: &Path,
    source: &Path,
    layout: &CompositeProbeLayout,
) -> Vec<String> {
    let file_name = source_file_name(source);
    let mut args = vec!["--batch".to_string()];
    for line in [
        layout.composite_line,
        layout.call_entry_line,
        layout.helper_line,
        layout.closure_call_line,
    ] {
        args.extend([
            "-o".to_string(),
            format!("breakpoint set --file {file_name} --line {line}"),
        ]);
    }
    for command in [
        "run".to_string(),
        format!("script print(\"{COMPOSITE_MARKER}\")"),
        "frame info".to_string(),
        "frame variable pair".to_string(),
        "frame variable pair.left".to_string(),
        "frame variable pair.enabled".to_string(),
        "frame variable picked".to_string(),
        "frame variable picked.discriminant".to_string(),
        "frame variable text".to_string(),
        "frame variable text.handle".to_string(),
        "frame variable values".to_string(),
        "frame variable values.handle".to_string(),
        "frame variable values.marker".to_string(),
        "continue".to_string(),
        format!("script print(\"{CALL_MARKER}\")"),
        "frame info".to_string(),
        "thread backtrace".to_string(),
        "frame variable value".to_string(),
        "step".to_string(),
        format!("script print(\"{CALL_STEP_MARKER}\")"),
        "frame info".to_string(),
        "frame variable value".to_string(),
        "step".to_string(),
        format!("script print(\"{CALL_BODY_MARKER}\")"),
        "frame info".to_string(),
        "finish".to_string(),
        "continue".to_string(),
        format!("script print(\"{CLOSURE_MARKER}\")"),
        "frame info".to_string(),
        "thread backtrace".to_string(),
        "next".to_string(),
        format!("script print(\"{CLOSURE_STEP_MARKER}\")"),
        "frame info".to_string(),
        "continue".to_string(),
        format!("script print(\"{EXIT_MARKER}\")"),
    ] {
        args.extend(["-o".to_string(), command]);
    }
    args.extend(["--".to_string(), executable.to_string_lossy().into_owned()]);
    args
}

fn cdb_script(source: &Path, layout: &ProbeLayout) -> String {
    format!(
        ".lines -e\nl+t\nl+s\n.reload /f\nbp `{source}:{line}`\nbl\n.echo {BREAK_MARKER}\ng\nl+s\ndv /t value\n.echo {STEP_MARKER}\np\nl+s\ndv /t doubled\ng\n.if (@rdx == 0) {{ .echo {EXIT_MARKER} }} .else {{ .echo SENGOO_DEBUG_EXIT_NONZERO; r rdx }}\ng\nq\n",
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
    let named_location = [
        format!("{file_name}:{line}"),
        format!("{file_name}({line})"),
        format!("{file_name} @ {line}"),
        format!("{full_path}:{line}"),
        format!("{full_path}({line})"),
        format!("{full_path} @ {line}"),
    ]
    .into_iter()
    .any(|pattern| normalized_output.contains(&pattern));
    let cdb_source_prompt = normalized_output.lines().any(|candidate| {
        candidate
            .trim_start()
            .strip_prefix('>')
            .is_some_and(|rest| rest.trim_start().starts_with(&format!("{line}:")))
    });
    named_location || cdb_source_prompt
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

fn marker_segment<'a>(output: &'a str, start: &str, end: &str) -> Result<&'a str, String> {
    output_after_marker(output, start)?
        .split_once(end)
        .map(|(segment, _)| segment)
        .ok_or_else(|| format!("debugger did not reach marker `{end}` after `{start}`:\n{output}"))
}

fn segment_has_named_nonzero_value(segment: &str, name: &str) -> bool {
    segment.lines().any(|line| {
        if !line.contains(name) {
            return false;
        }
        let compact = line.replace(' ', "").to_ascii_lowercase();
        let Some((_, value)) = compact.rsplit_once('=') else {
            return false;
        };
        let value = value.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        !value.is_empty() && !matches!(value, "0" | "0x0" | "0n0")
    })
}

fn segment_has_named_zero_value(segment: &str, name: &str) -> bool {
    segment.lines().any(|line| {
        if !line.contains(name) {
            return false;
        }
        let compact = line.replace(' ', "").to_ascii_lowercase();
        let Some((_, value)) = compact.rsplit_once('=') else {
            return false;
        };
        let value = value.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        matches!(value, "0" | "0x0" | "0n0")
    })
}

fn validate_composite_debugger_output(
    output: &str,
    source: &Path,
    layout: &CompositeProbeLayout,
) -> Result<(), String> {
    let composites = marker_segment(output, COMPOSITE_MARKER, CALL_MARKER)?;
    if !contains_source_location(composites, source, layout.composite_line) {
        return Err(format!(
            "debugger did not stop on the composite inspection line:\n{output}"
        ));
    }
    for (name, expected_type) in [
        ("pair", "Pair"),
        ("picked", "enum"),
        ("text", "String"),
        ("values", "Vec_i64"),
    ] {
        if !composites
            .lines()
            .any(|line| line.contains(name) && line.contains(expected_type))
        {
            return Err(format!(
                "debugger did not expose `{name}` with type `{expected_type}`:\n{output}"
            ));
        }
    }
    for (name, decimal, hex) in [("pair.left", 21, "15"), ("picked.discriminant", 1, "1")] {
        if !composites
            .lines()
            .any(|line| line_has_named_value(line, name, decimal, hex))
        {
            return Err(format!(
                "debugger did not expose `{name}` with value {decimal}:\n{output}"
            ));
        }
    }
    if !composites
        .lines()
        .any(|line| line.contains("pair.enabled") && line.to_ascii_lowercase().contains("true"))
    {
        return Err(format!(
            "debugger did not expose `pair.enabled` as true:\n{output}"
        ));
    }
    for name in ["text.handle", "values.handle"] {
        if !segment_has_named_nonzero_value(composites, name) {
            return Err(format!(
                "debugger did not expose `{name}` as a live non-zero value:\n{output}"
            ));
        }
    }
    if !segment_has_named_zero_value(composites, "values.marker") {
        return Err(format!(
            "debugger did not expose `values.marker` as the zero-valued phantom field:\n{output}"
        ));
    }

    let call = marker_segment(output, CALL_MARKER, CALL_STEP_MARKER)?;
    if !contains_source_location(call, source, layout.call_entry_line)
        || !call.contains("call_surface")
        || !call.contains("main")
        || !call
            .lines()
            .any(|line| line_has_named_value(line, "value", 21, "15"))
    {
        return Err(format!(
            "debugger did not expose the call_surface entry stack and live parameter:\n{output}"
        ));
    }
    let call_step = marker_segment(output, CALL_STEP_MARKER, CALL_BODY_MARKER)?;
    if !contains_source_location(call_step, source, layout.helper_entry_line)
        || !call_step.contains("scalar_helper")
        || !call_step
            .lines()
            .any(|line| line_has_named_value(line, "value", 21, "15"))
    {
        return Err(format!(
            "debugger did not step from call_surface into scalar_helper with its live parameter:\n{output}"
        ));
    }
    let call_body = marker_segment(output, CALL_BODY_MARKER, CLOSURE_MARKER)?;
    if !contains_source_location(call_body, source, layout.helper_line)
        || !call_body.contains("scalar_helper")
    {
        return Err(format!(
            "debugger did not step from scalar_helper entry to its first statement:\n{output}"
        ));
    }

    let closure = marker_segment(output, CLOSURE_MARKER, CLOSURE_STEP_MARKER)?;
    if !contains_source_location(closure, source, layout.closure_call_line)
        || !closure.contains("closure_surface")
    {
        return Err(format!(
            "debugger did not stop at the closure invocation:\n{output}"
        ));
    }
    let closure_step = marker_segment(output, CLOSURE_STEP_MARKER, EXIT_MARKER)?;
    if !contains_source_location(closure_step, source, layout.closure_step_line)
        || !closure_step.contains("closure_surface")
    {
        return Err(format!(
            "debugger did not step over the closure invocation to the next statement:\n{output}"
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

fn skip_or_fail(reason: &str) {
    if std::env::var("SENGOO_REQUIRE_NATIVE_DEBUGGER").as_deref() == Ok("1") {
        panic!("required native debugger evidence unavailable: {reason}");
    }
    skip(reason);
}

fn persist_transcript(name: &str, transcript: &str) {
    let Some(directory) = std::env::var_os("SENGOO_DEBUGGER_TRANSCRIPT_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    fs::create_dir_all(&directory).unwrap_or_else(|error| {
        panic!(
            "create debugger transcript directory {}: {error}",
            directory.display()
        )
    });
    let path = directory.join(name);
    fs::write(&path, transcript)
        .unwrap_or_else(|error| panic!("write debugger transcript {}: {error}", path.display()));
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
    assert!(script.contains(".lines -e"));
    assert!(script.contains("bl"));
    assert!(script.contains("dv /t value"));
    assert_eq!(script.lines().filter(|line| *line == "p").count(), 1);
    assert!(script.contains("dv /t doubled"));
    assert!(script.contains(".if (@rdx == 0)"));
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
SENGOO_DEBUG_EXIT_ZERO
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
SENGOO_DEBUG_EXIT_ZERO
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
        "SENGOO_DEBUG_BREAK\ndebugger_probe.sg:2\nvalue = 21\nSENGOO_DEBUG_STEP\ndebugger_probe.sg:2\ndoubled = 42\nSENGOO_DEBUG_EXIT_ZERO",
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

fn composite_probe_layout() -> CompositeProbeLayout {
    CompositeProbeLayout {
        composite_line: source_line_number(COMPOSITE_PROBE_SOURCE, "let observed ="),
        call_entry_line: source_line_number(COMPOSITE_PROBE_SOURCE, "def call_surface"),
        helper_entry_line: source_line_number(COMPOSITE_PROBE_SOURCE, "def scalar_helper"),
        helper_line: source_line_number(COMPOSITE_PROBE_SOURCE, "let adjusted ="),
        closure_call_line: source_line_number(COMPOSITE_PROBE_SOURCE, "let closed = add"),
        closure_step_line: source_line_number(COMPOSITE_PROBE_SOURCE, "let result ="),
    }
}

#[test]
fn lldb_composite_commands_inspect_live_values_calls_and_closures() {
    let layout = composite_probe_layout();
    let args = composite_lldb_arguments(
        Path::new("/tmp/debugger_composite_probe"),
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    );
    let joined = args.join("\n");

    for line in [
        layout.composite_line,
        layout.call_entry_line,
        layout.helper_line,
        layout.closure_call_line,
    ] {
        assert!(joined.contains(&format!(
            "breakpoint set --file debugger_composite_probe.sg --line {line}"
        )));
    }
    for expression in [
        "value",
        "pair",
        "pair.left",
        "pair.enabled",
        "picked",
        "picked.discriminant",
        "text",
        "text.handle",
        "values",
        "values.handle",
        "values.marker",
    ] {
        assert!(
            joined.contains(&format!("frame variable {expression}")),
            "missing live inspection for {expression}:\n{joined}"
        );
    }
    assert!(joined.contains("thread backtrace"));
    assert!(joined.contains("step"));
    assert!(joined.contains("finish"));
    assert!(joined.contains("next"));
    for marker in [
        COMPOSITE_MARKER,
        CALL_MARKER,
        CALL_STEP_MARKER,
        CALL_BODY_MARKER,
        CLOSURE_MARKER,
        CLOSURE_STEP_MARKER,
        EXIT_MARKER,
    ] {
        assert!(
            joined.contains(marker),
            "missing marker {marker}:\n{joined}"
        );
    }
}

#[test]
fn composite_debugger_output_requires_live_layouts_and_surface_steps() {
    let layout = composite_probe_layout();
    let transcript = format!(
        r#"
Breakpoint 1: debugger_composite_probe.sg:{composite_line}:5
{COMPOSITE_MARKER}
frame #0: inspect_composites at debugger_composite_probe.sg:{composite_line}:5
(Pair) pair = (left = 21, enabled = true)
(i64) pair.left = 21
(bool) pair.enabled = true
(enum) picked = (discriminant = 1, payload = {{...}})
(i64) picked.discriminant = 1
(String) text = (handle = 14)
(i64) text.handle = 14
(Vec_i64) values = (handle = 18, marker = 0)
(i64) values.handle = 18
(i64) values.marker = 0
{CALL_MARKER}
frame #0: call_surface at debugger_composite_probe.sg:{call_entry_line}:5
thread backtrace:
frame #0: call_surface at debugger_composite_probe.sg:{call_entry_line}:5
frame #1: main
(i64) value = 21
{CALL_STEP_MARKER}
frame #0: scalar_helper at debugger_composite_probe.sg:{helper_entry_line}:5
(i64) value = 21
{CALL_BODY_MARKER}
frame #0: scalar_helper at debugger_composite_probe.sg:{helper_line}:5
{CLOSURE_MARKER}
frame #0: closure_surface at debugger_composite_probe.sg:{closure_call_line}:5
{CLOSURE_STEP_MARKER}
frame #0: closure_surface at debugger_composite_probe.sg:{closure_step_line}:5
{EXIT_MARKER}
"#,
        composite_line = layout.composite_line,
        call_entry_line = layout.call_entry_line,
        helper_entry_line = layout.helper_entry_line,
        helper_line = layout.helper_line,
        closure_call_line = layout.closure_call_line,
        closure_step_line = layout.closure_step_line,
    );

    assert!(validate_composite_debugger_output(
        &transcript,
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    )
    .is_ok());

    let missing_member = transcript.replace("(i64) pair.left = 21\n", "");
    assert!(validate_composite_debugger_output(
        &missing_member,
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    )
    .is_err());

    let zero_handle = transcript.replace("(i64) text.handle = 14", "(i64) text.handle = 0");
    assert!(validate_composite_debugger_output(
        &zero_handle,
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    )
    .is_err());

    let wrong_marker = transcript.replace("(i64) values.marker = 0", "(i64) values.marker = 7");
    assert!(validate_composite_debugger_output(
        &wrong_marker,
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    )
    .is_err());

    let wrong_closure_line = transcript.replace(
        &format!(
            "debugger_composite_probe.sg:{}:5\n{EXIT_MARKER}",
            layout.closure_step_line
        ),
        &format!(
            "debugger_composite_probe.sg:{}:5\n{EXIT_MARKER}",
            layout.closure_call_line
        ),
    );
    assert!(validate_composite_debugger_output(
        &wrong_closure_line,
        Path::new("/tmp/debugger_composite_probe.sg"),
        &layout,
    )
    .is_err());
}

#[test]
fn native_debugger_breaks_steps_and_reads_local() {
    let layout = probe_layout();
    let Some((flavor, debugger_name)) = host_debugger() else {
        skip_or_fail("host platform has no supported LLDB/CDB driver");
        return;
    };
    let Some(debugger) = find_tool(debugger_name) else {
        skip_or_fail(&format!(
            "required debugger `{debugger_name}` was not found on PATH"
        ));
        return;
    };
    if find_tool(if cfg!(windows) { "clang.exe" } else { "clang" }).is_none() {
        skip_or_fail("native clang toolchain was not found on PATH");
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
    #[cfg(windows)]
    assert!(
        executable.with_extension("pdb").is_file(),
        "sgc did not create the CodeView PDB beside {}\nbuild output:\n{}",
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
    persist_transcript(
        &format!("debugger-native-{debugger_name}-scalar.txt"),
        &transcript,
    );
    assert!(
        output.status.success(),
        "{debugger_name} batch session failed:\n{transcript}"
    );
    validate_debugger_output(&transcript, &source, &layout)
        .unwrap_or_else(|error| panic!("{debugger_name} validation failed: {error}"));
}

#[test]
fn composite_probe_builds_for_native_debugging() {
    let project = TempProject::new();
    let source = project.composite_source_path();
    fs::write(&source, COMPOSITE_PROBE_SOURCE).expect("write composite debugger probe source");

    let check = Command::new(sgc())
        .arg("check")
        .arg(&source)
        .output()
        .expect("check composite debugger probe");
    assert!(
        check.status.success(),
        "composite debugger probe did not type-check:\n{}",
        combined_output(&check)
    );
}

#[test]
#[cfg(unix)]
fn native_lldb_steps_and_inspects_composite_surfaces() {
    let Some(debugger) = find_tool("lldb") else {
        skip_or_fail("required debugger `lldb` was not found on PATH");
        return;
    };
    if find_tool("clang").is_none() {
        skip_or_fail("native clang toolchain was not found on PATH");
        return;
    }

    let layout = composite_probe_layout();
    let project = TempProject::new();
    let source = project.composite_source_path();
    fs::write(&source, COMPOSITE_PROBE_SOURCE).expect("write composite debugger probe source");

    let build = Command::new(sgc())
        .arg("build")
        .arg(&source)
        .args(["-O", "0", "--debug-info", "--force-rebuild"])
        .output()
        .expect("run composite sgc debug build");
    assert!(
        build.status.success(),
        "composite sgc debug build failed:\n{}",
        combined_output(&build)
    );

    let executable = project.composite_executable_path();
    assert!(
        executable.is_file(),
        "sgc did not create expected composite debug executable {}\nbuild output:\n{}",
        executable.display(),
        combined_output(&build)
    );

    let output = Command::new(debugger)
        .args(composite_lldb_arguments(&executable, &source, &layout))
        .output()
        .expect("run composite LLDB batch session");
    let transcript = combined_output(&output);
    persist_transcript("debugger-native-lldb-composites.txt", &transcript);
    assert!(
        output.status.success(),
        "LLDB composite batch session failed:\n{transcript}"
    );
    validate_composite_debugger_output(&transcript, &source, &layout)
        .unwrap_or_else(|error| panic!("LLDB composite validation failed: {error}"));
}

#[test]
#[cfg(windows)]
fn debug_build_cache_recovery_recreates_the_pdb() {
    if find_tool("clang.exe").is_none() {
        skip("native clang toolchain was not found on PATH");
        return;
    }

    let project = TempProject::new();
    let source = project.source_path();
    fs::write(&source, PROBE_SOURCE).expect("write debugger cache-recovery probe source");

    let initial = Command::new(sgc())
        .arg("build")
        .arg(&source)
        .args(["-O", "0", "--debug-info", "--force-rebuild"])
        .output()
        .expect("run initial debug build");
    assert!(
        initial.status.success(),
        "initial sgc debug build failed:\n{}",
        combined_output(&initial)
    );

    let executable = project.executable_path();
    let pdb = executable.with_extension("pdb");
    assert!(executable.is_file() && pdb.is_file());
    fs::remove_file(&executable).expect("remove cached debug executable");
    fs::remove_file(&pdb).expect("remove cached debug PDB");

    let recovered = Command::new(sgc())
        .arg("build")
        .arg(&source)
        .args(["-O", "0", "--debug-info"])
        .output()
        .expect("recover debug build from cached artifacts");
    assert!(
        recovered.status.success(),
        "cached sgc debug recovery failed:\n{}",
        combined_output(&recovered)
    );
    assert!(
        executable.is_file() && pdb.is_file(),
        "debug cache recovery should recreate both executable and PDB:\n{}",
        combined_output(&recovered)
    );
}
