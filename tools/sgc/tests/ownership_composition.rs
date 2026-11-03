//! Native ownership checks use both observable Drop events (detect duplicates)
//! and runtime handle counts (detect leaks, including idempotent-release paths).
mod common;

use common::source_sgc_command;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Source {
    root: PathBuf,
    path: PathBuf,
}

impl Source {
    fn new(name: &str, source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "sengoo-ownership-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("main.sg");
        fs::write(&path, source).unwrap();
        Self { root, path }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "ownership failure source, logs and generated IR retained at {}",
                self.root.display()
            );
            return;
        }
        assert!(self.root.starts_with(std::env::temp_dir()));
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("sengoo-ownership-"));
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn native(name: &str, source: &str, expected_drops: &[i64]) {
    let source = Source::new(name, source);
    let output = source_sgc_command()
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .output()
        .unwrap();
    fs::write(source.root.join("stdout.txt"), &output.stdout).unwrap();
    fs::write(source.root.join("stderr.txt"), &output.stderr).unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{name}: native execution failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let actual = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<i64>().ok())
        .fold(BTreeMap::new(), |mut counts, id| {
            *counts.entry(id).or_insert(0usize) += 1;
            counts
        });
    let expected = expected_drops
        .iter()
        .fold(BTreeMap::new(), |mut counts, id| {
            *counts.entry(*id).or_insert(0usize) += 1;
            counts
        });
    assert_eq!(
        actual, expected,
        "{name}: every owned value must drop exactly once"
    );
}

const PROBE: &str = r#"
struct Owned { id: i64 }
impl Drop for Owned { def drop(&mut self) { print(self.id); } }
"#;

#[test]
fn returned_hashmap_and_rc_aggregates_release_typed_owned_payloads() {
    native(
        "returned-map-rc",
        r#"
import std::collections;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
struct Owned { id: i64, text: String }
impl Drop for Owned { def drop(&mut self) { print(self.id); } }
struct Bundle { map: HashMap<i64, Option<Owned>>, shared: Rc<Option<Owned>> }
def owner(id: i64) -> Result<Owned, i64> {
    let text = string_from_str("map-rc-世界")?;
    result_ok_with(Owned { id: id, text: text }, 0)
}
def bundle() -> Result<Bundle, i64> {
    let map: HashMap<i64, Option<Owned>> = hashmap_new();
    map.insert(1, option_some(owner(901)?));
    let shared: Rc<Option<Owned>> = rc_new(option_some(owner(902)?));
    result_ok_with(Bundle { map: map, shared: shared }, 0)
}
def consume(fail: bool) -> Result<i64, i64> {
    let value = bundle()?;
    let copied = value.shared.clone();
    let gate = if fail { result_err_i64(9) } else { result_ok_i64(1) };
    gate?;
    result_ok_i64(value.map.len() + copied.strong_count())
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let failed = consume(true);
    let succeeded = consume(false);
    let after = sengoo_string_live_handle_count();
    if before == after && failed.error == 9 && succeeded.value == 3 { 0 } else { 1 }
}
"#,
        &[901, 902, 901, 902],
    );
}

#[test]
fn user_destructors_release_remaining_owned_fields() {
    native(
        "destructor-fields",
        r#"
import std::string;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
struct Owned { id: i64, text: String }
impl Drop for Owned { def drop(&mut self) { print(self.id); } }
def exercise() -> i64 {
    let value = Owned { id: 801, text: string_from_str("destructor-field").unwrap_or(String { handle: 0 }) };
    value.text.len()
}

def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let size = exercise();
    let after = sengoo_string_live_handle_count();
    if size == 16 && before == after { 0 } else { 1 }
}
"#,
        &[801],
    );
}

#[test]
fn explicit_field_drop_free_and_close_suppress_automatic_field_cleanup() {
    native(
        "explicit-destructor-fields",
        r#"
struct Child { id: i64 }
impl Drop for Child { def drop(&mut self) { print(self.id); } }
impl Child {
    def free(&mut self) { print(self.id); }
    def close(&mut self) { print(self.id); }
}
struct ViaDrop { child: Child }
impl Drop for ViaDrop { def drop(&mut self) { self.child.drop(); } }
struct ViaFree { child: Child }
impl Drop for ViaFree { def drop(&mut self) { self.child.free(); } }
struct ViaClose { child: Child }
impl Drop for ViaClose { def drop(&mut self) { self.child.close(); } }
def main() -> i64 {
    let first = ViaDrop { child: Child { id: 831 } };
    let second = ViaFree { child: Child { id: 832 } };
    let third = ViaClose { child: Child { id: 833 } };
    first.child.id + second.child.id + third.child.id - 2496
}
"#,
        &[831, 832, 833],
    );
}

