//! sgpy - Sengoo 包管理器
//!
//! 用法:
//!   sgpy init              # 初始化新项目
//!   sgpy add <package>     # 添加依赖
//!   sgpy remove <package>  # 移除依赖
//!   sgpy update            # 更新依赖
//!   sgpy build             # 构建项目
//!   sgpy publish           # 发布包

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;
use which;

/// Sengoo 包管理器
#[derive(Parser, Debug)]
#[command(name = "sgpy")]
#[command(about = "Sengoo 包管理器", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 初始化新项目
    Init {
        /// 项目名称
        #[arg(short, long)]
        name: Option<String>,

        /// 项目路径
        #[arg(short, long, default_value = ".")]
        path: String,
    },

    /// 添加依赖
    Add {
        /// 包名称 (如: serde = "1.0")
        #[arg(required = true)]
        packages: Vec<String>,

        /// 开发依赖
        #[arg(long)]
        dev: bool,
    },

    /// 移除依赖
    Remove {
        /// 包名称
        #[arg(required = true)]
        package: String,
    },

    /// 更新依赖
    Update {
        /// 包名称 (不指定则更新全部)
        package: Option<String>,
    },

    /// 构建项目
    Build {
        /// 发布模式
        #[arg(long)]
        release: bool,
    },

    /// 发布包
    Publish {
        /// 发布到测试注册表
        #[arg(long)]
        dry_run: bool,
    },

    /// 搜索包
    Search {
        /// 搜索关键词
        query: String,
    },
}

/// 项目配置
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ProjectConfig {
    name: String,
    version: String,
    sengoo_version: String,
    authors: Vec<String>,
    description: Option<String>,
    dependencies: Option<serde_json::Value>,
    dev_dependencies: Option<serde_json::Value>,
}

/// 包配置
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PackageConfig {
    name: String,
    version: String,
    description: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    sengoo_version: String,
    dependencies: Option<serde_json::Value>,
}

/// 注册表配置
const REGISTRY_URL: &str = "https://registry.sengoo.dev";

/// 初始化项目
fn cmd_init(name: Option<String>, path: &str) -> Result<()> {
    let path = PathBuf::from(path);
    let project_name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my_project")
            .to_string()
    });

    // 创建项目目录结构
    fs::create_dir_all(path.join("src")).into_diagnostic()?;
    fs::create_dir_all(path.join("tests")).into_diagnostic()?;
    fs::create_dir_all(path.join("examples")).into_diagnostic()?;

    // 创建 Sengoo.toml
    let config = ProjectConfig {
        name: project_name.clone(),
        version: "0.1.0".to_string(),
        sengoo_version: "^0.1.0".to_string(),
        authors: vec!["Your Name <you@example.com>".to_string()],
        description: Some("A Sengoo project".to_string()),
        dependencies: None,
        dev_dependencies: None,
    };

    let toml_content = toml::to_string_pretty(&config).into_diagnostic()?;
    fs::write(path.join("Sengoo.toml"), toml_content).into_diagnostic()?;

    // 创建主文件
    let main_content = r#"fn main() -> i64 {
    println!("Hello, Sengoo!");
    0
}
"#;
    fs::write(path.join("src/main.sg"), main_content).into_diagnostic()?;

    // 创建 .gitignore
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

    println!("✅ 项目初始化完成: {}", project_name);
    println!(
        "📁 项目位置: {}",
        path.canonicalize()
            .unwrap_or_else(|_| path.clone())
            .display()
    );
    println!("\n下一步:");
    let path_str = path.to_str().unwrap_or(".");
    println!("  cd {}", if path_str == "." { "." } else { path_str });
    println!("  sgpy build");

    Ok(())
}

