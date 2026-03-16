use crate::mir::MIRType;

pub(crate) fn is_void_like(ty: &MIRType) -> bool {
    match ty {
        MIRType::Unit | MIRType::Never => true,
        MIRType::Tuple(fields) if fields.is_empty() => true,
        _ => false,
    }
}
