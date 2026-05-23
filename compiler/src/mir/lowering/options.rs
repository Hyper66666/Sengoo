use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// MirLowerOptions用于配置HIR到MIR的降级过程的选项。
#[derive(Debug, Clone)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
    pub async_functions: Rc<RefCell<HashSet<String>>>,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: Rc::new(RefCell::new(HashSet::new())),
        }
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
        }
    }

    pub fn with_async_functions(mut self, async_functions: HashSet<String>) -> Self {
        self.async_functions = Rc::new(RefCell::new(async_functions));
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
