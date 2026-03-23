use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant, MirFunction, MIR_I64,
    MIR_UNIT,
};
use crate::CompileError;

/// Per-async-function frame layout (logical, not physical - fields stored at offsets in a malloc'd block)
///   offset 0: state (i64)
///   offset 1: result (i64, holds return value once ready)
///   offset 2..2+N: parameter copies
///   offset 2+N..: child future handles + spilled locals
#[derive(Debug, Clone)]
pub struct AsyncFrameLayout {
    pub func_name: String,
    pub param_types: Vec<MIRType>,
    pub param_offsets: Vec<i64>,
    pub result_storage_ty: MIRType,
    pub await_count: usize,
    pub user_local_offsets: Vec<i64>,
    pub await_offset_start: i64,
    pub total_slots: usize,
}

impl AsyncFrameLayout {
    pub fn total_slots(&self) -> usize {
        self.total_slots
    }
}

pub(crate) fn frame_storage_ty(ty: &MIRType) -> MIRType {
    match ty {
        MIRType::Unit => MIR_I64,
        other => other.clone(),
    }
}

pub(crate) fn enum_is_payloadless(ty: &MIRType) -> bool {
    matches!(
        ty,
        MIRType::Enum { variants, .. } if variants.iter().all(|(_, payload)| payload.is_none())
    )
}

pub(crate) fn async_frame_slot_count(ty: &MIRType) -> Result<usize, CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Bool
        | MIRType::Int(8 | 16 | 32 | 64)
        | MIRType::Float(32 | 64)
        | MIRType::Ref(_)
        | MIRType::Ptr(_)
        | MIRType::Future(_) => Ok(1),
        MIRType::Tuple(items) => items.iter().try_fold(0usize, |acc, item| {
            Ok(acc + async_frame_slot_count(item)?)
        }),
        MIRType::Array(elem, len) => {
            let elem_slots = async_frame_slot_count(elem)?;
            Ok(elem_slots.saturating_mul(*len as usize))
        }
        MIRType::Struct { fields, .. } => fields.iter().try_fold(0usize, |acc, (_, field_ty)| {
            Ok(acc + async_frame_slot_count(field_ty)?)
        }),
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => Ok(1),
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        _ => Err(unsupported_async_frame_type(
            &storage_ty,
            "only scalar, pointer-like, tuple/struct/array, and Future values are supported in async frames yet",
        )),
    }
}

