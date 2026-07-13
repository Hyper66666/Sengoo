use crate::mir::{Instruction, Terminator};
use crate::{
    compile_to_mir, compile_to_mir_with_options, lower_ast_with_coverage, lower_hir_with_options,
    Codegen, CompileOptions, CoverageContext, DebugInfoConfig, FfiCodegenConfig, MirLowerOptions,
    Parser, TypeChecker,
};

fn line_for_site(source: &str, site: u32) -> usize {
    source.as_bytes()[..site as usize]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn debug_location_id(ir: &str, source_line: usize) -> String {
    let marker = format!("!DILocation(line: {source_line}, ");
    ir.lines()
        .find(|line| line.contains(&marker))
        .and_then(|line| line.split_once(" = ").map(|(id, _)| id.to_string()))
        .unwrap_or_else(|| panic!("missing debug location for source line {source_line}:\n{ir}"))
}

fn debug_location_ids(ir: &str, source_line: usize) -> Vec<String> {
    let marker = format!("!DILocation(line: {source_line}, ");
    ir.lines()
        .filter(|line| line.contains(&marker))
        .filter_map(|line| line.split_once(" = ").map(|(id, _)| id.to_string()))
        .collect()
}

fn has_instruction_location(ir: &str, instruction: &str, location: &str) -> bool {
    ir.lines()
        .any(|line| line.contains(instruction) && line.contains(&format!("!dbg {location}")))
}

#[test]
fn mir_preserves_statement_sites_for_debuggable_operations() {
    let source = "def twice(value: i64) -> i64 { value * 2 }\n\
def probe(value: i64) -> i64 {\n\
    let mut total = value;\n\
    total = twice(total);\n\
    if total > 10 {\n\
        return total;\n\
    };\n\
    return 0;\n\
}\n\
def main() -> i64 { probe(6) }\n";
    let functions = compile_to_mir(source).expect("debug span fixture should lower to MIR");
    let probe = functions
        .iter()
        .find(|function| function.name == "probe")
        .expect("probe MIR function");

    let mut call_lines = Vec::new();
    let mut assignment_lines = Vec::new();
    let mut branch_lines = Vec::new();
    let mut return_lines = Vec::new();

    for block in &probe.basic_blocks {
        for inst_id in &block.instructions {
            match probe.instruction(*inst_id) {
                Instruction::Call { .. } => {
                    let site = probe.instruction_source_sites[inst_id.0 as usize]
                        .expect("call should inherit a statement site");
                    call_lines.push(line_for_site(source, site));
                }
                Instruction::Store { .. } => {
                    let site = probe.instruction_source_sites[inst_id.0 as usize]
                        .expect("assignment should inherit a statement site");
                    assignment_lines.push(line_for_site(source, site));
                }
                _ => {}
            }
        }
        if let Some(terminator) = &block.terminator {
            match terminator {
                Terminator::If { .. } => {
                    let site = block
                        .terminator_source_site
                        .expect("branch should inherit a statement site");
                    branch_lines.push(line_for_site(source, site));
                }
                Terminator::Return(_) => {
                    let site = block
                        .terminator_source_site
                        .expect("return should inherit a statement site");
                    return_lines.push(line_for_site(source, site));
                }
                _ => {}
            }
        }
    }

    assert!(call_lines.contains(&4), "call sites: {call_lines:?}");
    assert!(
        assignment_lines.contains(&4),
        "assignment sites: {assignment_lines:?}"
    );
    assert!(branch_lines.contains(&5), "branch sites: {branch_lines:?}");
    assert!(return_lines.contains(&6), "return sites: {return_lines:?}");
    assert!(return_lines.contains(&8), "return sites: {return_lines:?}");
}

#[test]
fn llvm_debug_locations_follow_mir_statement_sites() {
    let source = "def twice(value: i64) -> i64 { value * 2 }\n\
def probe(value: i64) -> i64 {\n\
    let mut total = value;\n\
    total = twice(total);\n\
    if total > 10 {\n\
        return total;\n\
    };\n\
    return 0;\n\
}\n\
def main() -> i64 { probe(6) }\n";
    let functions = compile_to_mir(source).expect("debug span fixture should lower to MIR");
    let mut codegen = Codegen::with_ffi_target_and_debug(
        FfiCodegenConfig::default(),
        None,
        DebugInfoConfig::for_source("debug_span.sg", source),
    );
    let ir = codegen
        .codegen(&functions)
        .expect("debug span fixture should codegen");

    let call = debug_location_id(&ir, 4);
    let branch = debug_location_id(&ir, 5);
    let first_return = debug_location_id(&ir, 6);
    let second_return = debug_location_id(&ir, 8);

    assert!(has_instruction_location(&ir, "call i64 @twice", &call));
    assert!(has_instruction_location(&ir, "store i64", &call));
    assert!(has_instruction_location(&ir, "br i1", &branch));
    assert!(has_instruction_location(&ir, "ret i64", &first_return));
    assert!(has_instruction_location(&ir, "ret i64", &second_return));
}

#[test]
fn loop_back_edges_inherit_a_source_statement_site() {
    let source = "def main() -> i64 {\n\
    let mut value = 0;\n\
    while value < 2 {\n\
        value = value + 1;\n\
    };\n\
    value\n\
}\n";
    let functions = compile_to_mir(source).expect("loop debug span fixture should lower");
    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR function");
    let goto_sites = main
        .basic_blocks
        .iter()
        .filter(|block| matches!(block.terminator, Some(Terminator::Goto(_))))
        .map(|block| {
            block
                .terminator_source_site
                .map(|site| line_for_site(source, site))
        })
        .collect::<Vec<_>>();

    assert!(goto_sites.len() >= 2, "loop goto sites: {goto_sites:?}");
    assert!(
        goto_sites.iter().all(Option::is_some),
        "every loop edge should inherit a source site: {goto_sites:?}"
    );
}

#[test]
fn drop_exit_rewrite_preserves_the_explicit_return_site() {
    let source = "struct Resource {\n\
    handle: i64,\n\
}\n\
impl Drop for Resource {\n\
    def drop(&mut self) {\n\
    }\n\
}\n\
def main() -> i64 {\n\
    let resource = Resource { handle: 1 };\n\
    return 0;\n\
}\n";
    let return_line = line_for_site(source, source.find("return 0").unwrap() as u32);
    let functions = compile_to_mir(source).expect("drop debug span fixture should lower");
    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR function");
    let drop_call_sites = main
        .basic_blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|inst_id| match main.instruction(*inst_id) {
            Instruction::Call { func, .. } if func == "Resource_Drop_drop" => main
                .instruction_source_sites
                .get(inst_id.0 as usize)
                .copied()
                .flatten(),
            _ => None,
        })
        .map(|site| line_for_site(source, site))
        .collect::<Vec<_>>();
    let rewritten_exit_sites = main
        .basic_blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator,
                Some(Terminator::If { .. } | Terminator::Goto(_) | Terminator::Return(_))
            )
        })
        .map(|block| {
            block
                .terminator_source_site
                .map(|site| line_for_site(source, site))
        })
        .collect::<Vec<_>>();

    assert_eq!(drop_call_sites, vec![return_line]);
    assert!(
        rewritten_exit_sites
            .iter()
            .all(|line| *line == Some(return_line)),
        "drop exit CFG sites: {rewritten_exit_sites:?}"
    );
}

