use crate::compile_to_mir;
use crate::mir::async_cfg_helpers::{build_async_cfg_plan, compute_live_in_user_locals};
use crate::mir::{MirFunction, Terminator, MIR_I64};

#[test]
fn async_cfg_helpers_plan_and_liveness_simple_multi_await_body() {
    let source = r#"
async def step1() -> i64 { 1 }
async def step2(x: i64) -> i64 { x + 1 }

async def main() -> i64 {
    let first = await step1();
    let second = await step2(first);
    second
}
"#;

    let mir_fns = compile_to_mir(source).expect("source should compile to MIR");
    let body = mir_fns
        .iter()
        .find(|f| f.name == "main__body")
        .expect("async main body should be present");

    let plan = build_async_cfg_plan(body).expect("async cfg plan should build");
    let live_in = compute_live_in_user_locals(body, &plan).expect("liveness should compute");

    assert!(
        !live_in.is_empty(),
        "simple async body should produce live-in state"
    );
}

#[test]
fn async_cfg_helpers_accept_unreachable_leaf_blocks() {
    let mut function = MirFunction::new("main".to_string(), vec![], MIR_I64);
    function.is_async = true;
    function.basic_blocks[function.start_block].set_terminator(Terminator::Unreachable);

    let plan = build_async_cfg_plan(&function).expect("unreachable is a terminal CFG edge");
    let live_in =
        compute_live_in_user_locals(&function, &plan).expect("unreachable has no live successors");

    assert_eq!(plan.ordered_blocks, vec![function.start_block]);
    assert!(live_in[&function.start_block].is_empty());
}
