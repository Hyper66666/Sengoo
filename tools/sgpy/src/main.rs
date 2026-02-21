//! sgpy - Sengoo package and project helper CLI.

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use walkdir::WalkDir;

const REGISTRY_URL: &str = "https://registry.sengoo.dev";

#[derive(Parser, Debug)]
#[command(name = "sgpy")]
#[command(about = "Sengoo package manager and project helper", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a Sengoo project.
    Init {
        #[arg(short, long)]
        name: Option<String>,

        #[arg(short, long, default_value = ".")]
        path: String,
    },

    /// Add dependencies.
    Add {
        /// e.g. serde or serde=1.0
        #[arg(required = true)]
        packages: Vec<String>,

        /// Add to dev-dependencies.
        #[arg(long)]
        dev: bool,
    },

    /// Remove dependency.
    Remove {
        #[arg(required = true)]
        package: String,
    },

    /// Update one dependency or all dependencies.
    Update { package: Option<String> },

    /// Build all .sg files in src/.
    Build {
        #[arg(long)]
        release: bool,
    },

    /// Publish package (placeholder).
    Publish {
        #[arg(long)]
        dry_run: bool,
    },

    /// Search package from registry.
    Search { query: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectConfig {
    name: String,
    version: String,
    sengoo_version: String,
    authors: Vec<String>,
    description: Option<String>,
    dependencies: Option<Value>,
    dev_dependencies: Option<Value>,
}

fn load_project_config() -> Result<(PathBuf, ProjectConfig)> {
    let path = PathBuf::from("Sengoo.toml");
    if !path.exists() {
        miette::bail!("Sengoo.toml not found, run `sgpy init` first");
    }
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let config: ProjectConfig = toml::from_str(&content).into_diagnostic()?;
    Ok((path, config))
}

fn save_project_config(path: &Path, config: &ProjectConfig) -> Result<()> {
    let content = toml::to_string_pretty(config).into_diagnostic()?;
    fs::write(path, content).into_diagnostic()?;
    Ok(())
}

fn ensure_object_table(slot: &mut Option<Value>) -> &mut Map<String, Value> {
    let value = slot.get_or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("value is object")
}

fn map_contains(slot: &Option<Value>, key: &str) -> bool {
    slot.as_ref()
        .and_then(Value::as_object)
        .map(|m| m.contains_key(key))
        .unwrap_or(false)
}

fn map_keys(slot: &Option<Value>) -> Vec<String> {
    slot.as_ref()
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

fn parse_package_spec(spec: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = spec.splitn(2, '=').collect();
    let name = parts[0].trim().to_string();
    let version = parts.get(1).map(|v| v.trim().to_string());
    (name, version)
}

fn parse_latest_version(payload: &Value) -> Option<String> {
    payload
        .get("latest_version")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            payload
                .pointer("/dist-tags/latest")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            payload
                .get("version")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(parse_latest_version)
                .or_else(|| payload.get("package").and_then(parse_latest_version))
        })
}

fn registry_get_json(path: &str, query: &[(&str, &str)]) -> Result<Option<Value>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .into_diagnostic()?;

    let url = format!("{}{}", REGISTRY_URL, path);
    let response = rt.block_on(async {
        reqwest::Client::new()
            .get(url)
            .query(query)
            .timeout(Duration::from_secs(4))
            .send()
            .await
    });

    let Ok(resp) = response else {
        return Ok(None);
    };

    if !resp.status().is_success() {
        return Ok(None);
    }

    let body = rt.block_on(async { resp.json::<Value>().await });
    match body {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

fn query_latest_version(name: &str) -> Result<String> {
    let path = format!("/api/v1/packages/{}", name);
    if let Some(payload) = registry_get_json(&path, &[])? {
        if let Some(version) = parse_latest_version(&payload) {
            return Ok(version);
        }
    }

    // Graceful fallback when registry is unavailable/shape unknown.
    Ok("1.0.0".to_string())
}

fn cmd_init(name: Option<String>, path: &str) -> Result<()> {
    let path = PathBuf::from(path);
    let project_name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my_project")
            .to_string()
    });

    fs::create_dir_all(path.join("src")).into_diagnostic()?;
    fs::create_dir_all(path.join("tests")).into_diagnostic()?;
    fs::create_dir_all(path.join("examples")).into_diagnostic()?;

    let config = ProjectConfig {
        name: project_name.clone(),
        version: "0.1.0".to_string(),
        sengoo_version: "^0.1.0".to_string(),
        authors: vec!["Your Name <you@example.com>".to_string()],
        description: Some("A Sengoo project".to_string()),
        dependencies: None,
        dev_dependencies: None,
    };

    save_project_config(&path.join("Sengoo.toml"), &config)?;

    let main_content = r#"def main() -> i64 {
    print(\"Hello, Sengoo!\")
    0
}
"#;
    fs::write(path.join("src/main.sg"), main_content).into_diagnostic()?;

    let gitignore = r#"# Sengoo build output
target/
*.sgc
*.ll

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
"#;
    fs::write(path.join(".gitignore"), gitignore).into_diagnostic()?;

    println!("Initialized project: {}", project_name);
    let display_path = path.canonicalize().unwrap_or_else(|_| path.clone());
    println!("Path: {}", display_path.display());
    println!("Next:\n  cd {}\n  sgpy build", path.to_string_lossy());
    Ok(())
}

