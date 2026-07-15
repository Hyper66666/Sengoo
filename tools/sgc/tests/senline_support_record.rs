use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sgc crate should live under tools/sgc")
        .to_path_buf()
}

#[test]
fn sengoo_support_record_keeps_local_package_evidence_separate_from_senline_authority() {
    let path = repo_root().join("docs/senline-dogfood-support.md");
    let record = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));

    for required in [
        "source-development-local",
        "installed-windows-x64",
        "installed-linux-x64",
        "senline-domain-worker",
        "senline-http-dogfood",
        "sandbox and supervisor",
        "shadow",
        "guarded-development",
        "internal-alpha",
        "rollback",
        "Senline Rust",
        "not Senline pin evidence",
        "release_eligible=false",
        "authentication authority",
        "replay mutation",
        "prekey claim",
        "public ingress",
        "final mutation authority",
        "separate reviewed OpenSpec change",
    ] {
        assert!(
            record.contains(required),
            "support record must preserve `{required}`"
        );
    }

    assert!(
        record.contains("| source-development-local | proven |"),
        "the current local source worker evidence must be explicit"
    );
    // Installed hosts may be `pending` or intermediate `package-smoke-proven`
    // after distribution packaging, but must not claim full product-loop proven
    // until worker/HTTP installed gates and pin verification land.
    let windows_status_ok = record.contains("| installed-windows-x64 | pending |")
        || record.contains("| installed-windows-x64 | package-smoke-proven |");
    let linux_status_ok = record.contains("| installed-linux-x64 | pending |")
        || record.contains("| installed-linux-x64 | package-smoke-proven |");
    assert!(
        windows_status_ok && linux_status_ok,
        "installed platform rows must stay pending or package-smoke-proven only"
    );
    assert!(
        record.contains("Installed `senline-domain-worker` / HTTP product loops")
            || record.contains("Installed worker/HTTP product loops"),
        "installed package-smoke rows must still call out pending product loops"
    );
    assert!(
        record.contains("| sandbox and supervisor | Senline-owned |")
            && record.contains("| internal-alpha | Senline-owned |"),
        "host authority must not be presented as a Sengoo support claim"
    );

    for forbidden in [
        "| installed-windows-x64 | proven |",
        "| installed-linux-x64 | proven |",
        "| sandbox and supervisor | proven |",
        "| internal-alpha | proven |",
        "| production ingress | proven |",
    ] {
        assert!(
            !record.contains(forbidden),
            "support record overclaims `{forbidden}`"
        );
    }
}
