use crate::resolver::{render_tree, Graph, PackageNode};
use miette::{Context, IntoDiagnostic, Result};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const MODULE_MAP_ENV: &str = "SENGOO_MODULE_MAP";

#[derive(Debug, Clone, Copy)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn opt_level(self) -> &'static str {
        match self {
            Self::Debug => "0",
            Self::Release => "2",
        }
    }

    fn dir_name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toolchain {
    sgc: PathBuf,
    sgfmt: Option<PathBuf>,
}

impl Toolchain {
    pub fn discover() -> Result<Self> {
        Ok(Self {
            sgc: find_tool("SGPM_SGC", "sgc")?,
            sgfmt: find_optional_tool("SGPM_SGFMT", "sgfmt"),
        })
    }

    pub fn build(&self, graph: &Graph, profile: BuildProfile, verbose: bool) -> Result<()> {
        for node in &graph.nodes {
            if node.manifest.lib.is_some() && node.manifest.bin.is_none() {
                let mut command = Command::new(&self.sgc);
                command
                    .current_dir(&node.root_dir)
                    .arg("check")
                    .arg(&node.entry_path);
                configure_module_map(&mut command, graph, node, true)?;

                if verbose {
                    eprintln!("sgpm: {}", render_command(&command));
                }

                run_command(
                    command,
                    &format!("check failed for library package '{}'", node.name),
                )?;
                println!("checked library {}", node.name);
                continue;
            }

            let output = package_output_path(node, profile)?;
            ensure_parent(&output)?;

            let mut command = Command::new(&self.sgc);
            command
                .current_dir(&node.root_dir)
                .arg("build")
                .arg(&node.entry_path)
                .arg("--output")
                .arg(&output)
                .arg("-O")
                .arg(profile.opt_level());
            configure_module_map(&mut command, graph, node, true)?;

            if verbose {
                eprintln!("sgpm: {}", render_command(&command));
            }

            run_command(
                command,
                &format!("build failed for package '{}'", node.name),
            )?;
            println!("built {} -> {}", node.name, output.display());
        }

        Ok(())
    }

    pub fn check(&self, graph: &Graph, verbose: bool) -> Result<()> {
        for node in &graph.nodes {
            let mut command = Command::new(&self.sgc);
            command
                .current_dir(&node.root_dir)
                .arg("check")
                .arg(&node.entry_path);
            configure_module_map(&mut command, graph, node, true)?;

            if verbose {
                eprintln!("sgpm: {}", render_command(&command));
            }

            run_command(
                command,
                &format!("check failed for package '{}'", node.name),
            )?;
            println!("checked {}", node.name);
        }

        Ok(())
    }

    pub fn run(
        &self,
        graph: &Graph,
        profile: BuildProfile,
        args: &[String],
        verbose: bool,
    ) -> Result<()> {
        let root = graph
            .root_package()
            .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
        if root.manifest.bin.is_none() {
            miette::bail!(
                "cannot run library package '{}'; add [bin] to Sengoo.toml",
                root.name
            );
        }

        self.build(graph, profile, verbose)?;

        let output = package_output_path(root, profile)?;

        let mut command = Command::new(&output);
        command.current_dir(&root.root_dir);
        command.args(args);

        if verbose {
            eprintln!("sgpm: {}", render_command(&command));
        }

        run_command(command, &format!("run failed for package '{}'", root.name))
    }

    pub fn test(&self, graph: &Graph, profile: BuildProfile, verbose: bool) -> Result<()> {
        let mut ran = 0usize;
        for node in &graph.nodes {
            let mut command = Command::new(&self.sgc);
            command
                .current_dir(&node.root_dir)
                .arg("test")
                .arg("--manifest-path")
                .arg(&node.manifest_path);
            if matches!(profile, BuildProfile::Release) {
                command.arg("--release");
            }
            configure_module_map(&mut command, graph, node, true)?;

            if verbose {
                eprintln!("sgpm: {}", render_command(&command));
            }

            run_command(command, &format!("test failed for package '{}'", node.name))?;
            ran += 1;
        }

        if ran == 0 {
            println!("no Sengoo packages found");
        }

        Ok(())
    }

