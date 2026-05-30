//! sgpm - offline Sengoo package/project manager MVP.

mod cache;
mod lockfile;
mod manifest;
mod package;
mod resolver;
mod runner;
mod scaffold;
mod workspace;

use clap::{Parser, Subcommand, ValueEnum};
use miette::{Context, Result};
use resolver::{Graph, ResolveOptions};
use runner::{BuildProfile, Toolchain};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "sgpm")]
#[command(version)]
#[command(about = "Sengoo package manager MVP", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new Sengoo package.
    New {
        /// Package name.
        name: String,

        /// Destination directory. Defaults to ./<name>.
        #[arg(long)]
        path: Option<PathBuf>,

        /// Create a library package with src/lib.sg.
        #[arg(long)]
        lib: bool,
    },

    /// Initialize a Sengoo package in an existing directory.
    Init {
        /// Package name. Defaults to the destination directory name.
        name: Option<String>,

        /// Destination directory. Defaults to the current directory.
        #[arg(long)]
        path: Option<PathBuf>,

        /// Create a library package with src/lib.sg.
        #[arg(long)]
        lib: bool,
    },

    /// Build the package and path dependencies.
    Build(PackageArgs),

    /// Type-check the package and path dependencies.
    Check(PackageArgs),

    /// Run the root package entry point.
    Run(RunArgs),

    /// Run .sg files under tests/ for the package graph.
    Test(PackageArgs),

    /// Format package graph sources with sgfmt.
    Fmt(FmtArgs),

    /// Print the resolved package dependency tree.
    Tree(TreeArgs),

    /// Print machine-readable package graph metadata.
    Metadata(MetadataArgs),

    /// Remove root package build artifacts.
    Clean(CleanArgs),

    /// Validate and package the root package for publishing.
    Publish(PublishArgs),

    /// Resolve the package graph and write or verify Sengoo.lock.
    Update(UpdateArgs),

    /// Inspect or remove local sgpm caches.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum CacheCommand {
    /// Print local sgpm cache entries.
    List(CacheListArgs),

    /// Remove selected local sgpm caches.
    Clean(CacheCleanArgs),
}

#[derive(Parser, Debug, Clone)]
struct CacheListArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,
}

#[derive(Parser, Debug, Clone)]
struct CacheCleanArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Remove cached git dependency checkouts.
    #[arg(long)]
    git: bool,

    /// Remove cached remote registry packages.
    #[arg(long)]
    registry: bool,
}

#[derive(Parser, Debug, Clone)]
struct TreeArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Require Sengoo.lock to be current before running the command.
    #[arg(long)]
    locked: bool,
}

#[derive(Parser, Debug, Clone)]
struct MetadataArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = MetadataFormat::Json)]
    format: MetadataFormat,

    /// Require Sengoo.lock to be current before printing metadata.
    #[arg(long)]
    locked: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MetadataFormat {
    Json,
}

#[derive(Parser, Debug, Clone)]
struct CleanArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,
}

#[derive(Parser, Debug, Clone)]
struct PackageArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Use target/release and -O2 instead of target/debug and -O0.
    #[arg(long)]
    release: bool,

    /// Print delegated sgc/sgfmt commands.
    #[arg(short, long)]
    verbose: bool,

    /// Require Sengoo.lock to be current before running the command.
    #[arg(long)]
    locked: bool,
}

#[derive(Parser, Debug, Clone)]
struct RunArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Use -O2 instead of the debug -O0 profile.
    #[arg(long)]
    release: bool,

    /// Print delegated sgc command.
    #[arg(short, long)]
    verbose: bool,

    /// Require Sengoo.lock to be current before running the command.
    #[arg(long)]
    locked: bool,

    /// Arguments forwarded to the Sengoo program.
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
struct FmtArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Check formatting without writing files.
    #[arg(long)]
    check: bool,

    /// Print delegated sgfmt commands.
    #[arg(short, long)]
    verbose: bool,

    /// Require Sengoo.lock to be current before running the command.
    #[arg(long)]
    locked: bool,
}

#[derive(Parser, Debug, Clone)]
struct PublishArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Create a local package artifact without uploading to a registry.
    #[arg(long)]
    dry_run: bool,

    /// Publish to a configured registry.
    #[arg(long)]
    registry: Option<String>,

    /// Output directory for the generated package archive.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Require Sengoo.lock to be current before packaging.
    #[arg(long)]
    locked: bool,
}

