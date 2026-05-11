use crate::mir::{Instruction, Local};
use std::collections::HashMap;

pub(crate) fn infer_async_base_name_from_instructions<'a, I>(
    handle: Local,
    instructions: I,
    future_origins: &HashMap<Local, String>,
) -> Option<String>
where
    I: IntoIterator<Item = &'a Instruction>,
    I::IntoIter: DoubleEndedIterator,
{
    if let Some(name) = future_origins.get(&handle) {
        return Some(name.clone());
    }

    let mut instructions = instructions.into_iter();
    while let Some(instruction) = instructions.next_back() {
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

pub(crate) fn infer_last_async_start_base<'a, I>(instructions: I) -> Option<String>
where
    I: IntoIterator<Item = &'a Instruction>,
    I::IntoIter: DoubleEndedIterator,
{
    let mut instructions = instructions.into_iter();
    while let Some(instruction) = instructions.next_back() {
        if let Instruction::Call { func, .. } = instruction {
            if func.ends_with("__start") {
                return Some(func.trim_end_matches("__start").to_string());
            }
        }
    }

    None
}
