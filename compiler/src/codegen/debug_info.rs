use super::*;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugInfoConfig {
    pub enabled: bool,
    pub source_file: Option<String>,
    pub source_text: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DebugLocationIds {
    pub(crate) subprogram: u32,
    entry_line: u32,
    entry_location: u32,
    line_locations: BTreeMap<u32, u32>,
    local_lines: HashMap<String, u32>,
    last_statement_line: u32,
    current_line: Cell<u32>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SourceFunctionDebugPlan {
    entry_line: u32,
    statement_lines: Vec<u32>,
    local_lines: HashMap<String, u32>,
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

    pub(super) fn debug_subprogram_suffix(&self, function_name: &str) -> String {
        self.debug_locations
            .get(function_name)
            .map(|ids| {
                ids.current_line.set(ids.entry_line);
                format!(" !dbg !{}", ids.subprogram)
            })
            .unwrap_or_default()
    }

    pub(super) fn debug_instruction_location_suffix(
        &self,
        mir_fn: &MirFunction,
        inst_id: mir::InstId,
        inst: &mir::Instruction,
    ) -> String {
        if mir_fn.debug_hidden_instructions.contains(&inst_id) {
            return String::new();
        }
        let Some(ids) = self.debug_locations.get(&mir_fn.name) else {
            return String::new();
        };
        let source_line = mir_fn
            .instruction_source_sites
            .get(inst_id.0 as usize)
            .copied()
            .flatten()
            .and_then(|site| self.debug_line_for_site(site));
        if let Some(line) = source_line {
            ids.current_line.set(line);
        } else {
            let source_local = match inst {
                mir::Instruction::Store { destination, .. } => Some(*destination),
                _ => inst.destination(),
            };
            if let Some(line) = source_local
                .filter(|local| local.kind == LocalKind::User)
                .and_then(|local| mir_fn.local_debug_names.get(&local.index()))
                .and_then(|name| ids.local_lines.get(name))
                .copied()
            {
                ids.current_line.set(line);
            }
        }
        format!(", !dbg !{}", ids.location_for_line(ids.current_line.get()))
    }

    pub(super) fn debug_terminator_location_suffix(
        &self,
        mir_fn: &MirFunction,
        block_id: usize,
        terminator: &mir::Terminator,
    ) -> String {
        let Some(ids) = self.debug_locations.get(&mir_fn.name) else {
            return String::new();
        };
        let source_line = mir_fn
            .basic_blocks
            .get(block_id)
            .and_then(|block| block.terminator_source_site)
            .and_then(|site| self.debug_line_for_site(site));
        if let Some(line) = source_line {
            ids.current_line.set(line);
        } else if matches!(terminator, mir::Terminator::Return(_)) {
            ids.current_line.set(ids.last_statement_line);
        }
        format!(", !dbg !{}", ids.location_for_line(ids.current_line.get()))
    }

    pub(super) fn attach_debug_location_to_segment(&mut self, start: usize, suffix: &str) {
        if suffix.is_empty() || start >= self.ir.len() || self.ir[start..].contains("!dbg !") {
            return;
        }
        let segment = &self.ir[start..];
        let line_end = segment
            .strip_suffix('\n')
            .and_then(|without_trailing_newline| without_trailing_newline.rfind('\n'))
            .map_or(start, |offset| start + offset + 1);
        let insert_at = self.ir[line_end..]
            .find('\n')
            .map_or(self.ir.len(), |offset| line_end + offset);
        self.ir.insert_str(insert_at, suffix);
    }

    pub(super) fn emit_debug_metadata(&mut self, mir_fns: &[MirFunction]) {
        self.debug_locations.clear();
        self.debug_type_ids.clear();
        self.debug_expression_id = None;
        self.debug_next_metadata_id = 0;
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
            .push_str("declare void @llvm.dbg.declare(metadata, metadata, metadata)\n");
        self.declarations
            .push_str("declare void @llvm.dbg.value(metadata, metadata, metadata)\n");
        self.declarations
            .push_str("!llvm.module.flags = !{!4, !5}\n");
        self.declarations.push_str("!llvm.ident = !{!6}\n");
        let platform_debug_flag = if self.uses_codeview_debug_info() {
            "CodeView\", i32 1"
        } else {
            "Dwarf Version\", i32 4"
        };
        self.declarations.push_str(&format!(
            "!0 = distinct !DICompileUnit(language: DW_LANG_C99, file: !1, producer: \"Sengoo Compiler\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)\n\
             !1 = !DIFile(filename: \"{}\", directory: \"{}\")\n\
             !2 = !DISubroutineType(types: !3)\n\
             !3 = !{{}}\n\
             !4 = !{{i32 2, !\"{platform_debug_flag}}}\n\
             !5 = !{{i32 2, !\"Debug Info Version\", i32 3}}\n\
             !6 = !{{!\"Sengoo Compiler\"}}\n",
            filename, directory
        ));

        let expression_id = 7u32;
        self.declarations
            .push_str(&format!("!{expression_id} = !DIExpression()\n"));
        self.debug_expression_id = Some(expression_id);

        let mut next_id = 8u32;
        for mir_fn in mir_fns {
            let subprogram = next_id;
            let entry_location = next_id + 1;
            next_id += 2;
            let plan = self.function_debug_plan(mir_fn);
            let line = plan.entry_line.max(1);
            let debug_name = escape_metadata_string(&mir_fn.name);
            let linkage_name = escape_metadata_string(self.emitted_function_name(&mir_fn.name));
            self.declarations.push_str(&format!(
                "!{subprogram} = distinct !DISubprogram(name: \"{debug_name}\", linkageName: \"{linkage_name}\", scope: !1, file: !1, line: {line}, type: !2, scopeLine: {line}, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !3)\n\
                 !{entry_location} = !DILocation(line: {line}, column: 1, scope: !{subprogram})\n"
            ));
            let mut line_locations = BTreeMap::from([(line, entry_location)]);
            for statement_line in plan.statement_lines.iter().copied() {
                if line_locations.contains_key(&statement_line) {
                    continue;
                }
                let location = next_id;
                next_id += 1;
                self.declarations.push_str(&format!(
                    "!{location} = !DILocation(line: {statement_line}, column: 1, scope: !{subprogram})\n"
                ));
                line_locations.insert(statement_line, location);
            }
            let last_statement_line = plan.statement_lines.last().copied().unwrap_or(line);
            self.debug_locations.insert(
                mir_fn.name.clone(),
                DebugLocationIds {
                    subprogram,
                    entry_line: line,
                    entry_location,
                    line_locations,
                    local_lines: plan.local_lines,
                    last_statement_line,
                    current_line: Cell::new(line),
                },
            );
        }
        self.debug_next_metadata_id = next_id;
        self.declarations.push('\n');
    }

    fn uses_codeview_debug_info(&self) -> bool {
        self.target_triple
            .as_deref()
            .map_or(cfg!(windows), |triple| triple.contains("windows-msvc"))
    }

    fn function_debug_plan(&self, mir_fn: &MirFunction) -> SourceFunctionDebugPlan {
        let Some(source_text) = self.debug_info.source_text.as_deref() else {
            return SourceFunctionDebugPlan {
                entry_line: 1,
                ..SourceFunctionDebugPlan::default()
            };
        };
        let mut plan = source_function_debug_plan(source_text, &mir_fn.name).unwrap_or(
            SourceFunctionDebugPlan {
                entry_line: 1,
                ..SourceFunctionDebugPlan::default()
            },
        );
        let statement_lines = mir_fn
            .instruction_source_sites
            .iter()
            .copied()
            .flatten()
            .chain(
                mir_fn
                    .basic_blocks
                    .iter()
                    .filter_map(|block| block.terminator_source_site),
            )
            .filter_map(|site| source_line_for_site(source_text, site))
            .collect::<BTreeSet<_>>();
        if !statement_lines.is_empty() {
            plan.statement_lines = statement_lines.into_iter().collect();
        }
        plan
    }

    fn debug_line_for_site(&self, site: u32) -> Option<u32> {
        source_line_for_site(self.debug_info.source_text.as_deref()?, site)
    }

    fn debug_local_location(&self, mir_fn: &MirFunction, name: &str) -> Option<(u32, u32, u32)> {
        let ids = self.debug_locations.get(&mir_fn.name)?;
        let line = ids.local_lines.get(name).copied().unwrap_or(ids.entry_line);
        Some((ids.subprogram, line, ids.location_for_line(line)))
    }

    fn debug_entry_location(&self, mir_fn: &MirFunction) -> Option<(u32, u32, u32)> {
        let ids = self.debug_locations.get(&mir_fn.name)?;
        Some((ids.subprogram, ids.entry_line, ids.entry_location))
    }

    pub(super) fn emit_debug_param_value(
        &mut self,
        mir_fn: &MirFunction,
        local: Local,
        ty: &MIRType,
        arg_index: usize,
    ) {
        if !self.debug_enabled() {
            return;
        }
        let Some(name) = mir_fn.local_debug_names.get(&local.index()).cloned() else {
            return;
        };
        let Some((subprogram, line, location)) = self.debug_entry_location(mir_fn) else {
            return;
        };
        let Some(expr_id) = self.debug_expression_id else {
            return;
        };
        let type_id = self.debug_type_id(ty);
        let var_id = self.alloc_debug_metadata_id();
        let name = escape_metadata_string(&name);
        self.declarations.push_str(&format!(
            "!{var_id} = !DILocalVariable(name: \"{name}\", arg: {arg_index}, scope: !{}, file: !1, line: {}, type: !{type_id})\n",
            subprogram, line
        ));
        let llvm_ty = self.mir_type_to_llvm_cached(ty);
        let location = format!(", !dbg !{location}");
        self.emit_indent();
        self.ir.push_str(&format!(
            "call void @llvm.dbg.value(metadata {llvm_ty} {}, metadata !{var_id}, metadata !{expr_id}){}\n",
            self.local_name(local),
            location
        ));
    }

    pub(super) fn emit_debug_local_declare(
        &mut self,
        mir_fn: &MirFunction,
        local: Local,
        ty: &MIRType,
    ) {
        if !self.debug_enabled() {
            return;
        }
        let Some(name) = mir_fn.local_debug_names.get(&local.index()).cloned() else {
            return;
        };
        let Some((subprogram, line, location)) = self.debug_local_location(mir_fn, &name) else {
            return;
        };
        let Some(expr_id) = self.debug_expression_id else {
            return;
        };
        let type_id = self.debug_type_id(ty);
        let var_id = self.alloc_debug_metadata_id();
        let name = escape_metadata_string(&name);
        self.declarations.push_str(&format!(
            "!{var_id} = !DILocalVariable(name: \"{name}\", scope: !{}, file: !1, line: {}, type: !{type_id})\n",
            subprogram, line
        ));
        let llvm_ty = self.mir_type_to_llvm_cached(ty);
        let location = format!(", !dbg !{location}");
        self.emit_indent();
        self.ir.push_str(&format!(
            "call void @llvm.dbg.declare(metadata {llvm_ty}* {}, metadata !{var_id}, metadata !{expr_id}){}\n",
            self.local_name(local),
            location
        ));
    }

    fn alloc_debug_metadata_id(&mut self) -> u32 {
        let id = self.debug_next_metadata_id;
        self.debug_next_metadata_id += 1;
        id
    }

    fn debug_type_id(&mut self, ty: &MIRType) -> u32 {
        if let Some(id) = self.debug_type_ids.get(ty) {
            return *id;
        }
        let id = self.alloc_debug_metadata_id();
        self.debug_type_ids.insert(ty.clone(), id);
        let metadata = self.debug_type_metadata(id, ty);
        self.declarations.push_str(&metadata);
        id
    }

    fn debug_aggregate_metadata(
        &mut self,
        id: u32,
        name: &str,
        fields: &[(String, MIRType)],
    ) -> String {
        let elements_id = self.alloc_debug_metadata_id();
        let mut member_ids = Vec::with_capacity(fields.len());
        let mut members = String::new();
        let mut offset = 0u64;

        for (field_name, field_ty) in fields {
            let (field_size, field_alignment) = common::mir_type_size_align(field_ty);
            offset = debug_align_to(offset, field_alignment);
            let field_type_id = self.debug_type_id(field_ty);
            let member_id = self.alloc_debug_metadata_id();
            member_ids.push(member_id);
            members.push_str(&format!(
                "!{member_id} = !DIDerivedType(tag: DW_TAG_member, name: \"{}\", scope: !{id}, file: !1, baseType: !{field_type_id}, size: {}, offset: {})\n",
                escape_metadata_string(field_name),
                field_size * 8,
                offset * 8
            ));
            offset += field_size;
        }

        let (size, _) = common::mir_type_size_align(&MIRType::Struct {
            name: name.to_string(),
            fields: fields.to_vec(),
        });
        let elements = member_ids
            .iter()
            .map(|member_id| format!("!{member_id}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "!{id} = !DICompositeType(tag: DW_TAG_structure_type, name: \"{}\", file: !1, size: {}, elements: !{elements_id})\n!{elements_id} = !{{{elements}}}\n{members}",
            escape_metadata_string(name),
            size * 8
        )
    }

    fn debug_type_metadata(&mut self, id: u32, ty: &MIRType) -> String {
        match ty {
            MIRType::Bool => {
                format!("!{id} = !DIBasicType(name: \"bool\", size: 8, encoding: DW_ATE_boolean)\n")
            }
            MIRType::Int(bits) => format!(
                "!{id} = !DIBasicType(name: \"i{bits}\", size: {bits}, encoding: DW_ATE_signed)\n"
            ),
            MIRType::UInt(bits) => format!(
                "!{id} = !DIBasicType(name: \"u{bits}\", size: {bits}, encoding: DW_ATE_unsigned)\n"
            ),
            MIRType::Float(bits) => format!(
                "!{id} = !DIBasicType(name: \"f{bits}\", size: {bits}, encoding: DW_ATE_float)\n"
            ),
            MIRType::Ptr(inner) | MIRType::Ref(inner) => {
                let base = self.debug_type_id(inner);
                format!("!{id} = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !{base}, size: 64)\n")
            }
            MIRType::Struct { name, fields } => self.debug_aggregate_metadata(id, name, fields),
            MIRType::Enum {
                name: _,
                discr_type,
                variants: _,
            } => {
                let payload_size = common::enum_payload_storage_size(ty);
                let fields = vec![
                    ("discriminant".to_string(), discr_type.as_ref().clone()),
                    (
                        "payload".to_string(),
                        MIRType::Array(Box::new(MIRType::UInt(8)), payload_size),
                    ),
                ];
                self.debug_aggregate_metadata(id, "enum", &fields)
            }
            MIRType::Array(elem, len) => {
                let base = self.debug_type_id(elem);
                let (size, _) = common::mir_type_size_align(ty);
                let elements_id = self.alloc_debug_metadata_id();
                let subrange_id = self.alloc_debug_metadata_id();
                format!(
                    "!{id} = !DICompositeType(tag: DW_TAG_array_type, baseType: !{base}, size: {}, elements: !{elements_id})\n!{elements_id} = !{{!{subrange_id}}}\n!{subrange_id} = !DISubrange(count: {len})\n",
                    size * 8,
                )
            }
            MIRType::Tuple(fields) => {
                let fields = fields
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| (index.to_string(), ty.clone()))
                    .collect::<Vec<_>>();
                self.debug_aggregate_metadata(id, "tuple", &fields)
            }
            MIRType::Fn { .. } => {
                format!(
                    "!{id} = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !2, size: 64)\n"
                )
            }
            MIRType::Future(_) => {
                format!(
                    "!{id} = !DIBasicType(name: \"Future\", size: 64, encoding: DW_ATE_unsigned)\n"
                )
            }
            MIRType::Unit | MIRType::Never => {
                format!(
                    "!{id} = !DIBasicType(name: \"unit\", size: 0, encoding: DW_ATE_unsigned)\n"
                )
            }
        }
    }
}

impl DebugLocationIds {
    fn location_for_line(&self, line: u32) -> u32 {
        self.line_locations
            .get(&line)
            .copied()
            .unwrap_or(self.entry_location)
    }
}

fn source_function_debug_plan(
    source: &str,
    function_name: &str,
) -> Option<SourceFunctionDebugPlan> {
    let needle = format!("{function_name}(");
    let lines = source.lines().collect::<Vec<_>>();
    let (entry_index, entry_source) = lines.iter().enumerate().find(|(_, line)| {
        let trimmed = line.trim_start();
        (trimmed.starts_with("def ") || trimmed.starts_with("async def "))
            && trimmed.contains(&needle)
    })?;
    let entry_line = u32::try_from(entry_index + 1).unwrap_or(u32::MAX);
    let mut plan = SourceFunctionDebugPlan {
        entry_line,
        ..SourceFunctionDebugPlan::default()
    };
    let mut brace_depth = debug_brace_delta(entry_source);
    if brace_depth <= 0 {
        return Some(plan);
    }

    for (index, source_line) in lines.iter().enumerate().skip(entry_index + 1) {
        let trimmed = debug_source_before_comment(source_line).trim().to_string();
        let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let line_starts_in_body = brace_depth > 0;
        brace_depth += debug_brace_delta(source_line);

        if line_starts_in_body && is_debug_statement_line(&trimmed) {
            plan.statement_lines.push(line_number);
            if let Some(local_name) = debug_let_name(&trimmed) {
                plan.local_lines.insert(local_name.to_string(), line_number);
            }
        }
        if brace_depth <= 0 {
            break;
        }
    }
    Some(plan)
}

fn source_line_for_site(source: &str, site: u32) -> Option<u32> {
    let site = usize::try_from(site).ok()?;
    if site > source.len() || !source.is_char_boundary(site) {
        return None;
    }
    u32::try_from(
        source.as_bytes()[..site]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1,
    )
    .ok()
}

fn is_debug_statement_line(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && trimmed != "}"
        && trimmed != "};"
        && trimmed != "{"
        && !trimmed.starts_with("//")
}

fn debug_let_name(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("let ")?;
    let rest = rest.strip_prefix("mut ").unwrap_or(rest);
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (!(ch == '_' || ch.is_ascii_alphanumeric())).then_some(index))
        .unwrap_or(rest.len());
    (end > 0).then_some(&rest[..end])
}

fn debug_source_before_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\"' | b'\'') {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return &line[..index];
        }
        index += 1;
    }
    line
}

