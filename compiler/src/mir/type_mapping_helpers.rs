use crate::hir::{self, HIRType, HIRTypeKind};
use crate::mir::enum_defs::EnumDefMap;
use crate::mir::hir_specialization_helpers::substitute_hir_type;
use crate::mir::MIRType;
use crate::type_naming::mir_type_instance_name;
use std::cell::Cell;
use std::collections::HashMap;

thread_local! {
    static TARGET_POINTER_WIDTH: Cell<u8> = const { Cell::new(usize::BITS as u8) };
}

pub(crate) fn with_target_pointer_width<T>(bits: u8, action: impl FnOnce() -> T) -> T {
    TARGET_POINTER_WIDTH.with(|cell| {
        struct Reset<'a> {
            cell: &'a Cell<u8>,
            previous: u8,
        }

        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.cell.set(self.previous);
            }
        }

        let reset = Reset {
            cell,
            previous: cell.replace(bits),
        };
        let result = action();
        drop(reset);
        result
    })
}

pub(crate) fn integer_bits_for_mir(kind: crate::hir::IntKind) -> u8 {
    match kind {
        crate::hir::IntKind::ISize | crate::hir::IntKind::USize => {
            TARGET_POINTER_WIDTH.with(Cell::get)
        }
        _ => kind.bits() as u8,
    }
}

pub(crate) fn hir_type_to_mir_with_structs_and_enums(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    enum_defs: &EnumDefMap,
    subst: &HashMap<String, MIRType>,
) -> MIRType {
    match &ty.kind {
        HIRTypeKind::Named { name, args } => {
            if args.is_empty() {
                if let Some(replacement) = subst.get(name) {
                    return replacement.clone();
                }
            }

            if args.is_empty() {
                if let Some(enum_def) = enum_defs.get(name) {
                    return enum_def.mir_type();
                }
            }

            if let Some(def) = struct_defs.get(name) {
                let mut nested_subst = subst.clone();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    nested_subst.insert(
                        type_param.name.clone(),
                        hir_type_to_mir_with_structs_and_enums(arg, struct_defs, enum_defs, subst),
                    );
                }
                let instance_name = if args.is_empty() {
                    name.clone()
                } else {
                    let parts: Vec<String> = args
                        .iter()
                        .map(|arg| {
                            mir_type_instance_name(&hir_type_to_mir_with_structs_and_enums(
                                arg,
                                struct_defs,
                                enum_defs,
                                subst,
                            ))
                        })
                        .collect();
                    format!("{}_{}", name, parts.join("_"))
                };
                MIRType::Struct {
                    name: instance_name,
                    fields: def
                        .fields
                        .iter()
                        .map(|field| {
                            (
                                field.name.clone(),
                                hir_type_to_mir_with_structs_and_enums(
                                    &field.ty,
                                    struct_defs,
                                    enum_defs,
                                    &nested_subst,
                                ),
                            )
                        })
                        .collect(),
                }
            } else {
                ty.clone().into()
            }
        }
        HIRTypeKind::Str => MIRType::Ptr(Box::new(MIRType::Int(8))),
        HIRTypeKind::Byte => MIRType::UInt(8),
        HIRTypeKind::Ref(_, inner) if matches!(inner.kind, HIRTypeKind::Str) => {
            MIRType::Ptr(Box::new(MIRType::Int(8)))
        }
        HIRTypeKind::Ref(_, inner) => match &inner.kind {
            HIRTypeKind::TraitObject(traits) if !traits.is_empty() => {
                crate::mir::dyn_dispatch::dyn_fat_ptr_type(&traits[0])
            }
            _ => MIRType::Ref(Box::new(hir_type_to_mir_with_structs_and_enums(
                inner,
                struct_defs,
                enum_defs,
                subst,
            ))),
        },
        HIRTypeKind::TraitObject(traits) if !traits.is_empty() => {
            crate::mir::dyn_dispatch::dyn_fat_ptr_type(&traits[0])
        }
        HIRTypeKind::Ptr(inner) => MIRType::Ptr(Box::new(hir_type_to_mir_with_structs_and_enums(
            inner,
            struct_defs,
            enum_defs,
            subst,
        ))),
        HIRTypeKind::Array(elem, len) => MIRType::Array(
            Box::new(hir_type_to_mir_with_structs_and_enums(
                elem,
                struct_defs,
                enum_defs,
                subst,
            )),
            *len as u64,
        ),
        HIRTypeKind::Tuple(types) => MIRType::Tuple(
            types
                .iter()
                .map(|item| {
                    hir_type_to_mir_with_structs_and_enums(item, struct_defs, enum_defs, subst)
                })
                .collect(),
        ),
        HIRTypeKind::Fn { params, ret } => MIRType::Fn {
            params: params
                .iter()
                .map(|item| {
                    hir_type_to_mir_with_structs_and_enums(item, struct_defs, enum_defs, subst)
                })
                .collect(),
            ret: Box::new(hir_type_to_mir_with_structs_and_enums(
                ret,
                struct_defs,
                enum_defs,
                subst,
            )),
        },
        HIRTypeKind::Int(ik) if ik.is_signed() => MIRType::Int(integer_bits_for_mir(*ik)),
        HIRTypeKind::Int(ik) => MIRType::UInt(integer_bits_for_mir(*ik)),
        _ => ty.clone().into(),
    }
}