#[test]
fn flagged_drop_prologue_is_hidden_from_source_level_stepping() {
    let source = "struct Resource {\n\
    handle: i64,\n\
}\n\
impl Drop for Resource {\n\
    def drop(&mut self) {\n\
    }\n\
}\n\
def main() -> i64 {\n\
    let resource = Resource { handle: 1 };\n\
    if resource.handle > 0 { return 1; };\n\
    return 0;\n\
}\n";
    let functions = compile_to_mir(source).expect("flagged drop fixture should lower");
    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR function");
    let entry = &main.basic_blocks[main.start_block];
    let hidden_entry_instructions = entry
        .instructions
        .iter()
        .take_while(|inst| main.debug_hidden_instructions.contains(inst))
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(hidden_entry_instructions.len(), 2);
    assert!(hidden_entry_instructions.iter().all(|inst| main
        .instruction_source_sites
        .get(inst.0 as usize)
        .is_some_and(Option::is_none)));
}

#[test]
fn postcondition_rewrite_preserves_the_explicit_return_site() {
    let source = "def identity(value: i64) -> i64\n\
ensures result == value\n\
{\n\
    return value;\n\
}\n\
def main() -> i64 { identity(1) }\n";
    let return_line = line_for_site(source, source.find("return value").unwrap() as u32);
    let functions = compile_to_mir_with_options(
        source,
        CompileOptions {
            runtime_contract_checks: true,
            ..CompileOptions::default()
        },
    )
    .expect("contract debug span fixture should lower");
    let identity = functions
        .iter()
        .find(|function| function.name == "identity")
        .expect("identity MIR function");
    let exit_sites = identity
        .basic_blocks
        .iter()
        .filter(|block| block.terminator.is_some())
        .map(|block| {
            block
                .terminator_source_site
                .map(|site| line_for_site(source, site))
        })
        .collect::<Vec<_>>();

    assert!(exit_sites.len() >= 4, "contract CFG sites: {exit_sites:?}");
    assert!(
        exit_sites.iter().all(|line| *line == Some(return_line)),
        "postcondition CFG should preserve the return site: {exit_sites:?}"
    );
}

