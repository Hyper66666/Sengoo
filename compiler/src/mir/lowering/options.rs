use crate::hir;
use crate::mir::dyn_dispatch::DynMethodSlot;
use crate::mir::EnumDefMap;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

/// Source metadata used to inject optional assertion callsite fields.
#[derive(Debug, Clone, Default)]
pub struct AssertCallsiteContext {
    pub source_file: Option<Rc<str>>,
    pub source_text: Option<Rc<str>>,
    pub user_base_offset: u32,
}

/// Source interval and shared probe inventory for coverage-only MIR lowering.
/// Normal compilation leaves this disabled and emits no coverage calls.
#[derive(Debug, Clone, Default)]
pub struct CoverageContext {
    pub source_text: Option<Rc<str>>,
    pub user_base_offset: u32,
    pub user_source_len: u32,
    pub(crate) executable_lines: Rc<RefCell<BTreeSet<u32>>>,
}

impl CoverageContext {
    pub fn for_source(
        source_text: impl Into<Rc<str>>,
        user_base_offset: u32,
        user_source_len: u32,
    ) -> Self {
        Self {
            source_text: Some(source_text.into()),
            user_base_offset,
            user_source_len,
            executable_lines: Rc::new(RefCell::new(BTreeSet::new())),
        }
    }

    pub(crate) fn line_for_site(&self, site_lo: u32) -> Option<u32> {
        let source = self.source_text.as_deref()?;
        let start = self.user_base_offset as usize;
        let end = site_lo as usize;
        let limit = start.checked_add(self.user_source_len as usize)?;
        if end < start || end >= limit || limit > source.len() {
            return None;
        }
        Some(
            source[start..end]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count() as u32
                + 1,
        )
    }
}

/// MirLowerOptions用于配置HIR到MIR的降级过程的选项。
#[derive(Debug, Clone)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
    pub async_functions: Rc<RefCell<HashSet<String>>>,
    pub(crate) generic_function_templates: Rc<HashMap<String, hir::HIRFunction>>,
    pub(crate) enum_defs: Rc<EnumDefMap>,
    pub(crate) assert_callsite: Rc<AssertCallsiteContext>,
    pub(crate) coverage: Option<CoverageContext>,
    /// Per-trait ordered (sorted) object-safe method names. Defines the vtable
    /// slot layout for `dyn Trait` dynamic dispatch; shared by both the dispatch
    /// site and shim/vtable synthesis.
    pub(crate) trait_method_order: Rc<HashMap<String, Vec<DynMethodSlot>>>,
    /// Per-function expected `dyn Trait` parameter traits, indexed by explicit
    /// parameter position (`Some(trait)` when that parameter is `&dyn Trait`).
    pub(crate) dyn_param_traits: Rc<HashMap<String, Vec<Option<String>>>>,
    /// `(trait, concrete_type_prefix)` pairs discovered at `&Concrete -> &dyn Trait`
    /// coercion sites; consumed after lowering to synthesize vtable shims.
    pub(crate) dyn_vtable_requests: Rc<RefCell<HashSet<(String, String)>>>,
    /// Trait names whose owned `dyn Trait` values need a scope-exit drop helper
    /// (`__dyn_Trait_Drop_drop`) that dispatches through the vtable drop slot.
    pub(crate) dyn_owned_drop_requests: Rc<RefCell<HashSet<String>>>,
    pub(crate) target_pointer_width: u8,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: Rc::new(RefCell::new(HashSet::new())),
            generic_function_templates: Rc::new(HashMap::new()),
            enum_defs: Rc::new(EnumDefMap::new()),
            assert_callsite: Rc::new(AssertCallsiteContext::default()),
            coverage: None,
            trait_method_order: Rc::new(HashMap::new()),
            dyn_param_traits: Rc::new(HashMap::new()),
            dyn_vtable_requests: Rc::new(RefCell::new(HashSet::new())),
            dyn_owned_drop_requests: Rc::new(RefCell::new(HashSet::new())),
            target_pointer_width: usize::BITS as u8,
        }
    }
}

impl MirLowerOptions {
    pub(crate) fn with_enum_defs(mut self, enum_defs: EnumDefMap) -> Self {
        self.enum_defs = Rc::new(enum_defs);
        self
    }
}

impl MirLowerOptions {
    pub fn new(
        runtime_contract_checks: bool,
        lazy_generic_mono: bool,
        async_functions: HashSet<String>,
    ) -> Self {
        Self {
            runtime_contract_checks,
            lazy_generic_mono,
            async_functions: Rc::new(RefCell::new(async_functions)),
            generic_function_templates: Rc::new(HashMap::new()),
            enum_defs: Rc::new(EnumDefMap::new()),
            assert_callsite: Rc::new(AssertCallsiteContext::default()),
            coverage: None,
            trait_method_order: Rc::new(HashMap::new()),
            dyn_param_traits: Rc::new(HashMap::new()),
            dyn_vtable_requests: Rc::new(RefCell::new(HashSet::new())),
            dyn_owned_drop_requests: Rc::new(RefCell::new(HashSet::new())),
            target_pointer_width: usize::BITS as u8,
        }
    }

    pub(crate) fn with_dyn_dispatch_metadata(
        mut self,
        trait_method_order: HashMap<String, Vec<DynMethodSlot>>,
        dyn_param_traits: HashMap<String, Vec<Option<String>>>,
    ) -> Self {
        self.trait_method_order = Rc::new(trait_method_order);
        self.dyn_param_traits = Rc::new(dyn_param_traits);
        self
    }

    pub fn with_assert_callsite_context(mut self, context: AssertCallsiteContext) -> Self {
        self.assert_callsite = Rc::new(context);
        self
    }

    pub fn with_coverage_context(mut self, context: CoverageContext) -> Self {
        self.coverage = Some(context);
        self
    }

    pub fn with_async_functions(mut self, async_functions: HashSet<String>) -> Self {
        self.async_functions = Rc::new(RefCell::new(async_functions));
        self
    }

    pub fn with_target_pointer_width(mut self, target_pointer_width: u8) -> Self {
        assert!(
            matches!(target_pointer_width, 32 | 64),
            "target pointer width must be 32 or 64 bits"
        );
        self.target_pointer_width = target_pointer_width;
        self
    }

    pub(crate) fn with_generic_function_templates(
        mut self,
        templates: HashMap<String, hir::HIRFunction>,
    ) -> Self {
        self.generic_function_templates = Rc::new(templates);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mir_lower_options_clone_shares_async_function_set() {
        let options = MirLowerOptions::default();
        let cloned = options.clone();

        options
            .async_functions
            .borrow_mut()
            .insert("outer".to_string());
        cloned
            .async_functions
            .borrow_mut()
            .insert("inner".to_string());

        let async_functions = options.async_functions.borrow();
        assert!(async_functions.contains("outer"));
        assert!(async_functions.contains("inner"));
    }
}
