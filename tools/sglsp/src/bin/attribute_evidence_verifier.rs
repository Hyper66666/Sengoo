use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EvidenceEntry {
    id: String,
    package: String,
    filter: String,
    expected: String,
}

fn verify_test_output(expected: &str, success: bool, output: &str) -> Result<(), String> {
    if !success {
        return Err("cargo test failed".into());
    }
    let success_line = format!("test {expected} ... ok");
    if !output.lines().any(|line| line.trim() == success_line) {
        return Err(format!(
            "expected runnable evidence `{expected}` was missing, ignored, or filtered"
        ));
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let manifest = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tools/sglsp/attribute-evidence.json"));
    let source = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("failed to read {}: {error}", manifest.display()))?;
    let entries: Vec<EvidenceEntry> =
        serde_json::from_str(&source).map_err(|error| format!("invalid manifest: {error}"))?;
    let mut ids = std::collections::HashSet::new();
    for entry in entries {
        if !ids.insert(entry.id.clone()) {
            return Err(format!("duplicate evidence id: {}", entry.id));
        }
        let output = Command::new("cargo")
            .args([
                "test",
                "-p",
                &entry.package,
                &entry.filter,
                "--",
                "--nocapture",
            ])
            .output()
            .map_err(|error| format!("failed to start cargo for {}: {error}", entry.id))?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        verify_test_output(&entry.expected, output.status.success(), &combined)
            .map_err(|error| format!("{}: {error}\n{combined}", entry.id))?;
        println!("evidence ok: {}", entry.id);
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_gate_rejects_missing_ignored_and_failed_evidence() {
        let expected = "tests::owner::capability";
        assert!(verify_test_output(expected, true, "test tests::owner::capability ... ok").is_ok());
        assert!(verify_test_output(expected, true, "0 tests").is_err());
        assert!(
            verify_test_output(expected, true, "test tests::owner::capability ... ignored")
                .is_err()
        );
        assert!(verify_test_output(expected, false, "test failed").is_err());
    }
}