    pub fn fmt(&self, graph: &Graph, check: bool, verbose: bool) -> Result<()> {
        let sgfmt = self.sgfmt.as_ref().ok_or_else(|| {
            miette::miette!("sgfmt not found; set SGPM_SGFMT or add sgfmt to PATH")
        })?;

        let mut formatted = 0usize;
        for node in &graph.nodes {
            for file in discover_format_files(&node.root_dir)? {
                let mut command = Command::new(sgfmt);
                command.current_dir(&node.root_dir).arg(&file);
                if check {
                    command.arg("--check");
                } else {
                    command.arg("--write");
                }

                if verbose {
                    eprintln!("sgpm: {}", render_command(&command));
                }

                run_command(command, &format!("format failed: {}", file.display()))?;
                formatted += 1;
            }
        }

        println!(
            "{} {} Sengoo source file(s)",
            if check { "checked" } else { "formatted" },
            formatted
        );
        Ok(())
    }

    pub fn doc(&self, graph: &Graph, output: Option<&Path>, verbose: bool) -> Result<()> {
        let root = graph
            .root_package()
            .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
        let base_output = output
            .map(PathBuf::from)
            .unwrap_or_else(|| root.root_dir.join("target/doc"));
        let use_package_subdirs = graph.nodes.len() > 1;

        for node in &graph.nodes {
            let input = package_doc_entry_path(node);
            let output_dir = if use_package_subdirs {
                base_output.join(&node.name)
            } else {
                base_output.clone()
            };
            let mut command = Command::new(&self.sgc);
            command
                .current_dir(&node.root_dir)
                .arg("doc")
                .arg(&input)
                .arg("--output")
                .arg(&output_dir);
            configure_module_map(&mut command, graph, node, true)?;

            if verbose {
                eprintln!("sgpm: {}", render_command(&command));
            }

            run_command(command, &format!("doc failed for package '{}'", node.name))?;
            println!("documented {} -> {}", node.name, output_dir.display());
        }

        Ok(())
    }
}

pub fn print_tree(graph: &Graph) {
    println!("{}", render_tree(graph));
}

pub fn clean(graph: &Graph) -> Result<()> {
    let root = graph
        .root_package()
        .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
    let target = root.root_dir.join("target");
    if target.exists() {
        fs::remove_dir_all(&target)
            .into_diagnostic()
            .with_context(|| format!("failed to remove {}", target.display()))?;
        println!("removed {}", target.display());
    } else {
        println!("nothing to clean");
    }
    Ok(())
}

fn package_output_path(node: &PackageNode, profile: BuildProfile) -> Result<PathBuf> {
    let target_name = node
        .manifest
        .bin
        .as_ref()
        .and_then(|bin| bin.name.as_deref())
        .unwrap_or(&node.name);
    let mut output = node
        .root_dir
        .join("target")
        .join(profile.dir_name())
        .join(target_name);
    if !std::env::consts::EXE_SUFFIX.is_empty() {
        output.set_extension(std::env::consts::EXE_EXTENSION);
    }
    Ok(output)
}