pub(crate) fn hir_type_to_mir_with_structs_and_subst(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &HashMap<String, MIRType>,
) -> MIRType {
    hir_type_to_mir_with_structs_and_enums(ty, struct_defs, &EnumDefMap::new(), subst)
}

pub(crate) fn bind_mir_subst_from_hir_type(
    template: &HIRType,
    actual: &MIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &mut HashMap<String, MIRType>,
) {
    match &template.kind {
        HIRTypeKind::AssocProjection {
            base,
            trait_name,
            name,
        } => {
            if let HIRTypeKind::Named {
                name: base_name,
                args,
            } = &base.kind
            {
                if args.is_empty() {
                    subst
                        .entry(format!("<{base_name} as {trait_name}>::{name}"))
                        .or_insert_with(|| actual.clone());
                }
            }
        }
        HIRTypeKind::Named { name, args } if args.is_empty() && !struct_defs.contains_key(name) => {
            match subst.get(name) {
                Some(existing) if existing != actual => {}
                Some(_) => {}
                None => {
                    subst.insert(name.clone(), actual.clone());
                }
            }
        }
        HIRTypeKind::Named { name, args } => {
            if let (Some(def), MIRType::Struct { fields, .. }) = (struct_defs.get(name), actual) {
                let mut field_subst = HashMap::new();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    field_subst.insert(type_param.name.clone(), arg.clone());
                }
                for field in &def.fields {
                    if let Some((_, actual_field_ty)) = fields
                        .iter()
                        .find(|(field_name, _)| field_name == &field.name)
                    {
                        let template_field_ty = substitute_hir_type(&field.ty, &field_subst);
                        bind_mir_subst_from_hir_type(
                            &template_field_ty,
                            actual_field_ty,
                            struct_defs,
                            subst,
                        );
                    }
                }
            }
        }
        HIRTypeKind::Ref(_, inner) => {
            if let MIRType::Ref(actual_inner) | MIRType::Ptr(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Ptr(inner) => {
            if let MIRType::Ptr(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Array(inner, _) => {
            if let MIRType::Array(actual_inner, _) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Tuple(items) => {
            if let MIRType::Tuple(actual_items) = actual {
                for (template_item, actual_item) in items.iter().zip(actual_items.iter()) {
                    bind_mir_subst_from_hir_type(template_item, actual_item, struct_defs, subst);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associated_projection_binds_concrete_actual_type() {
        let projection = HIRType::new(HIRTypeKind::AssocProjection {
            base: Box::new(HIRType::named("I".to_string(), Vec::new())),
            trait_name: "Iterator".to_string(),
            name: "Item".to_string(),
        });
        let mut subst = HashMap::new();

        bind_mir_subst_from_hir_type(&projection, &MIRType::Bool, &HashMap::new(), &mut subst);

        assert_eq!(subst.get("<I as Iterator>::Item"), Some(&MIRType::Bool));
    }

    #[test]
    fn same_named_associated_types_from_distinct_traits_do_not_collide() {
        let projection = |trait_name: &str| {
            HIRType::new(HIRTypeKind::AssocProjection {
                base: Box::new(HIRType::named("I".to_string(), Vec::new())),
                trait_name: trait_name.to_string(),
                name: "Item".to_string(),
            })
        };
        let mut subst = HashMap::new();

        bind_mir_subst_from_hir_type(
            &projection("Iterator"),
            &MIRType::Bool,
            &HashMap::new(),
            &mut subst,
        );
        bind_mir_subst_from_hir_type(
            &projection("Stream"),
            &MIRType::Int(32),
            &HashMap::new(),
            &mut subst,
        );

        assert_eq!(subst.get("<I as Iterator>::Item"), Some(&MIRType::Bool));
        assert_eq!(subst.get("<I as Stream>::Item"), Some(&MIRType::Int(32)));
    }
}
