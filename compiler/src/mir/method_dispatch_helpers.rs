use crate::mir::MIRType;

pub(crate) fn receiver_type_prefix(receiver_ty: &MIRType) -> String {
    match receiver_ty {
        MIRType::Int(bits) => format!("i{}", bits),
        MIRType::Float(bits) => format!("f{}", bits),
        MIRType::Bool => "bool".to_string(),
        MIRType::Array(_, _) => "array".to_string(),
        MIRType::Tuple(_) => "tuple".to_string(),
        MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
            MIRType::Int(bits) => format!("i{}_ptr", bits),
            MIRType::Float(bits) => format!("f{}_ptr", bits),
            MIRType::Bool => "bool_ptr".to_string(),
            _ => "ptr".to_string(),
        },
        MIRType::Struct { name, .. } => name.clone(),
        MIRType::Enum { .. } => "enum".to_string(),
        _ => "i64".to_string(),
    }
}

pub(crate) fn method_dispatch_name(
    explicit_type_name: Option<&str>,
    receiver_ty: &MIRType,
    method: &str,
) -> String {
    if let Some(type_name) = explicit_type_name {
        format!("{}_{}", type_name, method)
    } else {
        match receiver_ty {
            MIRType::Int(bits) => format!("i{}_{}", bits, method),
            MIRType::Float(bits) => format!("f{}_{}", bits, method),
            MIRType::Bool => format!("bool_{}", method),
            MIRType::Array(_, _) => format!("array_{}", method),
            MIRType::Tuple(_) => format!("tuple_{}", method),
            MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                MIRType::Int(bits) => format!("i{}_ptr_{}", bits, method),
                MIRType::Float(bits) => format!("f{}_ptr_{}", bits, method),
                MIRType::Bool => format!("bool_ptr_{}", method),
                _ => format!("ptr_{}", method),
            },
            MIRType::Struct { name, .. } => format!("{}_{}", name, method),
            MIRType::Enum { .. } => format!("enum_{}", method),
            _ => format!("i64_{}", method),
        }
    }
}

pub(crate) fn receiver_type_display(
    explicit_type_name: Option<&str>,
    receiver_ty: &MIRType,
) -> String {
    if let Some(type_name) = explicit_type_name {
        type_name.to_string()
    } else {
        match receiver_ty {
            MIRType::Int(bits) => format!("i{}", bits),
            MIRType::Float(bits) => format!("f{}", bits),
            MIRType::Bool => "bool".to_string(),
            MIRType::Array(_, _) => "array".to_string(),
            MIRType::Tuple(_) => "tuple".to_string(),
            MIRType::Ptr(_) | MIRType::Ref(_) => "ptr".to_string(),
            MIRType::Struct { name, .. } => name.clone(),
            MIRType::Enum { .. } => "enum".to_string(),
            _ => format!("{:?}", receiver_ty),
        }
    }
}
