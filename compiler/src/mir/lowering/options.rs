use crate::hir;
use crate::mir::EnumDefMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// MirLowerOptions用于配置HIR到MIR的降级过程的选项。
#[derive(Debug, Clone)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
    pub async_functions: Rc<RefCell<HashSet<String>>>,
    pub(crate) generic_function_templates: Rc<HashMap<String, hir::HIRFunction>>,
    pub(crate) enum_defs: Rc<EnumDefMap>,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: Rc::new(RefCell::new(HashSet::new())),
            generic_function_templates: Rc::new(HashMap::new()),
            enum_defs: Rc::new(EnumDefMap::new()),
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
        }
    }

    pub fn with_async_functions(mut self, async_functions: HashSet<String>) -> Self {
        self.async_functions = Rc::new(RefCell::new(async_functions));
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
