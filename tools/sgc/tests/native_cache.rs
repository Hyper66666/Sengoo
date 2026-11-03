//! A top-level executable cache hit must not conceal changed native inputs.
mod common;

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cached_build_and_run_relink_when_a_transitive_native_header_changes() {
    let directory = std::env::temp_dir().join(format!(
        "sengoo-native-cache-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::create_dir_all(directory.join("native/headers with spaces")).unwrap();
    let fixture = Fixture(directory);
    fs::write(fixture.0.join("Sengoo.toml"), "[package]\nname = \"cache_probe\"\nversion = \"0.1.0\"\nedition = \"2026\"\n[bin]\npath = \"src/main.sg\"\n").unwrap();
    let source = fixture.0.join("src/main.sg");
    fs::write(
        &source,
        "extern \"C\" { fn native_value() -> i64; }\ndef main() -> i64 { native_value() }\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("native/bridge.c"),
        "#include \"outer.h\"\nlong long native_value(void) { return VALUE; }\n",
    )
    .unwrap();
    fs::write(
        fixture.0.join("native/outer.h"),
        "#include \"headers with spaces/value.h\"\n",
    )
    .unwrap();
    let header = fixture.0.join("native/headers with spaces/value.h");
    fs::write(&header, "#define VALUE 41\n").unwrap();
    let executable = fixture
        .0
        .join(if cfg!(windows) { "probe.exe" } else { "probe" });
    let build = || {
        let output = common::source_sgc_command()
            .arg("build")
            .arg(&source)
            .arg("-O0")
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    build();
    assert_eq!(Command::new(&executable).status().unwrap().code(), Some(41));
    assert!(
        build().contains("build cache hit"),
        "unchanged native inputs should reuse the executable"
    );
    let modified = fs::metadata(&header).unwrap().modified().unwrap();
    fs::write(&header, "#define VALUE 42\n").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&header)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    build();
    assert_eq!(
        Command::new(&executable).status().unwrap().code(),
        Some(42),
        "a top-level cache must not hide a header change"
    );
    let run = || {
        common::source_sgc_command()
            .arg("run")
            .arg(&source)
            .arg("-O0")
            .output()
            .unwrap()
    };
    assert_eq!(run().status.code(), Some(42));
    fs::write(&header, "#define VALUE 43\n").unwrap();
    assert_eq!(
        run().status.code(),
        Some(43),
        "the run cache must also see native header changes"
    );
}