#[test]
fn returned_aggregate_collections_and_nested_descriptors_drop_on_early_return() {
    native(
        "returned-collections",
        r#"
import std::collections;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
struct Owned { id: i64, text: String }
impl Drop for Owned { def drop(&mut self) { print(self.id); } }
struct Bundle { direct: Vec<Owned>, optional: Vec<Option<Owned>>, children: Vec<Vec<Owned>> }
def owner(id: i64) -> Result<Owned, i64> {
    let text = string_from_str("owned")?;
    Result { is_ok: true, value: Owned { id: id, text: text }, error: 0 }
}
def bundle() -> Result<Bundle, i64> {
    let direct: Vec<Owned> = vec_new();
    direct.push(owner(811)?);
    let optional: Vec<Option<Owned>> = vec_new();
    optional.push(option_some(owner(812)?));
    let inner: Vec<Owned> = vec_new();
    inner.push(owner(813)?);
    let children: Vec<Vec<Owned>> = vec_new();
    children.push(inner);
    Result { is_ok: true, value: Bundle { direct: direct, optional: optional, children: children }, error: 0 }
}
def consume(fail: bool) -> Result<i64, i64> {
    let value = bundle()?;
    let gate: Result<i64, i64> = if fail { result_err_i64(9) } else { result_ok_i64(1) };
    gate?;
    result_ok_i64(value.direct.len() + value.optional.len() + value.children.len())
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let failed = consume(true);
    let succeeded = consume(false);
    let after = sengoo_string_live_handle_count();
    if before == after && failed.error == 9 && succeeded.value == 3 { 0 } else { 1 }
}
"#,
        &[811, 812, 813, 811, 812, 813],
    );
}

#[test]
fn nested_owned_fields_and_partial_move_drop_exactly_once() {
    native(
        "nested-partial",
        &format!(
            r#"{PROBE}
struct Inner {{ first: Owned, second: Owned }}
struct Outer {{ inner: Inner, last: Owned }}
def exercise() -> i64 {{
    let outer = Outer {{ inner: Inner {{ first: Owned {{ id: 101 }}, second: Owned {{ id: 102 }} }}, last: Owned {{ id: 103 }} }};
    let moved = outer.last;
    let read = &outer.inner;
    (*read).first.id + moved.id
}}
def main() -> i64 {{ if exercise() == 204 {{ 0 }} else {{ 1 }} }}
"#
        ),
        &[101, 102, 103],
    );
}

#[test]
fn generic_vector_and_option_result_wrappers_drop_exactly_once() {
    native(
        "vec-wrappers",
        &format!(
            r#"
import std::collections;
{PROBE}
def exercise() -> i64 {{
    let values: Vec<Owned> = vec_new();
    values.push(Owned {{ id: 201 }});
    values.push(Owned {{ id: 202 }});
    let first = values.pop().unwrap_or(Owned {{ id: 203 }});
    let wrapped: Option<Result<Owned, i64>> = option_some(result_ok_with(Owned {{ id: 204 }}, 0));
    first.id + wrapped.value.value.id
}}
def main() -> i64 {{ if exercise() == 406 {{ 0 }} else {{ 1 }} }}
"#
        ),
        &[201, 202, 203, 204],
    );
}

#[test]
fn branches_loop_break_continue_and_returns_drop_exactly_once() {
    native(
        "control-exits",
        &format!(
            r#"{PROBE}
def choose(early: bool) -> i64 {{
    let outer = Owned {{ id: 301 }};
    let branch = if early {{ Owned {{ id: 302 }} }} else {{ Owned {{ id: 303 }} }};
    let mut index = 0;
    while index < 3 {{
        let iteration = Owned {{ id: 310 + index }};
        index += 1;
        if index == 1 {{ continue; }}
        if index == 2 {{ break; }}
    }}
    if early {{ return branch.id; }}
    branch.id
}}
def main() -> i64 {{ if choose(true) + choose(false) == 605 {{ 0 }} else {{ 1 }} }}
"#
        ),
        &[301, 301, 302, 303, 310, 310, 311, 311],
    );
}

#[test]
fn question_mark_and_explicit_return_release_nested_owned_locals() {
    native(
        "question-return",
        &format!(
            r#"
import std::result;
{PROBE}
struct Wrapper {{ value: Owned }}
def checked(fail: bool) -> Result<i64, i64> {{
    let wrapped = Wrapper {{ value: Owned {{ id: 401 }} }};
    let result = if fail {{ result_err_i64(9) }} else {{ result_ok_i64(7) }};
    let value = result?;
    return result_ok_i64(value);
}}
def main() -> i64 {{
    let failed = checked(true);
    let passed = checked(false);
    if failed.error == 9 && passed.value == 7 {{ 0 }} else {{ 1 }}
}}
"#
        ),
        &[401, 401],
    );
}

#[test]
fn closure_capture_and_by_value_parameter_drop_exactly_once() {
    native(
        "closure",
        &format!(
            r#"{PROBE}
def main() -> i64 {{
    let captured = 501;
    let callback: fn(Owned) -> i64 = |value| captured + value.id;
    let result = callback(Owned {{ id: 502 }});
    if result == 1003 {{ 0 }} else {{ 1 }}
}}
"#
        ),
        &[502],
    );
}

