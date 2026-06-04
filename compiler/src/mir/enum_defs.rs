use crate::hir::{HIREnum, HIRItem, HIRVariant};
use crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums;
use crate::mir::MIRType;
use std::collections::HashMap;

/// MIR metadata for a declared enum.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    /// (discriminant, variant name, optional payload type)
    pub variants: Vec<(u32, String, Option<MIRType>)>,
}

impl EnumDef {
    pub fn mir_type(&self) -> MIRType {
        MIRType::Enum {
            discr_type: Box::new(crate::mir::MIR_I64),
            variants: self
                .variants
                .iter()
                .map(|(discr, _, payload)| (*discr, payload.clone()))
                .collect(),
        }
    }

    pub fn variant_discriminant(&self, variant_name: &str) -> Option<u32> {
        self.variants
            .iter()
            .find(|(_, name, _)| name == variant_name)
            .map(|(discr, _, _)| *discr)
    }
}

pub type EnumDefMap = HashMap<String, EnumDef>;

pub fn build_enum_defs(
    items: &[HIRItem],
    struct_defs: &HashMap<String, &crate::hir::HIRStruct>,
) -> EnumDefMap {
    let mut defs = EnumDefMap::new();
    for item in items {
        let HIRItem::Enum(enum_item) = item else {
            continue;
        };
        defs.insert(
            enum_item.name.clone(),
            enum_def_from_hir(enum_item, struct_defs),
        );
    }
    defs
}

fn enum_def_from_hir(
    enum_item: &HIREnum,
    struct_defs: &HashMap<String, &crate::hir::HIRStruct>,
) -> EnumDef {
    let variants = enum_item
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let (name, payload) = match variant {
                HIRVariant::Unit(name) => (name.clone(), None),
                HIRVariant::Tuple(name, types) => {
                    let payload = if types.len() == 1 {
                        Some(hir_type_to_mir_with_structs_and_enums(
                            &types[0],
                            struct_defs,
                            &HashMap::new(),
                            &HashMap::new(),
                        ))
                    } else if types.is_empty() {
                        None
                    } else {
                        Some(MIRType::Tuple(
                            types
                                .iter()
                                .map(|t| {
                                    hir_type_to_mir_with_structs_and_enums(
                                        t,
                                        struct_defs,
                                        &HashMap::new(),
                                        &HashMap::new(),
                                    )
                                })
                                .collect(),
                        ))
                    };
                    (name.clone(), payload)
                }
                HIRVariant::Struct(name, fields) => {
                    let payload = MIRType::Struct {
                        name: format!("{}_{}", enum_item.name, name),
                        fields: fields
                            .iter()
                            .map(|f| {
                                (
                                    f.name.clone(),
                                    hir_type_to_mir_with_structs_and_enums(
                                        &f.ty,
                                        struct_defs,
                                        &HashMap::new(),
                                        &HashMap::new(),
                                    ),
                                )
                            })
                            .collect(),
                    };
                    (name.clone(), Some(payload))
                }
            };
            (index as u32, name, payload)
        })
        .collect();
    EnumDef {
        name: enum_item.name.clone(),
        variants,
    }
}
