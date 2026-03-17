use crate::hir::{HIRFunction, HIRParam, HIRStruct, HIRType, HIRTypeParam, IntKind};
use crate::mir::method_specialization_helpers::bind_method_specialization_subst;
use crate::mir::{MIRType, MIR_I64};
use crate::symbol::SymbolId;
use std::collections::HashMap;

fn generic_vec_struct() -> HIRStruct {
    HIRStruct {
        name: "Vec".to_string(),
        type_params: vec![HIRTypeParam {
            name: "T".to_string(),
            bounds: vec![],
            default: None,
        }],
        fields: vec![],
        is_pub: false,
    }
}

fn generic_push_method() -> HIRFunction {
    HIRFunction {
        name: "Vec_push".to_string(),
        type_params: vec![HIRTypeParam {
            name: "T".to_string(),
            bounds: vec![],
            default: None,
        }],
        params: vec![
            HIRParam::new(
                "self".to_string(),
                SymbolId::new(1),
                HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]),
            ),
            HIRParam::new(
                "value".to_string(),
                SymbolId::new(2),
                HIRType::named("T".to_string(), vec![]),
            ),
        ],
        return_type: HIRType::int(IntKind::I64),
        precondition: None,
        postcondition: None,
        body: crate::hir::HIRBody::new(),
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    }
}

#[test]
fn bind_method_specialization_subst_infers_from_args() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };

    let subst = bind_method_specialization_subst(
        &target_type,
        &method,
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
    )
    .expect("specialization should bind T from explicit arg types");

    assert_eq!(subst.get("T"), Some(&MIR_I64));
}

#[test]
fn bind_method_specialization_subst_rejects_wrong_arity() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };

    let subst =
        bind_method_specialization_subst(&target_type, &method, &receiver_ty, &[], &struct_defs);

    assert!(subst.is_none());
}