pub(crate) fn build_async_frame_layout(
    func_name: String,
    param_types: Vec<MIRType>,
    return_type: MIRType,
    await_count: usize,
    user_locals: &[(Local, MIRType)],
) -> Result<AsyncFrameLayout, CompileError> {
    let result_storage_ty = frame_storage_ty(&return_type);
    let result_slots = async_frame_slot_count(&result_storage_ty)?;

    let mut next_offset = 1 + result_slots as i64;

    let mut param_offsets = Vec::with_capacity(param_types.len());
    for ty in &param_types {
        param_offsets.push(next_offset);
        next_offset += async_frame_slot_count(ty)? as i64;
    }

    let mut user_local_offsets = Vec::with_capacity(user_locals.len());
    for (_, ty) in user_locals {
        user_local_offsets.push(next_offset);
        next_offset += async_frame_slot_count(ty)? as i64;
    }

    let await_offset_start = next_offset;
    let total_slots = (await_offset_start as usize) + await_count;

    Ok(AsyncFrameLayout {
        func_name,
        param_types,
        param_offsets,
        result_storage_ty,
        await_count,
        user_local_offsets,
        await_offset_start,
        total_slots,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AsyncFrameValueKind {
    I64,
    NarrowInt,
    Bool,
    Float32,
    Float64,
    PointerLike,
}

pub(crate) fn describe_async_frame_type(ty: &MIRType) -> String {
    match ty {
        MIRType::Unit => "unit".to_string(),
        MIRType::Never => "never".to_string(),
        MIRType::Bool => "bool".to_string(),
        MIRType::Int(bits) => format!("i{}", bits),
        MIRType::Float(bits) => format!("f{}", bits),
        MIRType::Ref(inner) => format!("&{}", describe_async_frame_type(inner)),
        MIRType::Ptr(inner) => format!("*{}", describe_async_frame_type(inner)),
        MIRType::Array(elem, len) => format!("[{}; {}]", describe_async_frame_type(elem), len),
        MIRType::Tuple(types) => format!(
            "({})",
            types
                .iter()
                .map(describe_async_frame_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        MIRType::Fn { .. } => "fn".to_string(),
        MIRType::Struct { name, .. } => name.clone(),
        MIRType::Enum { .. } => "enum".to_string(),
        MIRType::Future(inner) => format!("Future<{}>", describe_async_frame_type(inner)),
    }
}

pub(crate) fn unsupported_async_frame_type(ty: &MIRType, reason: &str) -> CompileError {
    CompileError::AsyncUnsupportedType {
        ty: describe_async_frame_type(ty),
        reason: reason.to_string(),
    }
}

pub(crate) fn classify_async_frame_type(ty: &MIRType) -> Result<AsyncFrameValueKind, CompileError> {
    match ty {
        MIRType::Bool => Ok(AsyncFrameValueKind::Bool),
        MIRType::Int(8 | 16 | 32) => Ok(AsyncFrameValueKind::NarrowInt),
        MIRType::Int(64) | MIRType::Future(_) => Ok(AsyncFrameValueKind::I64),
        MIRType::Float(32) => Ok(AsyncFrameValueKind::Float32),
        MIRType::Float(64) => Ok(AsyncFrameValueKind::Float64),
        MIRType::Ref(_) | MIRType::Ptr(_) => Ok(AsyncFrameValueKind::PointerLike),
        MIRType::Tuple(_) | MIRType::Struct { .. } | MIRType::Array(_, _) | MIRType::Enum { .. } => {
            Err(unsupported_async_frame_type(
                ty,
                "aggregate types (tuple/struct/array/enum) cannot cross await points yet",
            ))
        }
        _ => Err(unsupported_async_frame_type(
            ty,
            "only bool, i8/i16/i32/i64, f32/f64, ref/ptr, and Future handles are supported in async frames yet",
        )),
    }
}

pub(crate) fn frame_user_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    layout.user_local_offsets[index]
}

pub(crate) fn frame_await_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    layout.await_offset_start + index as i64
}

pub(crate) fn push_i64_const(f: &mut MirFunction, block: usize, value: i64) -> Local {
    let local = f.add_local(LocalKind::Temp, MIR_I64);
    let inst = f.alloc_inst(Instruction::Assign {
        destination: local,
        value: MirConstant::Int(value),
    });
    f.basic_blocks[block].push(inst);
    local
}

pub(crate) fn push_frame_store(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    value: Local,
) {
    let offset_local = push_i64_const(f, block, offset);
    let dest = f.add_local(LocalKind::Temp, MIR_UNIT);
    let inst = f.alloc_inst(Instruction::Call {
        destination: dest,
        func: "sengoo_async_frame_store".to_string(),
        args: vec![handle, offset_local, value],
    });
    f.basic_blocks[block].push(inst);
}

pub(crate) fn encode_async_frame_value(
    f: &mut MirFunction,
    block: usize,
    value: Local,
    ty: &MIRType,
) -> Result<Local, CompileError> {
    match classify_async_frame_type(ty)? {
        AsyncFrameValueKind::I64 => Ok(value),
        AsyncFrameValueKind::Bool
        | AsyncFrameValueKind::NarrowInt
        | AsyncFrameValueKind::PointerLike => {
            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let cast = f.alloc_inst(Instruction::Cast {
                destination: encoded,
                value,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(cast);
            Ok(encoded)
        }
        AsyncFrameValueKind::Float32 => {
            let bitcast_i32 = f.add_local(LocalKind::Temp, MIRType::Int(32));
            let bitcast = f.alloc_inst(Instruction::Bitcast {
                destination: bitcast_i32,
                value,
                to: MIRType::Int(32),
            });
            f.basic_blocks[block].push(bitcast);

            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let cast = f.alloc_inst(Instruction::Cast {
                destination: encoded,
                value: bitcast_i32,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(cast);
            Ok(encoded)
        }
        AsyncFrameValueKind::Float64 => {
            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let bitcast = f.alloc_inst(Instruction::Bitcast {
                destination: encoded,
                value,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(bitcast);
            Ok(encoded)
        }
    }
}

fn push_extract_value(
    f: &mut MirFunction,
    block: usize,
    value: Local,
    index: usize,
    field_ty: MIRType,
) -> Local {
    let extracted = f.add_local(LocalKind::Temp, field_ty);
    let inst = f.alloc_inst(Instruction::Extract {
        destination: extracted,
        value,
        index: index as u32,
    });
    f.basic_blocks[block].push(inst);
    extracted
}

fn push_aggregate_value(
    f: &mut MirFunction,
    block: usize,
    ty: MIRType,
    fields: Vec<Local>,
) -> Local {
    let aggregate = f.add_local(LocalKind::Temp, ty.clone());
    let inst = f.alloc_inst(Instruction::Aggregate {
        destination: aggregate,
        fields,
        ty,
    });
    f.basic_blocks[block].push(inst);
    aggregate
}

pub(crate) fn push_frame_store_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    value: Local,
    ty: &MIRType,
) -> Result<(), CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => {
            let discr = f.add_local(LocalKind::Temp, MIR_I64);
            let inst = f.alloc_inst(Instruction::Discriminant {
                destination: discr,
                source: value,
            });
            f.basic_blocks[block].push(inst);
            push_frame_store(f, block, handle, offset, discr);
            Ok(())
        }
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        MIRType::Tuple(items) => {
            let mut next_offset = offset;
            for (index, item_ty) in items.iter().enumerate() {
                let extracted = push_extract_value(f, block, value, index, item_ty.clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, item_ty)?;
                next_offset += async_frame_slot_count(item_ty)? as i64;
            }
            Ok(())
        }
        MIRType::Array(elem, len) => {
            let mut next_offset = offset;
            for index in 0..(*len as usize) {
                let extracted = push_extract_value(f, block, value, index, (**elem).clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, elem)?;
                next_offset += async_frame_slot_count(elem)? as i64;
            }
            Ok(())
        }
        MIRType::Struct { fields, .. } => {
            let mut next_offset = offset;
            for (index, (_, field_ty)) in fields.iter().enumerate() {
                let extracted = push_extract_value(f, block, value, index, field_ty.clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, field_ty)?;
                next_offset += async_frame_slot_count(field_ty)? as i64;
            }
            Ok(())
        }
        _ => {
            let encoded = encode_async_frame_value(f, block, value, &storage_ty)?;
            push_frame_store(f, block, handle, offset, encoded);
            Ok(())
        }
    }
}

pub(crate) fn push_frame_load_into(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    destination: Local,
) {
    let offset_local = push_i64_const(f, block, offset);
    let inst = f.alloc_inst(Instruction::Call {
        destination,
        func: "sengoo_async_frame_load".to_string(),
        args: vec![handle, offset_local],
    });
    f.basic_blocks[block].push(inst);
}

pub(crate) fn push_frame_load_into_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    destination: Local,
    ty: &MIRType,
) -> Result<(), CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Tuple(_) | MIRType::Array(_, _) | MIRType::Struct { .. } | MIRType::Enum { .. } => {
            let loaded = push_frame_load_typed(f, block, handle, offset, storage_ty.clone())?;
            let store = f.alloc_inst(Instruction::Store {
                destination,
                value: loaded,
            });
            f.basic_blocks[block].push(store);
        }
        _ => match classify_async_frame_type(&storage_ty)? {
            AsyncFrameValueKind::I64 => push_frame_load_into(f, block, handle, offset, destination),
            AsyncFrameValueKind::Bool
            | AsyncFrameValueKind::NarrowInt
            | AsyncFrameValueKind::PointerLike => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let cast = f.alloc_inst(Instruction::Cast {
                    destination,
                    value: encoded,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(cast);
            }
            AsyncFrameValueKind::Float32 => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let narrowed = f.add_local(LocalKind::Temp, MIRType::Int(32));
                let cast = f.alloc_inst(Instruction::Cast {
                    destination: narrowed,
                    value: encoded,
                    to: MIRType::Int(32),
                });
                f.basic_blocks[block].push(cast);
                let bitcast = f.alloc_inst(Instruction::Bitcast {
                    destination,
                    value: narrowed,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(bitcast);
            }
            AsyncFrameValueKind::Float64 => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let bitcast = f.alloc_inst(Instruction::Bitcast {
                    destination,
                    value: encoded,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(bitcast);
            }
        },
    }
    Ok(())
}

pub(crate) fn push_frame_load(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    ty: MIRType,
) -> Local {
    let destination = f.add_local(LocalKind::Temp, ty);
    push_frame_load_into(f, block, handle, offset, destination);
    destination
}

pub(crate) fn push_frame_load_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    ty: MIRType,
) -> Result<Local, CompileError> {
    let storage_ty = frame_storage_ty(&ty);
    match &storage_ty {
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => {
            let discr = push_frame_load(f, block, handle, offset, MIR_I64);
            let zero_payload = push_i64_const(f, block, 0);
            Ok(push_aggregate_value(
                f,
                block,
                storage_ty,
                vec![discr, zero_payload],
            ))
        }
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        MIRType::Tuple(items) => {
            let mut fields = Vec::with_capacity(items.len());
            let mut next_offset = offset;
            for item_ty in items {
                let loaded = push_frame_load_typed(f, block, handle, next_offset, item_ty.clone())?;
                fields.push(loaded);
                next_offset += async_frame_slot_count(item_ty)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, fields))
        }
        MIRType::Array(elem, len) => {
            let mut fields = Vec::with_capacity(*len as usize);
            let mut next_offset = offset;
            for _ in 0..(*len as usize) {
                let loaded = push_frame_load_typed(f, block, handle, next_offset, (**elem).clone())?;
                fields.push(loaded);
                next_offset += async_frame_slot_count(elem)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, fields))
        }
        MIRType::Struct { fields, .. } => {
            let mut values = Vec::with_capacity(fields.len());
            let mut next_offset = offset;
            for (_, field_ty) in fields {
                let loaded = push_frame_load_typed(f, block, handle, next_offset, field_ty.clone())?;
                values.push(loaded);
                next_offset += async_frame_slot_count(field_ty)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, values))
        }
        _ => {
            let destination = f.add_local(LocalKind::Temp, storage_ty.clone());
            push_frame_load_into_typed(f, block, handle, offset, destination, &storage_ty)?;
            Ok(destination)
        }
    }
}
