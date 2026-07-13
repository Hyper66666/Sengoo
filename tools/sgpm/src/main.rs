//! sgpm - offline Sengoo package/project manager MVP.

mod cache;
mod lockfile;
mod manifest;
mod package;
mod registry_server;
mod resolver;
mod runner;
mod scaffold;
mod workspace;

use clap::{Parser, Subcommand, ValueEnum};
use miette::{Context, Result};
use resolver::{Graph, PackageSource, RegistryPackageMetadata, ResolveOptions};
use runner::{BuildProfile, Toolchain};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SGPM_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("SENGOO_BUILD_HASH"),
    ")"
);

#[derive(Parser, Debug)]
#[command(name = "sgpm")]
#[command(version = SGPM_VERSION)]
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

    /// Generate API documentation for the package graph.
    Doc(DocArgs),

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

    /// Run or administer the reference package registry.
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum RegistryCommand {
    /// Run the filesystem-backed reference registry server.
    Serve {
        /// Directory used for package archives, metadata, and owner reservations.
        #[arg(long, default_value = "target/sgpm-registry")]
        root: PathBuf,

        /// TCP address for the HTTP listener.
        #[arg(long, default_value = "127.0.0.1:7878")]
        listen: String,

        /// Exit after serving this many requests (intended for deterministic smoke tests).
        #[arg(long, hide = true)]
        max_requests: Option<usize>,
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
struct DocArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Workspace member package to operate on when manifest-path points at a workspace root.
    #[arg(long)]
    package: Option<String>,

    /// Operate on every member when manifest-path points at a workspace root.
    #[arg(long)]
    workspace: bool,

    /// Documentation output directory. Defaults to target/doc for the selected package.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print delegated sgc commands.
    #[arg(short, long)]
    verbose: bool,

    /// Require Sengoo.lock to be current before generating docs.
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

    /// Output format.
    #[arg(long, value_enum, default_value_t = PublishFormat::Human)]
    format: PublishFormat,

    /// Require Sengoo.lock to be current before packaging.
    #[arg(long)]
    locked: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PublishFormat {
    Human,
    Json,
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
        Commands::Doc(args) => {
            if args.workspace && args.output.is_some() {
                miette::bail!("--output cannot be combined with --workspace");
            }
            let graphs = load_graphs(
                &args.manifest_path,
                args.locked,
                args.package.as_deref(),
                args.workspace,
            )?;
            let toolchain = Toolchain::discover()?;
            for graph in &graphs {
                toolchain.doc(graph, args.output.as_deref(), args.verbose)?;
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
            let graphs = load_graphs_with_options(
                &args.manifest_path,
                args.locked,
                ResolveOptions {
                    allow_yanked: true,
                    ..ResolveOptions::default()
                },
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
                match args.format {
                    PublishFormat::Human => {
                        println!("packaged {}", artifact.archive_path.display());
                        println!(
                            "checksum {} {}",
                            artifact.checksum,
                            artifact.checksum_path.display()
                        );
                    }
                    PublishFormat::Json => {
                        println!(
                            "{}",
                            render_publish_metadata_json(
                                &graph,
                                &artifact,
                                None,
                                args.package.as_deref(),
                                args.locked
                            )?
                        );
                    }
                }
                return Ok(());
            }

            if args.output.is_some() {
                miette::bail!("--output requires --dry-run");
            }

            let registry = args.registry.as_deref().unwrap_or("default");
            let artifact = if matches!(args.format, PublishFormat::Json) {
                Some(package::publish_dry_run(&graph, None)?)
            } else {
                None
            };
            match package::publish_to_registry(&graph, registry)? {
                package::RegistryPublishResult::Local(publish) => match args.format {
                    PublishFormat::Human => {
                        println!(
                            "published {} v{} to {} registry at {}",
                            publish.name,
                            publish.version,
                            registry,
                            publish.target_dir.display()
                        );
                    }
                    PublishFormat::Json => {
                        println!(
                            "{}",
                            render_publish_metadata_json(
                                &graph,
                                artifact
                                    .as_ref()
                                    .expect("json publish should have artifact"),
                                Some(registry),
                                args.package.as_deref(),
                                args.locked
                            )?
                        );
                    }
                },
                package::RegistryPublishResult::Remote(publish) => match args.format {
                    PublishFormat::Human => {
                        println!(
                            "published {} v{} to remote registry {}",
                            publish.name, publish.version, publish.endpoint
                        );
                        println!("checksum {}", publish.checksum);
                    }
                    PublishFormat::Json => {
                        println!(
                            "{}",
                            render_publish_metadata_json(
                                &graph,
                                artifact
                                    .as_ref()
                                    .expect("json publish should have artifact"),
                                Some(registry),
                                args.package.as_deref(),
                                args.locked
                            )?
                        );
                    }
                },
            }
            Ok(())
        }
        Commands::Update(args) => {
            let graphs = load_graphs_with_options(
                &args.manifest_path,
                false,
                ResolveOptions {
                    refresh_git: args.refresh,
                    ..ResolveOptions::default()
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
        Commands::Registry { command } => match command {
            RegistryCommand::Serve {
                root,
                listen,
                max_requests,
            } => registry_server::serve(&root, &listen, max_requests),
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
    mut options: ResolveOptions,
    package: Option<&str>,
    workspace_all: bool,
) -> Result<Vec<Graph>> {
    if locked {
        options.allow_yanked = true;
        let root_manifest = resolver::resolve_manifest_path(manifest_path)?;
        let lockfile_path = root_manifest
            .parent()
            .ok_or_else(|| miette::miette!("manifest has no parent directory"))?
            .join("Sengoo.lock");
        options.locked_registry = Some(Arc::new(lockfile::read_locked_registry_graph(
            &lockfile_path,
        )?));
    }
    let selections = workspace::select_manifests(manifest_path, package, workspace_all)
        .with_context(|| format!("failed to select package from {}", manifest_path.display()))?;
    let mut graphs = Vec::new();
    for selection in selections {
        graphs.push(resolve_graph(
            selection,
            locked && !workspace_all,
            options.clone(),
        )?);
    }
    if locked && workspace_all {
        let workspace_manifest = resolver::resolve_manifest_path(manifest_path)?;
        lockfile::check_workspace_lockfile(&workspace_manifest, &graphs)?;
        for graph in &graphs {
            warn_locked_yanked_packages(graph);
        }
    }
    Ok(graphs)
}

fn resolve_graph(
    selection: workspace::SelectedManifest,
    locked: bool,
    mut options: ResolveOptions,
) -> Result<Graph> {
    let selected_manifest_path = selection.manifest_path;
    let inherited_registries = selection.inherited_registries;
    if locked {
        options.allow_yanked = true;
    }
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
        warn_locked_yanked_packages(&graph);
    }
    Ok(graph)
}

fn warn_locked_yanked_packages(graph: &Graph) {
    for node in &graph.nodes {
        let PackageSource::Registry {
            registry,
            version,
            metadata,
        } = &node.source
        else {
            continue;
        };
        if !metadata.yanked {
            continue;
        }
        let reason = metadata
            .yank_reason
            .as_deref()
            .filter(|reason| !reason.trim().is_empty())
            .map(|reason| format!(": {}", reason))
            .unwrap_or_default();
        eprintln!(
            "warning: locked registry package '{}' version {} from registry '{}' is yanked{}; run sgpm update after adjusting the dependency constraint",
            node.name, version, registry, reason
        );
    }
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<MetadataDependency>,
}

#[derive(Serialize)]
struct MetadataPackage {
    id: String,
    name: String,
    version: String,
    source: MetadataSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    yanked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    yank_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    features: Option<Vec<String>>,
    manifest: String,
    root: String,
    entry: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tests: Vec<String>,
}

#[derive(Serialize)]
struct MetadataSource {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Serialize)]
struct MetadataDependency {
    from: String,
    alias: String,
    to: String,
}

#[derive(Serialize)]
struct PublishMetadataPayload {
    schema_version: u32,
    package: PublishPackageIdentity,
    manifest: String,
    archive_path: String,
    checksum_path: String,
    sha256: String,
    included_file_count: usize,
    excluded_file_count: usize,
    lockfile: PublishLockfileStatus,
    registry: Option<String>,
    workspace_package: Option<String>,
}

#[derive(Serialize)]
struct PublishPackageIdentity {
    name: String,
    version: String,
}

#[derive(Serialize)]
struct PublishLockfileStatus {
    path: String,
    status: String,
}

fn render_metadata_json(graphs: &[Graph]) -> Result<String> {
    if graphs.is_empty() {
        miette::bail!("no package graphs selected");
    }
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    let mut packages = Vec::new();
    let mut dependencies = Vec::new();
    let mut seen_edges = BTreeSet::new();
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
                id: node.id.clone(),
                name: node.name.clone(),
                version: node.manifest.package.version.clone(),
                source: metadata_source(&root.root_dir, node),
                yanked: registry_metadata(node).map(|metadata| metadata.yanked),
                yank_reason: registry_metadata(node)
                    .and_then(|metadata| metadata.yank_reason.clone()),
                features: registry_metadata(node).map(|metadata| metadata.features.clone()),
                manifest: slash_path(&node.manifest_path),
                root: slash_path(&node.root_dir),
                entry: slash_path(&node.entry_path),
                tests: node
                    .manifest
                    .test
                    .iter()
                    .map(|target| slash_path(&target.path))
                    .collect(),
            });
        }
        for edge in &graph.edges {
            if seen_edges.insert((edge.from.clone(), edge.alias.clone(), edge.to.clone())) {
                dependencies.push(MetadataDependency {
                    from: edge.from.clone(),
                    alias: edge.alias.clone(),
                    to: edge.to.clone(),
                });
            }
        }
    }
    let payload = MetadataPayload {
        schema_version: 2,
        workspace: graphs.len() > 1,
        root: if graphs.len() == 1 {
            roots.first().cloned()
        } else {
            None
        },
        roots,
        packages,
        dependencies,
    };
    serde_json::to_string_pretty(&payload)
        .map_err(|err| miette::miette!("failed to encode metadata json: {}", err))
}

fn render_publish_metadata_json(
    graph: &Graph,
    artifact: &package::PackageArtifact,
    registry: Option<&str>,
    workspace_package: Option<&str>,
    locked: bool,
) -> Result<String> {
    let root = graph
        .root_package()
        .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
    let lockfile_path = root.root_dir.join("Sengoo.lock");
    let payload = PublishMetadataPayload {
        schema_version: 1,
        package: PublishPackageIdentity {
            name: root.name.clone(),
            version: root.manifest.package.version.clone(),
        },
        manifest: slash_path(&root.manifest_path),
        archive_path: slash_path(&artifact.archive_path),
        checksum_path: slash_path(&artifact.checksum_path),
        sha256: artifact.checksum.clone(),
        included_file_count: artifact.included_file_count,
        excluded_file_count: artifact.excluded_file_count,
        lockfile: PublishLockfileStatus {
            path: slash_path(&lockfile_path),
            status: if locked { "current" } else { "unchecked" }.to_string(),
        },
        registry: registry.map(str::to_string),
        workspace_package: workspace_package.map(str::to_string),
    };
    serde_json::to_string_pretty(&payload)
        .map_err(|err| miette::miette!("failed to encode publish metadata json: {}", err))
}

fn metadata_source(base_dir: &Path, node: &resolver::PackageNode) -> MetadataSource {
    match &node.source {
        resolver::PackageSource::Path => MetadataSource {
            kind: "path".to_string(),
            path: Some(resolver::canonical_path_source(base_dir, &node.root_dir)),
            url: None,
            rev: None,
            registry: None,
            version: None,
        },
        resolver::PackageSource::Git { url, rev } => MetadataSource {
            kind: "git".to_string(),
            path: None,
            url: Some(url.clone()),
            rev: Some(rev.clone()),
            registry: None,
            version: None,
        },
        resolver::PackageSource::Registry {
            registry, version, ..
        } => MetadataSource {
            kind: "registry".to_string(),
            path: None,
            url: None,
            rev: None,
            registry: Some(registry.clone()),
            version: Some(version.clone()),
        },
    }
}

fn registry_metadata(node: &resolver::PackageNode) -> Option<&RegistryPackageMetadata> {
    match &node.source {
        resolver::PackageSource::Registry { metadata, .. } => Some(metadata),
        _ => None,
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
