use crate::hir::{self, HIRType};
use crate::mir::enum_defs::EnumDefMap;
use crate::mir::lowering::FunctionSig;
use crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums;
use crate::mir::MIRType;
use std::collections::HashMap;

pub(crate) fn build_function_sig(
    ret_type: MIRType,
    param_count: usize,
    env: Vec<(String, MIRType)>,
) -> FunctionSig {
    FunctionSig {
        ret_type,
        param_count,
        receiver_is_ref: false,
        env,
    }
}

pub(crate) fn build_hir_function_sig_with_enums(
    return_type: &HIRType,
    param_count: usize,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    enum_defs: &EnumDefMap,
) -> FunctionSig {
    build_function_sig(
        hir_type_to_mir_with_structs_and_enums(
            return_type,
            struct_defs,
            enum_defs,
            &HashMap::new(),
        ),
        param_count,
        vec![],
    )
}

pub(crate) fn build_hir_method_sig_with_enums(
    method: &hir::HIRFunction,
    param_count: usize,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    enum_defs: &EnumDefMap,
) -> FunctionSig {
    let mut sig = build_hir_function_sig_with_enums(
        &method.return_type,
        param_count,
        struct_defs,
        enum_defs,
    );
    sig.receiver_is_ref = method.params.first().is_some_and(|param| {
        param.name == "self" && matches!(param.ty.kind, hir::HIRTypeKind::Ref(..))
    });
    sig
}