fn debug_brace_delta(line: &str) -> i32 {
    let bytes = line.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\"' | b'\'') {
            quote = Some(byte);
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        } else if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth -= 1;
        }
        index += 1;
    }
    depth
}

fn debug_align_to(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
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

    #[test]
    fn debug_metadata_selects_platform_format_from_target_triple() {
        let debug = DebugInfoConfig::for_source("src/main.sg", "def main() -> i64 { 0 }\n");
        let mir_fn = MirFunction::new("main".to_string(), vec![], MIRType::Int(64));
        let mut windows = Codegen::with_ffi_target_and_debug(
            FfiCodegenConfig::default(),
            Some("x86_64-pc-windows-msvc".to_string()),
            debug.clone(),
        );
        let mut linux = Codegen::with_ffi_target_and_debug(
            FfiCodegenConfig::default(),
            Some("x86_64-unknown-linux-gnu".to_string()),
            debug,
        );

        windows.emit_debug_metadata(std::slice::from_ref(&mir_fn));
        linux.emit_debug_metadata(&[mir_fn]);

        assert!(windows.declarations.contains("!\"CodeView\", i32 1"));
        assert!(!windows.declarations.contains("!\"Dwarf Version\""));
        assert!(linux.declarations.contains("!\"Dwarf Version\", i32 4"));
        assert!(!linux.declarations.contains("!\"CodeView\""));
    }

    #[test]
    fn source_debug_plan_tracks_statements_and_local_declarations() {
        let source = r#"// leading source
def probe(value: i64) -> i64 {
    let doubled = value * 2;
    let label = "brace { inside text }";
    let stepped = doubled + 1; // trailing comment }
    stepped
}

def main() -> i64 { probe(21) }
"#;

        let plan = source_function_debug_plan(source, "probe").expect("probe debug plan");

        assert_eq!(plan.entry_line, 2);
        assert_eq!(plan.statement_lines, vec![3, 4, 5, 6]);
        assert_eq!(plan.local_lines.get("doubled"), Some(&3));
        assert_eq!(plan.local_lines.get("label"), Some(&4));
        assert_eq!(plan.local_lines.get("stepped"), Some(&5));
    }
}