#[derive(Parser, Debug, Clone)]
struct UpdateArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Verify Sengoo.lock is current without rewriting it.
    #[arg(long)]
    check: bool,

    /// Reclone git dependency caches while resolving the graph.
    #[arg(long)]
    refresh: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, path, lib } => {
            let root = scaffold::new_project_with_kind(&name, path.as_deref(), scaffold_kind(lib))?;
            println!("created package '{}' at {}", name, root.display());
            println!("next: cd {} && sgpm check", root.display());
            Ok(())
        }
        Commands::Init { name, path, lib } => {
            let (name, root) = scaffold::init_project_with_kind(
                name.as_deref(),
                path.as_deref(),
                scaffold_kind(lib),
            )?;
            println!("initialized package '{}' at {}", name, root.display());
            println!("next: sgpm check --manifest-path {}", root.display());
            Ok(())
        }
        Commands::Build(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            let toolchain = Toolchain::discover()?;
            for graph in &graphs {
                toolchain.build(graph, profile(args.release), args.verbose)?;
            }
            Ok(())
        }
        Commands::Check(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            let toolchain = Toolchain::discover()?;
            for graph in &graphs {
                toolchain.check(graph, args.verbose)?;
            }
            Ok(())
        }
        Commands::Run(args) => {
            let graph = load_graph(&args.manifest_path, args.locked, args.package.as_deref())?;
            let toolchain = Toolchain::discover()?;
            toolchain.run(&graph, profile(args.release), &args.args, args.verbose)
        }
        Commands::Test(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            let toolchain = Toolchain::discover()?;
            for graph in &graphs {
                toolchain.test(graph, profile(args.release), args.verbose)?;
            }
            Ok(())
        }
        Commands::Fmt(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            let toolchain = Toolchain::discover()?;
            for graph in &graphs {
                toolchain.fmt(graph, args.check, args.verbose)?;
            }
            Ok(())
        }
        Commands::Tree(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            print_graphs(&graphs);
            Ok(())
        }
        Commands::Metadata(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            match args.format {
                MetadataFormat::Json => {
                    println!("{}", render_metadata_json(&graphs)?);
                }
            }
            Ok(())
        }
        Commands::Clean(args) => {
            let graphs = load_graphs(
                &args.manifest_path,
                false,
                args.package.as_deref(),
                args.workspace,
            )?;
            for graph in &graphs {
                runner::clean(graph)?;
            }
            Ok(())
        }
        Commands::Publish(args) => {
            let graph = load_graph(&args.manifest_path, args.locked, args.package.as_deref())?;
            if args.dry_run {
                if args.registry.is_some() {
                    miette::bail!("--dry-run cannot be combined with --registry");
                }
                let artifact = package::publish_dry_run(&graph, args.output.as_deref())?;
                println!("packaged {}", artifact.archive_path.display());
                println!(
                    "checksum {} {}",
                    artifact.checksum,
                    artifact.checksum_path.display()
                );
                return Ok(());
            }

            if args.output.is_some() {
                miette::bail!("--output requires --dry-run");
            }

            let registry = args.registry.as_deref().unwrap_or("default");
            match package::publish_to_registry(&graph, registry)? {
                package::RegistryPublishResult::Local(publish) => {
                    println!(
                        "published {} v{} to {} registry at {}",
                        publish.name,
                        publish.version,
                        registry,
                        publish.target_dir.display()
                    );
                }
                package::RegistryPublishResult::Remote(publish) => {
                    println!(
                        "published {} v{} to remote registry {}",
                        publish.name, publish.version, publish.endpoint
                    );
                    println!("checksum {}", publish.checksum);
                }
            }
            Ok(())
        }
        Commands::Update(args) => {
            let graphs = load_graphs_with_options(
                &args.manifest_path,
                false,
                ResolveOptions {
                    refresh_git: args.refresh,
                },
                args.package.as_deref(),
                args.workspace,
            )?;
            if args.workspace {
                let workspace_manifest = resolver::resolve_manifest_path(&args.manifest_path)?;
                if args.check {
                    let path = lockfile::check_workspace_lockfile(&workspace_manifest, &graphs)?;
                    println!("Sengoo.lock is up to date: {}", path.display());
                } else {
                    let path = lockfile::write_workspace_lockfile(&workspace_manifest, &graphs)?;
                    println!("updated {}", path.display());
                }
                return Ok(());
            }

            for graph in &graphs {
                if args.check {
                    let path = lockfile::check_lockfile(graph)?;
                    println!("Sengoo.lock is up to date: {}", path.display());
                } else {
                    let path = lockfile::write_lockfile(graph)?;
                    println!("updated {}", path.display());
                }
            }
            Ok(())
        }
        Commands::Cache { command } => match command {
            CacheCommand::List(args) => {
                let selection =
                    workspace::select_manifest(&args.manifest_path, args.package.as_deref())?;
                let entries = cache::list(&selection.manifest_path)?;
                if entries.is_empty() {
                    println!("no sgpm caches found");
                } else {
                    for entry in entries {
                        println!("{} {} {}", entry.kind, entry.name, entry.path.display());
                    }
                }
                Ok(())
            }
            CacheCommand::Clean(args) => {
                if !args.git && !args.registry {
                    miette::bail!("select a cache kind to clean, for example --git or --registry");
                }
                let selection =
                    workspace::select_manifest(&args.manifest_path, args.package.as_deref())?;
                if args.git {
                    match cache::clean_git(&selection.manifest_path)? {
                        Some(path) => println!("removed git cache {}", path.display()),
                        None => println!("git cache already empty"),
                    }
                }
                if args.registry {
                    match cache::clean_registry(&selection.manifest_path)? {
                        Some(path) => println!("removed registry cache {}", path.display()),
                        None => println!("registry cache already empty"),
                    }
                }
                Ok(())
            }
        },
    }
}

