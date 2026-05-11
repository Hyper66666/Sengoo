use crate::compile_to_mir;
use crate::mir::async_cfg_helpers::{
    build_async_cfg_plan, collect_spill_user_locals, collect_user_locals,
    compute_live_in_user_locals,
};
use crate::mir::async_frame_helpers::build_async_frame_layout;
use crate::mir::async_poll_helpers::synthesize_cfg_poll;

#[test]
fn async_poll_helpers_synthesize_cfg_poll_for_simple_multi_await_body() {
    let src = r#"
async def step1() -> i64 { return 1; }
async def step2() -> i64 { return 2; }
async def main() -> i64 {
    let a = await step1();
    let b = await step2();
    return a + b;
}
"#;

    let mir = compile_to_mir(src).expect("compile_to_mir should succeed");
    let mir_fn = mir
        .iter()
        .find(|f| f.name == "main__body")
        .expect("async main body should be preserved");

    let user_locals = collect_user_locals(mir_fn);
    let plan = build_async_cfg_plan(mir_fn).expect("cfg plan should build");
    let live_in = compute_live_in_user_locals(mir_fn, &plan).expect("liveness should compute");
    let spill_user_locals = collect_spill_user_locals(&plan, &user_locals, &live_in);
    let layout = build_async_frame_layout(
        mir_fn.name.clone(),
        mir_fn.params.clone(),
        mir_fn.return_type.clone(),
        crate::mir::async_entry_helpers::count_await_points(mir_fn),
        &spill_user_locals,
    )
    .expect("frame layout should build");

    let poll = synthesize_cfg_poll(&layout, mir_fn, &plan, &spill_user_locals)
        .expect("cfg poll synthesis should succeed");

    assert_eq!(poll.name, "main__body__poll");
    assert!(poll.basic_blocks.len() > mir_fn.basic_blocks.len());
}