fn cmd_add(packages: Vec<String>, dev: bool) -> Result<()> {
    let (toml_path, mut config) = load_project_config()?;

    for pkg in packages {
        let (name, requested_version) = parse_package_spec(&pkg);
        if name.is_empty() {
            continue;
        }

        let version = if let Some(v) = requested_version {
            v
        } else {
            query_latest_version(&name)?
        };

        let target = if dev {
            ensure_object_table(&mut config.dev_dependencies)
        } else {
            ensure_object_table(&mut config.dependencies)
        };
        target.insert(name.clone(), Value::String(version.clone()));
        println!(
            "Added {} = \"{}\"{}",
            name,
            version,
            if dev { " (dev)" } else { "" }
        );
    }

    save_project_config(&toml_path, &config)?;
    Ok(())
}

fn cmd_remove(package: String) -> Result<()> {
    let (toml_path, mut config) = load_project_config()?;
    let mut removed = false;

    if let Some(map) = config.dependencies.as_mut().and_then(Value::as_object_mut) {
        removed |= map.remove(&package).is_some();
    }
    if let Some(map) = config
        .dev_dependencies
        .as_mut()
        .and_then(Value::as_object_mut)
    {
        removed |= map.remove(&package).is_some();
    }

    if removed {
        save_project_config(&toml_path, &config)?;
        println!("Removed dependency: {}", package);
    } else {
        println!("Dependency not found: {}", package);
    }
    Ok(())
}

fn cmd_update(package: Option<String>) -> Result<()> {
    let (toml_path, mut config) = load_project_config()?;

    let mut targets = BTreeSet::new();
    if let Some(pkg) = package {
        targets.insert(pkg);
    } else {
        for k in map_keys(&config.dependencies) {
            targets.insert(k);
        }
        for k in map_keys(&config.dev_dependencies) {
            targets.insert(k);
        }
    }

    if targets.is_empty() {
        println!("No dependencies to update");
        return Ok(());
    }

    let mut updated = 0usize;
    for dep in targets {
        let exists = map_contains(&config.dependencies, &dep)
            || map_contains(&config.dev_dependencies, &dep);
        if !exists {
            println!("Skip {} (not found)", dep);
            continue;
        }

        let latest = query_latest_version(&dep)?;
        if let Some(map) = config.dependencies.as_mut().and_then(Value::as_object_mut) {
            if map.contains_key(&dep) {
                map.insert(dep.clone(), Value::String(latest.clone()));
                updated += 1;
            }
        }
        if let Some(map) = config
            .dev_dependencies
            .as_mut()
            .and_then(Value::as_object_mut)
        {
            if map.contains_key(&dep) {
                map.insert(dep.clone(), Value::String(latest.clone()));
                updated += 1;
            }
        }

        println!("Updated {} -> {}", dep, latest);
    }

    save_project_config(&toml_path, &config)?;
    println!("Updated {} entries", updated);
    Ok(())
}

fn cmd_build(release: bool) -> Result<()> {
    let (_toml_path, config) = load_project_config()?;
    let sgc_path = which::which("sgc").map_err(|_| miette::miette!("sgc not found in PATH"))?;

    let src_dir = PathBuf::from("src");
    if !src_dir.exists() {
        miette::bail!("src directory not found");
    }

    let target_dir = if release {
        PathBuf::from("target/release")
    } else {
        PathBuf::from("target/debug")
    };
    fs::create_dir_all(&target_dir).into_diagnostic()?;

    let mut built = 0usize;
    for entry in WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
        let file = entry.path();
        if file.extension().and_then(|s| s.to_str()) != Some("sg") {
            continue;
        }

        let rel = file.strip_prefix(&src_dir).unwrap_or(file);
        let mut output = target_dir.join(rel);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }
        output.set_extension("sgc");

        let mut cmd = Command::new(&sgc_path);
        cmd.arg("build")
            .arg(file)
            .arg("--output")
            .arg(&output)
            .arg("-O")
            .arg(if release { "2" } else { "0" });

        let status = cmd.status().into_diagnostic()?;
        if !status.success() {
            miette::bail!("build failed for {}", file.display());
        }

        built += 1;
        println!("Built {} -> {}", file.display(), output.display());
    }

    println!(
        "Build done: {} v{}, files={}",
        config.name, config.version, built
    );
    Ok(())
}

fn cmd_search(query: String) -> Result<()> {
    let payload = registry_get_json("/api/v1/search", &[("q", &query)])?;
    let Some(payload) = payload else {
        println!("Registry unavailable, cannot search now");
        return Ok(());
    };

    let list = payload
        .get("results")
        .or_else(|| payload.get("packages"))
        .or_else(|| payload.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if list.is_empty() {
        println!("No packages found for query: {}", query);
        return Ok(());
    }

    for item in list {
        if let Some(obj) = item.as_object() {
            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            let version = obj
                .get("version")
                .and_then(Value::as_str)
                .or_else(|| obj.get("latest_version").and_then(Value::as_str))
                .unwrap_or("?");
            let description = obj.get("description").and_then(Value::as_str).unwrap_or("");
            println!("{} {} - {}", name, version, description);
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, path } => cmd_init(name, &path),
        Commands::Add { packages, dev } => cmd_add(packages, dev),
        Commands::Remove { package } => cmd_remove(package),
        Commands::Update { package } => cmd_update(package),
        Commands::Build { release } => cmd_build(release),
        Commands::Publish { dry_run } => {
            println!("Publish is not implemented yet (dry-run={})", dry_run);
            Ok(())
        }
        Commands::Search { query } => cmd_search(query),
    }
}
