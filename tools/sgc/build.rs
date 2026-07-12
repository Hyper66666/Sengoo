use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=SENGOO_BUILD_HASH");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let hash = std::env::var("SENGOO_BUILD_HASH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(git_short_hash)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=SENGOO_BUILD_HASH={hash}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Windows executables default to a 1 MiB main-thread stack. Parsing and
        // specializing the full stdlib surface legitimately exceeds that on
        // large modules, while Unix hosts normally provide a much larger stack.
        println!("cargo:rustc-link-arg-bin=sgc=/STACK:16777216");
    }
}

fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let hash = hash.trim();
    (!hash.is_empty()).then(|| hash.to_string())
}