#[test]
fn async_poll_synthesis_preserves_statement_sites_through_codegen() {
    let source = "async def step1() -> i64 { return 1; }\n\
async def step2() -> i64 { return 2; }\n\
async def main() -> i64 {\n\
    let first = await step1();\n\
    let second = await step2();\n\
    return first + second;\n\
}\n";
    let functions = compile_to_mir(source).expect("async debug span fixture should lower");
    let poll = functions
        .iter()
        .find(|function| function.name == "main__poll")
        .expect("async main poll helper");
    let poll_lines = poll
        .instruction_source_sites
        .iter()
        .copied()
        .flatten()
        .chain(
            poll.basic_blocks
                .iter()
                .filter_map(|block| block.terminator_source_site),
        )
        .map(|site| line_for_site(source, site))
        .collect::<std::collections::BTreeSet<_>>();

    assert!(poll_lines.contains(&4), "async poll sites: {poll_lines:?}");
    assert!(poll_lines.contains(&5), "async poll sites: {poll_lines:?}");
    assert!(poll_lines.contains(&6), "async poll sites: {poll_lines:?}");

    let mut codegen = Codegen::with_ffi_target_and_debug(
        FfiCodegenConfig::default(),
        None,
        DebugInfoConfig::for_source("async_debug_span.sg", source),
    );
    let ir = codegen
        .codegen(&functions)
        .expect("async debug span fixture should codegen");
    let poll_start = ir.find("@main__poll(").expect("poll function IR");
    let poll_end = ir[poll_start..]
        .find("\n}\n")
        .map(|offset| poll_start + offset)
        .expect("poll function end");
    let poll_ir = &ir[poll_start..poll_end];
    for source_line in [4, 5, 6] {
        let locations = debug_location_ids(&ir, source_line);
        assert!(
            locations
                .iter()
                .any(|location| poll_ir.contains(&format!("!dbg {location}"))),
            "async poll IR should reference line {source_line} ({locations:?}):\n{poll_ir}"
        );
    }
}

#[test]
fn coverage_registration_is_hidden_from_source_level_debug_stepping() {
    let source = "def main() -> i64 {\n    let value = 41;\n    value + 1\n}\n";
    let program = Parser::parse(source).expect("coverage debug fixture should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("coverage debug fixture should typecheck");
    let type_env = checker.into_env();
    let hir = lower_ast_with_coverage(&program, &type_env);
    let coverage = CoverageContext::for_source(source, 0, source.len() as u32);
    let functions = lower_hir_with_options(
        &hir.items,
        MirLowerOptions::default().with_coverage_context(coverage),
    )
    .expect("coverage debug fixture should lower");
    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR function");
    assert_eq!(main.debug_hidden_instructions.len(), 4);

    let mut codegen = Codegen::with_ffi_target_and_debug(
        FfiCodegenConfig::default(),
        None,
        DebugInfoConfig::for_source("coverage_debug_span.sg", source),
    );
    let ir = codegen
        .codegen(&functions)
        .expect("coverage debug fixture should codegen");
    let registration_lines = ir
        .lines()
        .filter(|line| line.contains("call void @sengoo_coverage_register"))
        .collect::<Vec<_>>();
    assert_eq!(registration_lines.len(), 2);
    assert!(
        registration_lines
            .iter()
            .all(|line| !line.contains("!dbg !")),
        "coverage registration should be hidden from stepping:\n{ir}"
    );
    assert!(
        ir.lines()
            .filter(|line| line.contains("call void @sengoo_coverage_hit"))
            .all(|line| line.contains("!dbg !")),
        "user coverage hits should retain statement locations:\n{ir}"
    );
}
