use crate::mir::{Instruction, Local};
use std::collections::HashMap;

pub(crate) fn infer_async_base_name_from_instructions(
    handle: Local,
    instructions: &[Instruction],
    future_origins: &HashMap<Local, String>,
) -> Option<String> {
    if let Some(name) = future_origins.get(&handle) {
        return Some(name.clone());
    }

    for instruction in instructions.iter().rev() {
        if let Instruction::Load { destination, source } = instruction {
            if *destination == handle {
                if let Some(name) = future_origins.get(source) {
                    return Some(name.clone());
                }
            }
        }
    }

    None
}

pub(crate) fn infer_last_async_start_base(instructions: &[Instruction]) -> Option<String> {
    for instruction in instructions.iter().rev() {
        if let Instruction::Call { func, .. } = instruction {
            if func.ends_with("__start") {
                return Some(func.trim_end_matches("__start").to_string());
            }
        }
    }

    None
}