#[test]
fn second_question_mark_preserves_cleanup_of_first_owned_success() {
    native(
        "second-question",
        &format!(
            r#"
import std::result;
{PROBE}
def construct() -> Result<Owned, i64> {{ result_ok_with(Owned {{ id: 451 }}, 0) }}
def exercise(fail: bool) -> Result<Owned, i64> {{
    let owned = construct()?;
    let checked = if fail {{ result_err_i64(9) }} else {{ result_ok_i64(7) }};
    checked?;
    result_ok_with(owned, 0)
}}
def main() -> i64 {{
    let failed = exercise(true);
    let succeeded = exercise(false);
    if failed.error == 9 && succeeded.value.id == 451 {{ 0 }} else {{ 1 }}
}}
"#
        ),
        &[451, 451],
    );
}

#[test]
fn option_result_owned_string_fallbacks_release_on_both_branches() {
    native(
        "string-fallbacks",
        r#"
import std::string;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
def owned(text: &str) -> String { string_from_str(text).unwrap_or(String { handle: 0 }) }
def exercise() -> i64 {
    let present_value = option_some(owned("some")).unwrap_or(owned("unused-option"));
    let absent_value: Option<String> = Option { is_some: false, value: String { handle: 0 } };
    let option_fallback = absent_value.unwrap_or(owned("used-option"));
    let successful_value = result_ok_with(owned("ok"), 0).unwrap_or(owned("unused-result"));
    let failure_value: Result<String, i64> = result_err_with(String { handle: 0 }, 9);
    let result_fallback = failure_value.unwrap_or(owned("used-result"));
    present_value.len() + option_fallback.len() + successful_value.len() + result_fallback.len()
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let count = exercise();
    let after = sengoo_string_live_handle_count();
    if before == after && count > 0 { 0 } else { 1 }
}
"#,
        &[],
    );
}

#[test]
fn nested_string_vector_and_early_error_leave_no_live_handles() {
    native(
        "string-handles",
        r#"
import std::collections;
extern "C" { fn sengoo_string_live_handle_count() -> i64; }
struct Inner { text: String }
struct Outer { inner: Inner, sibling: String }
def exercise(fail: bool) -> Result<i64, i64> {
    let nested = Outer {
        inner: Inner { text: string_from_str("nested").unwrap_or(String { handle: 0 }) },
        sibling: string_from_str("sibling").unwrap_or(String { handle: 0 }),
    };
    let moved = nested.sibling;
    let values: Vec<Inner> = vec_new();
    values.push(Inner { text: string_from_str("vector").unwrap_or(String { handle: 0 }) });
    let wrapped = option_some(result_ok_with(Inner { text: string_from_str("wrapped").unwrap_or(String { handle: 0 }) }, 0));
    let step = if fail { result_err_i64(9) } else { result_ok_i64(7) };
    let checked = step?;
    result_ok_i64(checked + moved.len() + wrapped.value.value.text.len())
}
def main() -> i64 {
    let before = sengoo_string_live_handle_count();
    let failed = exercise(true);
    let passed = exercise(false);
    let after = sengoo_string_live_handle_count();
    if before == after && failed.error == 9 && passed.value == 21 { 0 } else { 1 }
}
"#,
        &[],
    );
}

#[test]
fn result_unit_is_stored_as_a_zero_sized_value() {
    native(
        "result-unit",
        r#"
import std::result;
def complete() -> Result<(), i64> { result_ok_with((), 0) }
def main() -> i64 { let result = complete(); if result.is_ok { 0 } else { 1 } }
"#,
        &[],
    );
}

#[test]
fn async_cancellation_rejects_owned_frame_until_drop_state_is_supported() {
    let source = Source::new(
        "async-cancel",
        &format!(
            r#"{PROBE}
async def worker() -> i64 {{
    let owned = Owned {{ id: 601 }};
    await sleep(10000);
    owned.id
}}
async def main() -> i64 {{
    let task = spawn_task(worker());
    await sleep(5);
    let canceled = cancel_task(task);
    await sleep(5);
    if canceled {{ 0 }} else {{ 1 }}
}}
"#
        ),
    );
    let output = source_sgc_command()
        .arg("check")
        .arg(&source.path)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "an owning canceled frame must not compile into a resource leak"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported-owned-async-cancellation"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn async_normal_completion_preserves_owned_cleanup_with_unrelated_cancellation() {
    native(
        "async-complete",
        &format!(
            r#"{PROBE}
async def worker() -> i64 {{
    let owned = Owned {{ id: 611 }};
    await sleep(1);
    owned.id
}}
async def resource_free() -> i64 {{ await sleep(10000); 0 }}
async def main() -> i64 {{
    let task = spawn_task(resource_free());
    let canceled = cancel_task(task);
    let value = await worker();
    if canceled && value == 611 {{ 0 }} else {{ 1 }}
}}
"#
        ),
        &[611],
    );
}

#[test]
fn aggregate_closure_capture_has_a_stable_diagnostic() {
    let source = Source::new(
        "owned-capture",
        &format!(
            r#"{PROBE}
def main() -> i64 {{
    let owned = Owned {{ id: 701 }};
    let callback: fn(i64) -> i64 = |value| owned.id + value;
    callback(1)
}}
"#
        ),
    );
    let output = source_sgc_command()
        .arg("check")
        .arg(&source.path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported-closure-capture"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
