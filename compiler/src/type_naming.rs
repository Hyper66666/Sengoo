use crate::hir::{HIRType, HIRTypeKind};
use crate::mir::MIRType;

pub(crate) fn hir_type_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => ik.to_string(),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Named { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}

pub(crate) fn hir_type_instance_name(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => ik.to_string(),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Str => "str".to_string(),
        HIRTypeKind::Ref(_, inner) => format!("ref_{}", hir_type_instance_name(inner)),
        HIRTypeKind::Ptr(inner) => format!("ptr_{}", hir_type_instance_name(inner)),
        HIRTypeKind::Array(elem, len) => format!("array_{}_{}", len, hir_type_instance_name(elem)),
        HIRTypeKind::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(hir_type_instance_name).collect();
            format!("tuple_{}", parts.join("_"))
        }
        HIRTypeKind::Named { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                let parts: Vec<String> = args.iter().map(hir_type_instance_name).collect();
                format!("{}_{}", name, parts.join("_"))
            }
        }
        _ => hir_type_prefix(ty),
    }
}

pub(crate) fn mir_type_instance_name(ty: &MIRType) -> String {
    match ty {
        MIRType::Int(bits) => format!("i{}", bits),
        MIRType::UInt(bits) => format!("u{}", bits),
        MIRType::Float(bits) => format!("f{}", bits),
        MIRType::Bool => "bool".to_string(),
        MIRType::Unit => "unit".to_string(),
        MIRType::Ref(inner) => format!("ref_{}", mir_type_instance_name(inner)),
        MIRType::Ptr(inner) => format!("ptr_{}", mir_type_instance_name(inner)),
        MIRType::Array(elem, len) => format!("array_{}_{}", len, mir_type_instance_name(elem)),
        MIRType::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(mir_type_instance_name).collect();
            format!("tuple_{}", parts.join("_"))
        }
        MIRType::Struct { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}
