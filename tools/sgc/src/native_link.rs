use miette::Result;
use sengoo_compiler::ast::DeclKind;
use sengoo_compiler::Parser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cross_compile::NativeBuildTarget;
use crate::module_graph::collect_module_sources_with_edges;
use crate::ModuleSourceInfo;

pub(crate) const NATIVE_LIB_DIRS_ENV: &str = "SENGOO_NATIVE_LIB_DIRS";
pub(crate) const SDL2_LIB_DIR_ENV: &str = "SENGOO_SDL2_LIB_DIR";
pub(crate) const SDL2_INCLUDE_DIR_ENV: &str = "SENGOO_SDL2_INCLUDE_DIR";
pub(crate) const SGPLATFORM_SKIP_GRAPHICS_ENV: &str = "SGPLATFORM_SKIP_GRAPHICS";

fn parse_booleanish_env(value: Option<&str>) -> bool {
    value
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && value != "0" && value != "false" && value != "no"
        })
        .unwrap_or(false)
}

pub(crate) fn sgplatform_graphics_skip_enabled() -> bool {
    let value = std::env::var(SGPLATFORM_SKIP_GRAPHICS_ENV).ok();
    parse_booleanish_env(value.as_deref())
}

fn collect_native_link_libraries_from_program(
    program: &sengoo_compiler::ast::Program,
) -> Vec<String> {
    let mut libraries = Vec::new();
    for decl in &program.decls {
        let DeclKind::ExternBlock(block) = &decl.kind else {
            continue;
        };
        let Some(name) = block.link_name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        if libraries.iter().any(|existing| existing == name) {
            continue;
        }
        libraries.push(name.to_string());
    }
    libraries
}

pub(crate) fn collect_native_link_libraries_from_source(source: &str) -> Result<Vec<String>> {
    if !source.contains("extern") {
        return Ok(Vec::new());
    }
    let program = Parser::parse(source)
        .map_err(|e| miette::miette!("failed to parse source for native link metadata: {}", e))?;
    Ok(collect_native_link_libraries_from_program(&program))
}

pub(crate) fn collect_native_link_libraries_for_graph(
    input_path: &Path,
    root_source: &str,
) -> Result<Vec<String>> {
    let module_sources = collect_module_sources_with_edges(input_path, root_source);
    let mut libraries = union_native_link_libraries_from_module_sources(&module_sources)?;
    if sgplatform_graphics_skip_enabled() {
        libraries.retain(|library| !library.eq_ignore_ascii_case("SDL2"));
    }
    Ok(libraries)
}

fn union_native_link_libraries_from_module_sources(
    module_sources: &BTreeMap<String, ModuleSourceInfo>,
) -> Result<Vec<String>> {
    let mut libraries = Vec::new();
    for info in module_sources.values() {
        for name in collect_native_link_libraries_from_source(info.source.as_ref())? {
            if libraries.iter().any(|existing| existing == &name) {
                continue;
            }
            libraries.push(name);
        }
    }
    Ok(libraries)
}

pub(crate) fn native_library_search_paths_from_env() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(value) = std::env::var(SDL2_LIB_DIR_ENV) {
        paths.extend(parse_path_list(&value));
    }
    if let Ok(value) = std::env::var(NATIVE_LIB_DIRS_ENV) {
        paths.extend(parse_path_list(&value));
    }
    paths
}