fn load_graph(manifest_path: &Path, locked: bool, package: Option<&str>) -> Result<Graph> {
    load_graph_with_options(manifest_path, locked, ResolveOptions::default(), package)
}

fn load_graph_with_options(
    manifest_path: &Path,
    locked: bool,
    options: ResolveOptions,
    package: Option<&str>,
) -> Result<Graph> {
    let mut graphs = load_graphs_with_options(manifest_path, locked, options, package, false)?;
    graphs
        .pop()
        .ok_or_else(|| miette::miette!("no package graph selected"))
}

fn load_graphs(
    manifest_path: &Path,
    locked: bool,
    package: Option<&str>,
    workspace_all: bool,
) -> Result<Vec<Graph>> {
    load_graphs_with_options(
        manifest_path,
        locked,
        ResolveOptions::default(),
        package,
        workspace_all,
    )
}

fn load_graphs_with_options(
    manifest_path: &Path,
    locked: bool,
    options: ResolveOptions,
    package: Option<&str>,
    workspace_all: bool,
) -> Result<Vec<Graph>> {
    let selections = workspace::select_manifests(manifest_path, package, workspace_all)
        .with_context(|| format!("failed to select package from {}", manifest_path.display()))?;
    let mut graphs = Vec::new();
    for selection in selections {
        graphs.push(resolve_graph(selection, locked && !workspace_all, options)?);
    }
    if locked && workspace_all {
        let workspace_manifest = resolver::resolve_manifest_path(manifest_path)?;
        lockfile::check_workspace_lockfile(&workspace_manifest, &graphs)?;
    }
    Ok(graphs)
}

fn resolve_graph(
    selection: workspace::SelectedManifest,
    locked: bool,
    options: ResolveOptions,
) -> Result<Graph> {
    let selected_manifest_path = selection.manifest_path;
    let inherited_registries = selection.inherited_registries;
    let graph = if inherited_registries.is_empty() {
        Graph::from_root_with_options(&selected_manifest_path, options)
    } else {
        Graph::from_root_with_registries(&selected_manifest_path, options, inherited_registries)
    }
    .with_context(|| {
        format!(
            "failed to resolve package graph from {}",
            selected_manifest_path.display()
        )
    })?;
    if locked {
        lockfile::check_lockfile(&graph)?;
    }
    Ok(graph)
}

fn print_graphs(graphs: &[Graph]) {
    let multiple = graphs.len() > 1;
    for (index, graph) in graphs.iter().enumerate() {
        if multiple {
            if index > 0 {
                println!();
            }
            let name = graph
                .root_package()
                .map(|package| package.name.as_str())
                .unwrap_or("<unknown>");
            println!("# {}", name);
        }
        runner::print_tree(graph);
    }
}

#[derive(Serialize)]
struct MetadataPayload {
    schema_version: u32,
    workspace: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<String>,
    roots: Vec<String>,
    packages: Vec<MetadataPackage>,
}

#[derive(Serialize)]
struct MetadataPackage {
    name: String,
    version: String,
    source: String,
    manifest: String,
    root: String,
    entry: String,
    dependencies: Vec<String>,
}

fn render_metadata_json(graphs: &[Graph]) -> Result<String> {
    if graphs.is_empty() {
        miette::bail!("no package graphs selected");
    }
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    let mut packages = Vec::new();
    for graph in graphs {
        let root = graph
            .root_package()
            .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
        roots.push(root.name.clone());
        for node in &graph.nodes {
            if !seen.insert(node.manifest_path.clone()) {
                continue;
            }
            packages.push(MetadataPackage {
                name: node.name.clone(),
                version: node.manifest.package.version.clone(),
                source: metadata_source(node),
                manifest: slash_path(&node.manifest_path),
                root: slash_path(&node.root_dir),
                entry: slash_path(&node.entry_path),
                dependencies: node.manifest.dependencies.keys().cloned().collect(),
            });
        }
    }
    let payload = MetadataPayload {
        schema_version: 1,
        workspace: graphs.len() > 1,
        root: if graphs.len() == 1 {
            roots.first().cloned()
        } else {
            None
        },
        roots,
        packages,
    };
    serde_json::to_string_pretty(&payload)
        .map_err(|err| miette::miette!("failed to encode metadata json: {}", err))
}

fn metadata_source(node: &resolver::PackageNode) -> String {
    match &node.source {
        resolver::PackageSource::Path => "path".to_string(),
        resolver::PackageSource::Git { url, rev } => format!("git+{}#{}", url, rev),
        resolver::PackageSource::Registry { registry, version } => {
            format!("registry+{}/{}@{}", registry, node.name, version)
        }
    }
}

fn slash_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn profile(release: bool) -> BuildProfile {
    if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    }
}

fn scaffold_kind(lib: bool) -> scaffold::ProjectKind {
    if lib {
        scaffold::ProjectKind::Library
    } else {
        scaffold::ProjectKind::Binary
    }
}
