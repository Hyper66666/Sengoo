use sengoo_compiler::{lower_ast, lower_hir, Parser, TypeChecker};

fn main() {
    let source = std::fs::read_to_string("tests/lambda_capture.sg").unwrap();

    let mut parser = Parser::new(&source);
    let program = parser.parse_program().unwrap();

    let mut typeck = TypeChecker::new();
    typeck.check_program(&program).unwrap();

    let type_env = typeck.env();
    let hir_module = lower_ast(&program, type_env);

    let mir_fns = lower_hir(&hir_module.items).unwrap();

    for func in &mir_fns {
        println!("\n=== Function: {} ===", func.name);
        println!("Locals:");
        for (local, ty) in &func.locals {
            println!("  {:?}: {:?}", local, ty);
        }
        println!("\nBasic Blocks:");
        for bb in &func.basic_blocks {
            println!("\n  Block {}:", bb.id);
            for inst in func.block_instructions(bb) {
                println!("    {:?}", inst);
            }
            if let Some(terminator) = &bb.terminator {
                println!("    Terminator: {:?}", terminator);
            }
        }
    }
}