fn parse_path_list(value: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    let separator = ';';
    #[cfg(not(windows))]
    let separator = ':';

    value
        .split(separator)
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn uses_msvc_link_path_syntax(target: &NativeBuildTarget) -> bool {
    target.is_windows_msvc() && cfg!(windows) && !target.is_cross()
}

pub(crate) fn native_library_link_args(
    libraries: &[String],
    target: &NativeBuildTarget,
    search_paths: &[PathBuf],
) -> Vec<String> {
    let mut args = Vec::new();
    for dir in search_paths {
        if uses_msvc_link_path_syntax(target) {
            args.push(format!("/LIBPATH:{}", dir.display()));
        } else {
            args.push(format!("-L{}", dir.display()));
        }
    }
    for library in libraries {
        let arg = native_library_link_arg(library, target);
        if !arg.is_empty() {
            args.push(arg);
        }
    }
    args
}

fn native_library_link_arg(library: &str, target: &NativeBuildTarget) -> String {
    if target.is_windows_msvc() && library.eq_ignore_ascii_case("m") {
        return String::new();
    }
    if uses_msvc_link_path_syntax(target) {
        if library.ends_with(".lib") {
            library.to_string()
        } else {
            format!("{library}.lib")
        }
    } else {
        let name = library.strip_prefix("lib").unwrap_or(library);
        let name = name.strip_suffix(".lib").unwrap_or(name);
        let name = name.strip_suffix(".a").unwrap_or(name);
        format!("-l{name}")
    }
}

pub(crate) fn append_native_library_link_args(
    command: &mut Command,
    libraries: &[String],
    target: &NativeBuildTarget,
    search_paths: &[PathBuf],
) {
    for arg in native_library_link_args(libraries, target, search_paths) {
        command.arg(arg);
    }
}

pub(crate) fn format_native_link_failure_message(libraries: &[String]) -> String {
    if libraries
        .iter()
        .any(|library| library.eq_ignore_ascii_case("SDL2"))
    {
        return format!(
            "native link failed: could not link SDL2. Install the SDL2 development package and ensure the linker can find SDL2 (SDL2.lib on Windows, libSDL2 on Linux). Set {SDL2_LIB_DIR_ENV} or {NATIVE_LIB_DIRS_ENV} for extra search paths. See docs/sgplatform.md."
        );
    }
    if libraries.is_empty() {
        "compile failed".to_string()
    } else {
        format!(
            "native link failed: could not link native libraries ({}). Set {NATIVE_LIB_DIRS_ENV} for extra search paths. See docs/sgplatform.md.",
            libraries.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_library_link_args_use_dash_l_on_unix_targets() {
        if cfg!(windows) {
            return;
        }
        let target = NativeBuildTarget::host();
        let args = native_library_link_args(&["sample".to_string()], &target, &[]);
        assert_eq!(args, vec!["-lsample".to_string()]);
    }

    #[test]
    fn native_library_link_args_add_search_paths() {
        let target = NativeBuildTarget::host();
        let search_paths = vec![PathBuf::from("/opt/libs")];
        let args = native_library_link_args(&["sample".to_string()], &target, &search_paths);
        if cfg!(windows) {
            assert!(args
                .iter()
                .any(|arg| arg.contains("/LIBPATH:") || arg.contains("-L")));
        } else {
            assert_eq!(
                args,
                vec!["-L/opt/libs".to_string(), "-lsample".to_string()]
            );
        }
    }

    #[test]
    fn native_library_link_args_skip_libm_on_windows_msvc() {
        let target =
            NativeBuildTarget::resolve(Some(crate::cross_compile::REFERENCE_TARGET_WINDOWS_MSVC))
                .unwrap();
        let args = native_library_link_args(&["m".to_string()], &target, &[]);
        assert!(!args.iter().any(|arg| arg == "m.lib"));
    }

    #[test]
    fn collect_native_link_libraries_from_source_skips_typecheck_for_unlinked_extern() {
        let source = r#"
            import std::status;

            def helper() -> i64 {
                STATUS_OK()
            }

            extern "C" {
                fn sengoo_stdlib_str_ptr(value: &str) -> i64;
            }
        "#;
        let libraries = collect_native_link_libraries_from_source(source).unwrap();
        assert!(libraries.is_empty());
    }

    #[test]
    fn collect_native_link_libraries_from_source_reads_link_attribute() {
        let source = r#"
            #[link(name = "sample")]
            extern "C" {
                pub fn sample_ping() -> i64;
            }
        "#;
        let libraries = collect_native_link_libraries_from_source(source).unwrap();
        assert_eq!(libraries, vec!["sample".to_string()]);
    }

    #[test]
    fn format_native_link_failure_message_names_sdl2_doc() {
        let message = format_native_link_failure_message(&["SDL2".to_string()]);
        assert!(message.contains("SDL2"));
        assert!(message.contains("docs/sgplatform.md"));
    }

    #[test]
    fn sgplatform_graphics_skip_env_uses_booleanish_values() {
        assert!(parse_booleanish_env(Some("1")));
        assert!(parse_booleanish_env(Some("true")));
        assert!(parse_booleanish_env(Some("yes")));
        assert!(!parse_booleanish_env(Some("0")));
        assert!(!parse_booleanish_env(Some("false")));
        assert!(!parse_booleanish_env(Some("no")));
        assert!(!parse_booleanish_env(Some("")));
        assert!(!parse_booleanish_env(None));
    }
}
