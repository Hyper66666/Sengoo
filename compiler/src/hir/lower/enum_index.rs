use crate::ast::{DeclKind, Program};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub type EnumVariantIndex = HashMap<String, HashMap<String, u32>>;

thread_local! {
    static ACTIVE_ENUM_INDEX: RefCell<Option<Rc<EnumVariantIndex>>> = const { RefCell::new(None) };
}

pub fn build_enum_variant_index(program: &Program) -> EnumVariantIndex {
    let mut index = EnumVariantIndex::new();
    for decl in &program.decls {
        let DeclKind::Enum(enum_decl) = &decl.kind else {
            continue;
        };
        let enum_name = enum_decl.name.name.clone();
        let variants = enum_decl
            .variants
            .iter()
            .enumerate()
            .map(|(i, variant)| (variant.name.name.clone(), i as u32))
            .collect();
        index.insert(enum_name, variants);
    }
    index
}

pub fn with_enum_index<R>(index: EnumVariantIndex, f: impl FnOnce() -> R) -> R {
    ACTIVE_ENUM_INDEX.with(|cell| {
        *cell.borrow_mut() = Some(Rc::new(index));
        let result = f();
        *cell.borrow_mut() = None;
        result
    })
}

pub fn variant_discriminant(enum_name: &str, variant_name: &str) -> Option<u32> {
    ACTIVE_ENUM_INDEX.with(|cell| {
        cell.borrow().as_ref().and_then(|index| {
            index
                .get(enum_name)
                .and_then(|variants| variants.get(variant_name).copied())
        })
    })
}