/// 添加依赖
fn cmd_add(packages: Vec<String>, dev: bool) -> Result<()> {
    let toml_path = PathBuf::from("Sengoo.toml");
    if !toml_path.exists() {
        miette::bail!("未找到 Sengoo.toml，请先在项目根目录运行 sgpy init");
    }

    let toml_content = fs::read_to_string(&toml_path).into_diagnostic()?;
    let mut config: ProjectConfig = toml::from_str(&toml_content).into_diagnostic()?;

    for pkg in packages {
        // 解析包版本 (如: serde = "1.0")
        let parts: Vec<&str> = pkg.split('=').collect();
        let name = parts[0].trim().to_string();
        let version = parts.get(1).map(|s| s.trim().to_string());

        // 搜索并解析依赖
        let dep_spec = if let Some(ref v) = version {
            format!(r#"{} = "{}""#, name, v)
        } else {
            // 查询注册表获取最新版本
            let latest_version = query_latest_version(&name)?;
            format!(r#"{} = "{}""#, name, latest_version)
        };

        println!("📦 添加依赖: {}", dep_spec);

        // 添加到配置
        let version_str = version.unwrap_or_else(|| "*".to_string());
        if dev {
            let deps = config.dev_dependencies.get_or_insert(serde_json::json!({}));
            if let serde_json::Value::Object(ref mut map) = deps {
                map.insert(name.clone(), serde_json::Value::String(version_str.clone()));
                config.dev_dependencies = Some(serde_json::Value::Object(map.clone()));
            }
        } else {
            let deps = config.dependencies.get_or_insert(serde_json::json!({}));
            if let serde_json::Value::Object(ref mut map) = deps {
                map.insert(name.clone(), serde_json::Value::String(version_str));
                config.dependencies = Some(serde_json::Value::Object(map.clone()));
            }
        }
    }

    // 写回配置
    let toml_content = toml::to_string_pretty(&config).into_diagnostic()?;
    fs::write(&toml_path, toml_content).into_diagnostic()?;

    println!("✅ 依赖添加完成");
    Ok(())
}

/// 查询最新版本
fn query_latest_version(name: &str) -> Result<String> {
    // TODO: 实际调用注册表 API
    println!("🔍 查询注册表: {}", name);
    Ok("1.0.0".to_string()) // 临时返回
}

/// 移除依赖
fn cmd_remove(package: String) -> Result<()> {
    let toml_path = PathBuf::from("Sengoo.toml");
    let toml_content = fs::read_to_string(&toml_path).into_diagnostic()?;
    let mut config: ProjectConfig = toml::from_str(&toml_content).into_diagnostic()?;

    if let Some(deps) = &mut config.dependencies {
        if let serde_json::Value::Object(mut map) = deps.take() {
            if map.remove(&package).is_some() {
                config.dependencies = Some(serde_json::Value::Object(map));
                println!("✅ 移除依赖: {}", package);
            } else {
                println!("⚠️  依赖 {} 不存在", package);
            }
        }
    }

    let toml_content = toml::to_string_pretty(&config).into_diagnostic()?;
    fs::write(&toml_path, toml_content).into_diagnostic()?;

    Ok(())
}

/// 更新依赖
fn cmd_update(package: Option<String>) -> Result<()> {
    println!("🔄 更新依赖...");

    if let Some(pkg) = package {
        println!("更新: {}", pkg);
        // TODO: 实现单个包更新
    } else {
        println!("更新所有依赖...");
        // TODO: 实现全部更新
    }

    Ok(())
}

/// 构建项目
fn cmd_build(release: bool) -> Result<()> {
    println!("🔨 构建项目...");

    // 读取配置
    let toml_path = PathBuf::from("Sengoo.toml");
    if !toml_path.exists() {
        miette::bail!("未找到 Sengoo.toml");
    }

    let toml_content = fs::read_to_string(&toml_path).into_diagnostic()?;
    let config: ProjectConfig = toml::from_str(&toml_content).into_diagnostic()?;

    // 检查编译器
    let sgc_path = which::which("sgc")
        .map_err(|_| miette::miette!("未找到 sgc 编译器，请先安装 Sengoo 工具链"))?;

    // 编译源文件
    let src_dir = PathBuf::from("src");
    if !src_dir.exists() {
        miette::bail!("未找到 src 目录");
    }

    for entry in walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("sg") {
            println!("编译: {}", entry.path().display());
            // TODO: 调用 sgc 编译
        }
    }

    println!("✅ 构建完成: {} v{}", config.name, config.version);
    Ok(())
}

/// 搜索包
fn cmd_search(query: String) -> Result<()> {
    println!("🔍 搜索: {}", query);

    // TODO: 调用注册表 API
    println!("找到 0 个结果");

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
        Commands::Publish { dry_run: _ } => {
            println!("📤 发布功能即将推出");
            Ok(())
        }
        Commands::Search { query } => cmd_search(query),
    }
}