fn package_doc_entry_path(node: &PackageNode) -> PathBuf {
    if let Some(lib) = &node.manifest.lib {
        return node.root_dir.join(&lib.path);
    }
    node.entry_path.clone()
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn configure_module_map(
    command: &mut Command,
    graph: &Graph,
    node: &PackageNode,
    include_current: bool,
) -> Result<()> {
    command.env_remove(MODULE_MAP_ENV);
    if let Some(module_map) = module_map_value(graph, node, include_current)? {
        command.env(MODULE_MAP_ENV, module_map);
    }
    Ok(())
}

fn module_map_value(
    graph: &Graph,
    node: &PackageNode,
    include_current: bool,
) -> Result<Option<OsString>> {
    let mut entries = Vec::new();
    for edge in &graph.edges {
        if edge.from != node.id {
            continue;
        }
        let Some(dep) = graph.node_by_id(&edge.to) else {
            continue;
        };
        let Some(lib) = dep.manifest.lib.as_ref() else {
            continue;
        };
        entries.push(format!(
            "{}={}",
            edge.alias,
            portable_path(&dep.root_dir.join(&lib.path))
        ));
    }
    if include_current {
        if let Some(lib) = node.manifest.lib.as_ref() {
            entries.push(format!(
                "{}={}",
                node.name,
                portable_path(&node.root_dir.join(&lib.path))
            ));
        }
    }
    if entries.is_empty() {
        return Ok(None);
    }
    env::join_paths(entries)
        .map(Some)
        .into_diagnostic()
        .context("failed to encode dependency library module map")
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string()
}

fn discover_format_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for dir_name in ["src", "tests"] {
        let dir = root.join(dir_name);
        if dir.exists() {
            files.extend(collect_sg_files(&dir)?);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_sg_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.into_diagnostic())
        .collect::<Result<Vec<_>>>()
        .with_context(|| format!("failed to enumerate Sengoo files under {}", root.display()))?
        .into_iter()
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("sg"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn run_command(mut command: Command, context: &str) -> Result<()> {
    let status = command
        .status()
        .into_diagnostic()
        .with_context(|| format!("failed to start {}", render_command(&command)))?;
    if !status.success() {
        miette::bail!("{} (exit status: {})", context, status);
    }
    Ok(())
}

fn find_tool(env_name: &str, tool: &str) -> Result<PathBuf> {
    if let Some(path) = env::var_os(env_name).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Ok(path) = which::which(tool) {
        return Ok(path);
    }
    workspace_tool_path(tool).ok_or_else(|| {
        miette::miette!(
            "{} not found; set {} or add {} to PATH",
            tool,
            env_name,
            tool
        )
    })
}

fn find_optional_tool(env_name: &str, tool: &str) -> Option<PathBuf> {
    if let Some(path) = env::var_os(env_name).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    which::which(tool)
        .ok()
        .or_else(|| workspace_tool_path(tool))
}

fn workspace_tool_path(tool: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent()?.parent()?;
    for profile in ["debug", "release"] {
        let mut path = workspace.join("target").join(profile).join(tool);
        if !std::env::consts::EXE_SUFFIX.is_empty() {
            path.set_extension(std::env::consts::EXE_EXTENSION);
        }
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn render_command(command: &Command) -> String {
    let mut parts = Vec::new();
    parts.push(command.get_program().to_string_lossy().to_string());
    parts.extend(command.get_args().map(render_arg));
    parts.join(" ")
}

fn render_arg(arg: &std::ffi::OsStr) -> String {
    let text = arg.to_string_lossy();
    if text.contains(' ') {
        format!("\"{}\"", text)
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("sgpm_runner_{}_{}", name, stamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn collect_sg_files_is_sorted_and_filtered() {
        let dir = temp_dir("collect");
        fs::create_dir_all(dir.join("src/nested")).unwrap();
        fs::write(dir.join("src/b.sg"), "").unwrap();
        fs::write(dir.join("src/a.txt"), "").unwrap();
        fs::write(dir.join("src/nested/a.sg"), "").unwrap();

        let files = collect_sg_files(&dir.join("src")).unwrap();
        let names = files
            .iter()
            .map(|path| {
                path.strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["src/b.sg", "src/nested/a.sg"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collect_sg_files_rejects_missing_root() {
        let dir = temp_dir("collect_missing");
        let err = collect_sg_files(&dir.join("missing")).unwrap_err();
        assert!(err.to_string().contains("failed to enumerate Sengoo files"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn render_command_quotes_spaced_args() {
        let mut command = Command::new("sgc");
        command
            .arg("build")
            .arg(PathBuf::from("src/hello world.sg"));
        assert_eq!(render_command(&command), "sgc build \"src/hello world.sg\"");
    }

    #[test]
    fn package_output_uses_profile_directory() {
        let node = PackageNode {
            id: "demo@0.1.0+path:.".to_string(),
            name: "demo".to_string(),
            manifest_path: PathBuf::from("Sengoo.toml"),
            root_dir: PathBuf::from("pkg"),
            entry_path: PathBuf::from("pkg/src/main.sg"),
            manifest: crate::manifest::Manifest::parse(
                "[package]\nname = 'demo'\nversion = '0.1.0'\n",
            )
            .unwrap(),
            source: crate::resolver::PackageSource::Path,
        };
        let output = package_output_path(&node, BuildProfile::Release).unwrap();
        assert!(output.to_string_lossy().contains("target"));
        assert!(output.to_string_lossy().contains("release"));
    }
}
