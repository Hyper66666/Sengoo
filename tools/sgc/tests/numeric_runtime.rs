use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempSource {
    root: PathBuf,
    path: PathBuf,
}

impl TempSource {
    fn new(source: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sengoo-numeric-runtime-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create numeric runtime test directory");
        let path = root.join("main.sg");
        fs::write(&path, source).expect("write numeric runtime source");
        Self { root, path }
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn every_integer_width_executes_checked_wrapping_and_saturating_methods() {
    let source = TempSource::new(
        r#"
import std::math;

def main() -> i64 {
    let i8_ok = (127i8).checked_add(1i8).is_none()
        && (127i8).wrapping_add(1i8) == -128i8
        && (127i8).saturating_add(1i8) == 127i8;
    let i16_ok = (32767i16).checked_add(1i16).is_none()
        && (32767i16).wrapping_add(1i16) == -32768i16
        && (32767i16).saturating_add(1i16) == 32767i16;
    let i32_ok = (2147483647i32).checked_add(1i32).is_none()
        && (2147483647i32).wrapping_add(1i32) == -2147483648i32
        && (2147483647i32).saturating_add(1i32) == 2147483647i32;
    let i64_ok = (9223372036854775807i64).checked_add(1i64).is_none()
        && (9223372036854775807i64).wrapping_add(1i64) == -9223372036854775808i64
        && (9223372036854775807i64).saturating_add(1i64) == 9223372036854775807i64;
    let u8_ok = (255u8).checked_add(1u8).is_none()
        && (255u8).wrapping_add(1u8) == 0u8
        && (255u8).saturating_add(1u8) == 255u8;
    let u16_ok = (65535u16).checked_add(1u16).is_none()
        && (65535u16).wrapping_add(1u16) == 0u16
        && (65535u16).saturating_add(1u16) == 65535u16;
    let u32_ok = (4294967295u32).checked_add(1u32).is_none()
        && (4294967295u32).wrapping_add(1u32) == 0u32
        && (4294967295u32).saturating_add(1u32) == 4294967295u32;
    let u64_ok = (18446744073709551615u64).checked_add(1u64).is_none()
        && (18446744073709551615u64).wrapping_add(1u64) == 0u64
        && (18446744073709551615u64).saturating_add(1u64) == 18446744073709551615u64;
    let isize_ok = (9223372036854775807isize).checked_add(1isize).is_none()
        && (9223372036854775807isize).wrapping_add(1isize) == -9223372036854775808isize
        && (9223372036854775807isize).saturating_add(1isize) == 9223372036854775807isize;
    let usize_ok = (18446744073709551615usize).checked_add(1usize).is_none()
        && (18446744073709551615usize).wrapping_add(1usize) == 0usize
        && (18446744073709551615usize).saturating_add(1usize) == 18446744073709551615usize;
    let checked_u64_ok = checked_u64_to_i64(9223372036854775807u64).is_ok
        && !checked_u64_to_i64(9223372036854775808u64).is_ok;

    if i8_ok && i16_ok && i32_ok && i64_ok
        && u8_ok && u16_ok && u32_ok && u64_ok
        && isize_ok && usize_ok && checked_u64_ok {
        0
    } else {
        1
    }
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .arg("-O2")
        .output()
        .expect("sgc should launch");

    assert_eq!(
        output.status.code(),
        Some(0),
        "numeric runtime fixture failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn float_to_integer_casts_saturate_nan_infinity_and_bounds() {
    let source = TempSource::new(
        r#"
def main() -> i64 {
    let nan = 0.0 / 0.0;
    let positive_infinity = 1.0 / 0.0;
    let negative_infinity = -1.0 / 0.0;
    let ok = (nan as i32) == 0i32
        && (positive_infinity as i8) == 127i8
        && (negative_infinity as i8) == -128i8
        && (-1.0 as u8) == 0u8
        && (300.0 as u8) == 255u8;
    if ok { 0 } else { 1 }
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_sgc"))
        .arg("run")
        .arg(&source.path)
        .arg("--force-rebuild")
        .arg("-O2")
        .output()
        .expect("sgc should launch");

    assert_eq!(
        output.status.code(),
        Some(0),
        "float cast fixture failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
