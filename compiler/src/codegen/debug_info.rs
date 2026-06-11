use super::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugInfoConfig {
    pub enabled: bool,
    pub source_file: Option<String>,
    pub source_text: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DebugLocationIds {
    pub(crate) subprogram: u32,
    pub(crate) location: u32,
}

impl DebugInfoConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn for_source(source_file: impl Into<String>, source_text: impl Into<String>) -> Self {
        Self {
            enabled: true,
            source_file: Some(source_file.into()),
            source_text: Some(source_text.into()),
        }
    }
}

impl Codegen {
    pub(super) fn debug_enabled(&self) -> bool {
        self.debug_info.enabled
    }

    pub(super) fn debug_location_suffix(&self, function_name: &str) -> String {
        self.debug_locations
            .get(function_name)
            .map(|ids| format!(", !dbg !{}", ids.location))
            .unwrap_or_default()
    }

    pub(super) fn debug_subprogram_suffix(&self, function_name: &str) -> String {
        self.debug_locations
            .get(function_name)
            .map(|ids| format!(" !dbg !{}", ids.subprogram))
            .unwrap_or_default()
    }

    pub(super) fn emit_debug_metadata(&mut self, mir_fns: &[MirFunction]) {
        self.debug_locations.clear();
        if !self.debug_enabled() {
            return;
        }

        let source_file = self
            .debug_info
            .source_file
            .clone()
            .unwrap_or_else(|| "input.sg".to_string());
        let (filename, directory) = split_debug_source_path(&source_file);
        let filename = escape_metadata_string(&filename);
        let directory = escape_metadata_string(&directory);

        self.declarations.push_str("!llvm.dbg.cu = !{!0}\n");
        self.declarations
            .push_str("!llvm.module.flags = !{!4, !5}\n");
        self.declarations.push_str("!llvm.ident = !{!6}\n");
        self.declarations.push_str(&format!(
            "!0 = distinct !DICompileUnit(language: DW_LANG_C99, file: !1, producer: \"Sengoo Compiler\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)\n\
             !1 = !DIFile(filename: \"{}\", directory: \"{}\")\n\
             !2 = !DISubroutineType(types: !3)\n\
             !3 = !{{}}\n\
             !4 = !{{i32 2, !\"Dwarf Version\", i32 4}}\n\
             !5 = !{{i32 2, !\"Debug Info Version\", i32 3}}\n\
             !6 = !{{!\"Sengoo Compiler\"}}\n",
            filename, directory
        ));

        let mut next_id = 7u32;
        for mir_fn in mir_fns {
            let subprogram = next_id;
            let location = next_id + 1;
            next_id += 2;
            let line = self.function_line(&mir_fn.name).max(1);
            let debug_name = escape_metadata_string(&mir_fn.name);
            let linkage_name = escape_metadata_string(self.emitted_function_name(&mir_fn.name));
            self.declarations.push_str(&format!(
                "!{subprogram} = distinct !DISubprogram(name: \"{debug_name}\", linkageName: \"{linkage_name}\", scope: !1, file: !1, line: {line}, type: !2, scopeLine: {line}, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !3)\n\
                 !{location} = !DILocation(line: {line}, column: 1, scope: !{subprogram})\n"
            ));
            self.debug_locations.insert(
                mir_fn.name.clone(),
                DebugLocationIds {
                    subprogram,
                    location,
                },
            );
        }
        self.declarations.push('\n');
    }

    fn function_line(&self, function_name: &str) -> u32 {
        let Some(source_text) = self.debug_info.source_text.as_deref() else {
            return 1;
        };
        let needle = format!("{function_name}(");
        for (index, line) in source_text.lines().enumerate() {
            let trimmed = line.trim_start();
            if (trimmed.starts_with("def ") || trimmed.starts_with("async def "))
                && trimmed.contains(&needle)
            {
                return index.saturating_add(1) as u32;
            }
        }
        1
    }
}

fn split_debug_source_path(source_file: &str) -> (String, String) {
    let normalized = source_file.replace('\\', "/");
    let path = Path::new(&normalized);
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(source_file)
        .to_string();
    let directory = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .replace('\\', "/");
    (filename, directory)
}

fn escape_metadata_string(value: &str) -> String {
    value.replace('\\', "\\5C").replace('"', "\\22")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_metadata_records_function_line() {
        let mut codegen = Codegen::with_ffi_target_and_debug(
            FfiCodegenConfig::default(),
            None,
            DebugInfoConfig::for_source(
                "src/main.sg",
                "def helper() -> i64 { 1 }\n\ndef main() -> i64 { helper() }\n",
            ),
        );
        let mir_fn = MirFunction::new("main".to_string(), vec![], MIRType::Int(64));

        codegen.emit_debug_metadata(&[mir_fn]);

        assert!(codegen.declarations.contains("!DICompileUnit"));
        assert!(codegen.declarations.contains("name: \"main\""));
        assert!(codegen.declarations.contains("line: 3"));
    }
}
