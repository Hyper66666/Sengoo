use super::*;
use crate::mir::dyn_dispatch::{parse_shim_name, vtable_global_name};

impl Codegen {
    /// Emit one `[N x i64]` vtable global per `(trait, concrete type)` pair that
    /// has generated dispatch shims. Each slot holds the shim's address as a
    /// pointer-sized integer (`ptrtoint`), matching the integer-word load done at
    /// the dispatch site. The global's LLVM type is recorded so `GlobalRef`s to
    /// it can be bitcast to `i8*` for the fat pointer's vtable field.
    pub(super) fn emit_dyn_vtables(&mut self, mir_fns: &[MirFunction]) {
        // (trait, type_prefix) -> slot -> (shim_name, fn_ptr_type_string)
        let mut tables: HashMap<(String, String), HashMap<usize, (String, String)>> =
            HashMap::new();

        for mir_fn in mir_fns {
            let Some(parsed) = parse_shim_name(&mir_fn.name) else {
                continue;
            };
            let fn_ptr_ty = self.shim_fn_pointer_type(mir_fn);
            tables
                .entry((parsed.trait_name, parsed.type_prefix))
                .or_default()
                .insert(parsed.slot, (mir_fn.name.clone(), fn_ptr_ty));
        }

        if tables.is_empty() {
            return;
        }

        // Deterministic emission order.
        let mut keys: Vec<(String, String)> = tables.keys().cloned().collect();
        keys.sort();

        self.ir.push_str("\n; dyn Trait vtables\n");
        for (trait_name, type_prefix) in keys {
            let slots = &tables[&(trait_name.clone(), type_prefix.clone())];
            let slot_count = slots.keys().copied().max().map(|m| m + 1).unwrap_or(0);

            let mut elements: Vec<String> = Vec::with_capacity(slot_count);
            for slot in 0..slot_count {
                match slots.get(&slot) {
                    Some((shim_name, fn_ptr_ty)) => elements.push(format!(
                        "i64 ptrtoint ({} @{} to i64)",
                        fn_ptr_ty, shim_name
                    )),
                    None => elements.push("i64 0".to_string()),
                }
            }

            let global_name = vtable_global_name(&trait_name, &type_prefix);
            let array_ty = format!("[{} x i64]", slot_count);
            self.ir.push_str(&format!(
                "@{} = internal constant {} [{}]\n",
                global_name,
                array_ty,
                elements.join(", ")
            ));
            self.global_types.insert(global_name, array_ty);
        }
    }

    /// LLVM function-pointer type of a dispatch shim, e.g. `i64 (i8*)*`.
    fn shim_fn_pointer_type(&mut self, shim: &MirFunction) -> String {
        let ret = self.mir_type_to_llvm_cached(&shim.return_type);
        let params: Vec<String> = shim
            .params
            .iter()
            .map(|ty| self.mir_type_to_llvm_cached(ty))
            .collect();
        format!("{} ({})*", ret, params.join(", "))
    }

    /// LLVM type string of a previously-emitted module global, if known.
    pub(super) fn global_llvm_type(&self, name: &str) -> Option<&str> {
        self.global_types.get(name).map(String::as_str)
    }
}
