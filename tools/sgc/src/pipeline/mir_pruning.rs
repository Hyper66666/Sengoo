use sengoo_compiler::mir::{
    Instruction as MirInstruction, MirFunction, Terminator as MirTerminator,
};
use std::collections::{HashMap, HashSet};

pub(super) fn prune_unreachable_mir_functions(mir_fns: &mut Vec<MirFunction>) -> usize {
    if mir_fns.len() <= 1 {
        return 0;
    }

    let mut index_by_name = HashMap::new();
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        index_by_name.insert(mir_fn.name.clone(), idx);
    }

    let Some(&main_index) = index_by_name.get("main") else {
        return 0;
    };

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); mir_fns.len()];
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        edges[idx] = collect_mir_call_targets(mir_fn, &index_by_name);
    }

    let mut reachable = vec![false; mir_fns.len()];
    let mut stack = vec![main_index];
    for root_async_helper in [
        "main__async_body",
        "main__start",
        "main__poll",
        "main__result",
        "sengoo_async_poll_dispatch",
        "sengoo_async_cancel_dispatch",
        "sengoo_async_drop_dispatch",
        "sengoo_async_result_dispatch_i8",
        "sengoo_async_result_dispatch_i16",
        "sengoo_async_result_dispatch_i32",
        "sengoo_async_result_dispatch_i64",
        "sengoo_async_result_dispatch_bool",
        "sengoo_async_result_dispatch_f32",
        "sengoo_async_result_dispatch_f64",
    ] {
        if let Some(&idx) = index_by_name.get(root_async_helper) {
            stack.push(idx);
        }
    }
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        // Lambdas and dyn-dispatch shims are entered indirectly (function
        // pointers / vtable slots), so keep them as reachability roots.
        if mir_fn.name.starts_with("$__lambda")
            || mir_fn
                .name
                .starts_with(sengoo_compiler::mir::dyn_dispatch::VTABLE_SHIM_PREFIX)
        {
            stack.push(idx);
        }
    }
    while let Some(idx) = stack.pop() {
        if reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        for &target in &edges[idx] {
            if !reachable[target] {
                stack.push(target);
            }
        }
    }

    let before = mir_fns.len();
    let mut old_fns = std::mem::take(mir_fns);
    old_fns.reverse();
    let mut kept = Vec::with_capacity(before);
    while let Some(mir_fn) = old_fns.pop() {
        if let Some(&idx) = index_by_name.get(&mir_fn.name) {
            if reachable[idx] {
                kept.push(mir_fn);
            }
        }
    }
    let removed = before.saturating_sub(kept.len());
    *mir_fns = kept;
    removed
}

fn collect_mir_call_targets(
    mir_fn: &MirFunction,
    index_by_name: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for block in &mir_fn.basic_blocks {
        for inst_id in &block.instructions {
            let inst = mir_fn.instruction(*inst_id);
            if let MirInstruction::Call { func, .. } = inst {
                if let Some(&idx) = index_by_name.get(func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
        }
        match &block.terminator {
            Some(MirTerminator::Call { func, .. }) => {
                if let Some(&idx) = index_by_name.get(func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
            Some(MirTerminator::Suspend { poll_func, .. }) => {
                if let Some(&idx) = index_by_name.get(poll_func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
                let result_func = poll_func.replace("__poll", "__result");
                if let Some(&idx) = index_by_name.get(&result_func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
                let start_func = poll_func.replace("__poll", "__start");
                if let Some(&idx) = index_by_name.get(&start_func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
            _ => {}
        }
    }
    targets
}
