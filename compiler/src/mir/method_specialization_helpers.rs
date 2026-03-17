use crate::hir::{self, HIRType};
use crate::method_resolution::explicit_hir_method_params;
use crate::mir::type_mapping_helpers::bind_mir_subst_from_hir_type;
use crate::mir::MIRType;
use std::collections::HashMap;

pub(crate) fn bind_method_specialization_subst(
    target_type: &HIRType,
    method: &hir::HIRFunction,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
) -> Option<HashMap<String, MIRType>> {
    let mut mir_subst = HashMap::new();
    bind_mir_subst_from_hir_type(target_type, receiver_ty, struct_defs, &mut mir_subst);

    let explicit_params = explicit_hir_method_params(&method.params);
    if explicit_params.len() != actual_arg_types.len() {
        return None;
    }
    for (param, actual_ty) in explicit_params.iter().zip(actual_arg_types.iter()) {
        bind_mir_subst_from_hir_type(&param.ty, actual_ty, struct_defs, &mut mir_subst);
    }

    Some(mir_subst)
}
