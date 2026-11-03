use crate::hir::{HIREnum, HIRItem, HIRType, HIRVariant};
use crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums;
use crate::mir::MIRType;
use crate::type_naming::mir_type_instance_name;
use std::collections::HashMap;

/// Payload shape of one enum variant, kept in HIR form so generic enums can be
/// monomorphised per instantiation.
#[derive(Debug, Clone)]
pub enum EnumVariantPayload {
    /// Unit variant, e.g. `None`.
    Unit,
    /// Tuple variant, e.g. `Some(T)`.
    Tuple(Vec<HIRType>),
    /// Struct variant, e.g. `Ok { value: T }`.
    Struct(Vec<(String, HIRType)>),
}

/// MIR metadata for a declared enum.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    /// Declared generic parameter names, in order.
    pub type_params: Vec<String>,
    /// (discriminant, variant name, optional payload type) for the *uninstantiated*
    /// declaration. Generic parameters resolve to `i64` here; use
    /// [`EnumDef::instantiate`] for a concrete instance.
    pub variants: Vec<(u32, String, Option<MIRType>)>,
    /// (discriminant, variant name, HIR payload shape) used for monomorphisation.
    pub hir_variants: Vec<(u32, String, EnumVariantPayload)>,
}

impl EnumDef {
    pub fn is_generic(&self) -> bool {
        !self.type_params.is_empty()
    }

    /// MIR type of the uninstantiated declaration.
    pub fn mir_type(&self) -> MIRType {
        MIRType::Enum {
            name: self.name.clone(),
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

    /// Instance name for this enum applied to `args`, e.g. `Option_i64`.
    pub fn instance_name(&self, args: &[MIRType]) -> String {
        if args.is_empty() {
            return self.name.clone();
        }
        let parts: Vec<String> = args.iter().map(mir_type_instance_name).collect();
        format!("{}_{}", self.name, parts.join("_"))
    }

    /// Build the monomorphised MIR type for this enum applied to `args`.
    pub fn instantiate(
        &self,
        args: &[MIRType],
        struct_defs: &HashMap<String, &crate::hir::HIRStruct>,
        enum_defs: &EnumDefMap,
    ) -> MIRType {
        let mut subst: HashMap<String, MIRType> = HashMap::new();
        for (param, arg) in self.type_params.iter().zip(args.iter()) {
            subst.insert(param.clone(), arg.clone());
        }
        let variants = self
            .hir_variants
            .iter()
            .map(|(discr, variant_name, payload)| {
                (
                    *discr,
                    payload_to_mir(
                        payload,
                        &format!("{}_{}", self.name, variant_name),
                        struct_defs,
                        enum_defs,
                        &subst,
                    ),
                )
            })
            .collect();
        MIRType::Enum {
            name: self.instance_name(args),
            discr_type: Box::new(crate::mir::MIR_I64),
            variants,
        }
    }

    /// Payload type of a named variant in the given instance type.
    pub fn instance_variant_payload(instance: &MIRType, discriminant: u32) -> Option<MIRType> {
        let MIRType::Enum { variants, .. } = instance else {
            return None;
        };
        variants
            .iter()
            .find(|(discr, _)| *discr == discriminant)
            .and_then(|(_, payload)| payload.clone())
    }
}

fn payload_to_mir(
    payload: &EnumVariantPayload,
    payload_struct_name: &str,
    struct_defs: &HashMap<String, &crate::hir::HIRStruct>,
    enum_defs: &EnumDefMap,
    subst: &HashMap<String, MIRType>,
) -> Option<MIRType> {
    match payload {
        EnumVariantPayload::Unit => None,
        EnumVariantPayload::Tuple(types) if types.is_empty() => None,
        EnumVariantPayload::Tuple(types) if types.len() == 1 => Some(
            hir_type_to_mir_with_structs_and_enums(&types[0], struct_defs, enum_defs, subst),
        ),
        EnumVariantPayload::Tuple(types) => Some(MIRType::Tuple(
            types
                .iter()
                .map(|ty| hir_type_to_mir_with_structs_and_enums(ty, struct_defs, enum_defs, subst))
                .collect(),
        )),
        EnumVariantPayload::Struct(fields) => Some(MIRType::Struct {
            name: payload_struct_name.to_string(),
            fields: fields
                .iter()
                .map(|(name, ty)| {
                    (
                        name.clone(),
                        hir_type_to_mir_with_structs_and_enums(ty, struct_defs, enum_defs, subst),
                    )
                })
                .collect(),
        }),
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
    let type_params: Vec<String> = enum_item
        .type_params
        .iter()
        .map(|param| param.name.clone())
        .collect();

    let hir_variants: Vec<(u32, String, EnumVariantPayload)> = enum_item
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            let (name, payload) = match variant {
                HIRVariant::Unit(name) => (name.clone(), EnumVariantPayload::Unit),
                HIRVariant::Tuple(name, types) => {
                    (name.clone(), EnumVariantPayload::Tuple(types.clone()))
                }
                HIRVariant::Struct(name, fields) => (
                    name.clone(),
                    EnumVariantPayload::Struct(
                        fields
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect(),
                    ),
                ),
            };
            (index as u32, name, payload)
        })
        .collect();

    // The uninstantiated layout resolves generic parameters through the empty
    // substitution, which maps them onto the default `i64` word.
    let empty_subst = HashMap::new();
    let variants = hir_variants
        .iter()
        .map(|(discr, name, payload)| {
            let payload = payload_to_mir(
                payload,
                &format!("{}_{}", enum_item.name, name),
                struct_defs,
                &EnumDefMap::new(),
                &empty_subst,
            );
            (*discr, name.clone(), payload)
        })
        .collect();

    EnumDef {
        name: enum_item.name.clone(),
        type_params,
        variants,
        hir_variants,
    }
}
