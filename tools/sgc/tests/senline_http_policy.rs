//! Tasks 6.6 / 6.7: harness non-ingress policy and anti-deployment checks.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn http_root() -> PathBuf {
    workspace_root().join("examples/realworld/senline-http-dogfood")
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).expect("read file")
}

/// Text extensions scanned for harness anti-targeting (includes real Windows /
/// Android / web client surfaces, not just Sengoo/docs sources).
const SCAN_EXTENSIONS: &[&str] = &[
    "sg", "md", "toml", "json", "rs", "yml", "yaml", // Sengoo / docs / config
    "kt", "kts", "gradle", "xml", // Android
    "cs", "xaml", "csproj", "props", "targets", // Windows / .NET
    "ts", "tsx", "js", "jsx", // web / desktop clients
    "swift", "m", "mm", // Apple clients (if present)
    "plist", "pbxproj",
];

fn is_scanned_extension(ext: &str) -> bool {
    SCAN_EXTENSIONS
        .iter()
        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
}

fn walk_text_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "target" || name == "build" || name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_text_files(&path, out);
            continue;
        }
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if is_scanned_extension(ext) {
                out.push(path);
            }
        }
    }
}

#[test]
fn http_dogfood_documents_serial_plaintext_non_ingress_limits() {
    let readme = read(http_root().join("README.md"));
    for needle in [
        "serial",
        "plaintext",
        "Connection: close",
        "TLS",
        "keep-alive",
        "internal-alpha",
        "production",
        "127.0.0.1:0",
    ] {
        assert!(
            readme.contains(needle),
            "README missing retained-limit claim: {needle}"
        );
    }
}

#[test]
fn http_dogfood_sources_never_advertise_non_loopback_or_client_endpoints() {
    let mut files = Vec::new();
    walk_text_files(&http_root(), &mut files);
    assert!(!files.is_empty(), "expected http dogfood source files");
    let forbidden = ["0.0.0.0", "SENLINE_API", "play.google", "apps/chat_client"];
    for path in files {
        let text = read(&path);
        // Policy tests and README intentionally mention rejected non-loopback inputs.
        if path.ends_with("policy_contract.sg") || path.ends_with("README.md") {
            continue;
        }
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{} must not target or embed production/client endpoint material containing {needle}",
                path.display()
            );
        }
        assert!(
            !text.contains("https://"),
            "{} must not embed remote HTTPS endpoints",
            path.display()
        );
    }
}

/// Product-surface roots under a Senline checkout that must not target dogfood.
const SENLINE_PRODUCT_ROOTS: &[&str] = &["apps", "win", "config", "services", "android", "clients"];

fn harness_markers() -> [&'static str; 4] {
    [
        "senline-http-dogfood",
        "senline_http_dogfood",
        "/v1/submit-envelope",
        "READY 127.0.0.1",
    ]
}

fn product_surface_hits_harness(text: &str) -> Option<&'static str> {
    let markers = harness_markers();
    for marker in markers {
        if !text.contains(marker) {
            continue;
        }
        if marker == "/v1/submit-envelope" {
            // Operation path alone is ambiguous only when no dogfood identity is
            // present. Fail when the same file also names the dogfood package or
            // READY loopback banner (client wiring the harness).
            let dogfood_identity = text.contains("senline-http-dogfood")
                || text.contains("senline_http_dogfood")
                || text.contains("READY 127.0.0.1");
            if !dogfood_identity {
                continue;
            }
        }
        return Some(marker);
    }
    None
}

#[test]
fn optional_senline_checkout_does_not_target_http_dogfood_harness() {
    // Prefer SENLINE_ROOT; fall back to D:\senline. When absent on a runner that
    // is not expected to carry Senline, skip — but the scanned extension set and
    // marker rules still apply whenever a checkout is present (fail-closed).
    let senline = std::env::var_os("SENLINE_ROOT")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| {
            let fallback = PathBuf::from(r"D:\senline");
            if fallback.is_dir() {
                Some(fallback)
            } else {
                None
            }
        });
    let Some(root) = senline else {
        eprintln!(
            "senline checkout absent; skipping live consumer path scan \
             (extension/marker rules still covered by synthetic fixture test)"
        );
        return;
    };

    let mut files = Vec::new();
    for rel in SENLINE_PRODUCT_ROOTS {
        walk_text_files(&root.join(rel), &mut files);
    }
    // Also scan top-level config-ish files if present.
    for rel in ["docs", "openspec"] {
        // docs/openspec may name the harness as forbidden — only product roots fail.
        let _ = rel;
    }

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let is_product_surface = SENLINE_PRODUCT_ROOTS
            .iter()
            .any(|prefix| rel.starts_with(&format!("{prefix}/")) || rel.starts_with(prefix));
        if !is_product_surface {
            continue;
        }
        if let Some(marker) = product_surface_hits_harness(&text) {
            panic!(
                "Senline product path {} must not target HTTP dogfood harness marker {marker}",
                path.display()
            );
        }
    }
}

#[test]
fn policy_scan_detects_android_and_windows_client_harness_targets() {
    // Synthetic fail-closed fixture: product-surface client files that would be
    // invisible under the previous .sg/.md/.rs-only walk must be rejected.
    let root = std::env::temp_dir().join(format!(
        "senline-policy-scan-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("apps/chat_client")).expect("create apps");
    fs::create_dir_all(root.join("android/app/src")).expect("create android");
    fs::create_dir_all(root.join("win/ChatClient")).expect("create win");

    fs::write(
        root.join("android/app/src/MainActivity.kt"),
        "val url = \"http://127.0.0.1:9/v1/submit-envelope\" // senline-http-dogfood\n",
    )
    .expect("write kt");
    fs::write(
        root.join("win/ChatClient/Client.cs"),
        "var path = \"/v1/submit-envelope\"; // uses senline_http_dogfood\n",
    )
    .expect("write cs");
    fs::write(
        root.join("apps/chat_client/build.gradle"),
        "applicationId 'com.example' // READY 127.0.0.1 banner for dogfood\n",
    )
    .expect("write gradle");
    fs::write(
        root.join("apps/chat_client/App.xaml"),
        "<!-- senline-http-dogfood harness must not be linked -->\n",
    )
    .expect("write xaml");
    fs::write(
        root.join("apps/chat_client/api.ts"),
        "export const endpoint = 'senline_http_dogfood';\n",
    )
    .expect("write ts");

    let mut files = Vec::new();
    for rel in SENLINE_PRODUCT_ROOTS {
        walk_text_files(&root.join(rel), &mut files);
    }
    assert!(
        files.len() >= 5,
        "expected client surfaces to be discovered, got {}: {:?}",
        files.len(),
        files
    );

    let mut hits = 0usize;
    for path in &files {
        let text = fs::read_to_string(path).expect("read fixture");
        if product_surface_hits_harness(&text).is_some() {
            hits += 1;
        }
    }
    assert!(
        hits >= 4,
        "synthetic Android/Windows/web client fixtures must fail the harness scan (hits={hits})"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn policy_scan_extension_allowlist_covers_client_languages() {
    for ext in ["kt", "kts", "gradle", "xml", "cs", "xaml", "ts", "tsx", "js"] {
        assert!(
            is_scanned_extension(ext),
            "client extension .{ext} must be scanned for task 6.6"
        );
    }
}
