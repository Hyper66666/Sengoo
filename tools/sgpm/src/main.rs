//! sgpm - offline Sengoo package/project manager MVP.

mod manifest;
mod resolver;
mod runner;
mod scaffold;

use clap::{Parser, Subcommand};
use miette::{Context, Result};
use resolver::Graph;
use runner::{BuildProfile, Toolchain};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "sgpm")]
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

    /// Print the resolved path-dependency tree.
    Tree(TreeArgs),

    /// Remove root package build artifacts.
    Clean(TreeArgs),
}

#[derive(Parser, Debug, Clone)]
struct TreeArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,
}

#[derive(Parser, Debug, Clone)]
struct PackageArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Use target/release and -O2 instead of target/debug and -O0.
    #[arg(long)]
    release: bool,

    /// Print delegated sgc/sgfmt commands.
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Parser, Debug, Clone)]
struct RunArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Use -O2 instead of the debug -O0 profile.
    #[arg(long)]
    release: bool,

    /// Print delegated sgc command.
    #[arg(short, long)]
    verbose: bool,

    /// Arguments forwarded to the Sengoo program.
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[derive(Parser, Debug, Clone)]
struct FmtArgs {
    /// Path to Sengoo.toml, or a package directory containing it.
    #[arg(long, default_value = "Sengoo.toml")]
    manifest_path: PathBuf,

    /// Check formatting without writing files.
    #[arg(long)]
    check: bool,

    /// Print delegated sgfmt commands.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, path } => {
            let root = scaffold::new_project(&name, path.as_deref())?;
            println!("created package '{}' at {}", name, root.display());
            println!("next: cd {} && sgpm check", root.display());
            Ok(())
        }
        Commands::Build(args) => {
            let graph = load_graph(&args.manifest_path)?;
            let toolchain = Toolchain::discover()?;
            toolchain.build(&graph, profile(args.release), args.verbose)
        }
        Commands::Check(args) => {
            let graph = load_graph(&args.manifest_path)?;
            let toolchain = Toolchain::discover()?;
            toolchain.check(&graph, args.verbose)
        }
        Commands::Run(args) => {
            let graph = load_graph(&args.manifest_path)?;
            let toolchain = Toolchain::discover()?;
            toolchain.run(&graph, profile(args.release), &args.args, args.verbose)
        }
        Commands::Test(args) => {
            let graph = load_graph(&args.manifest_path)?;
            let toolchain = Toolchain::discover()?;
            toolchain.test(&graph, args.verbose)
        }
        Commands::Fmt(args) => {
            let graph = load_graph(&args.manifest_path)?;
            let toolchain = Toolchain::discover()?;
            toolchain.fmt(&graph, args.check, args.verbose)
        }
        Commands::Tree(args) => {
            let graph = load_graph(&args.manifest_path)?;
            runner::print_tree(&graph);
            Ok(())
        }
        Commands::Clean(args) => {
            let graph = load_graph(&args.manifest_path)?;
            runner::clean(&graph)
        }
    }
}

fn load_graph(manifest_path: &PathBuf) -> Result<Graph> {
    Graph::from_root(manifest_path).with_context(|| {
        format!(
            "failed to resolve package graph from {}",
            manifest_path.display()
        )
    })
}

fn profile(release: bool) -> BuildProfile {
    if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    }
}
