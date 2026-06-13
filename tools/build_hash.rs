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
    if hash.is_empty() {
        None
    } else {
        Some(hash.to_string())
    }
}
