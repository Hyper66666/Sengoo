use crate::compile_to_mir;
use crate::mir::async_cfg_helpers::{build_async_cfg_plan, compute_live_in_user_locals};

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

    assert!(!live_in.is_empty(), "simple async body should produce live-in state");
}
