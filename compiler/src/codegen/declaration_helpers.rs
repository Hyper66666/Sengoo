use super::*;
use crate::mir::async_dispatch_synthesis_helpers::select_runtime_declaration;

impl Codegen {
    /// Declare external runtime functions used by generated LLVM IR.
    pub(super) fn declare_runtime_functions(&mut self) {

        self.declarations

            .push_str("; External C library functions\n");

        self.declarations.push_str("declare i32 @puts(i8*)\n");

        self.declarations

            .push_str("declare i32 @printf(i8*, ...)\n");

        self.declarations.push_str("\n");



        // Sengoo runtime print functions

        self.declarations

            .push_str("; Sengoo runtime print functions\n");

        self.declarations

            .push_str("declare void @sengoo_print_i64(i64)\n");

        self.declarations

            .push_str("declare void @sengoo_print_bool(i64)\n");

        self.declarations

            .push_str("declare void @sengoo_print_f64(double)\n");

        self.declarations

            .push_str("declare void @sengoo_print_str(i8*)\n");

        self.declarations.push_str("\n");



        // Sengoo runtime string functions

        self.declarations

            .push_str("; Sengoo runtime string functions\n");

        self.declarations

            .push_str("declare i64 @sengoo_str_len(i8*)\n");

        self.declarations

            .push_str("declare i8* @sengoo_str_concat(i8*, i8*)\n");

        self.declarations

            .push_str("declare i64 @sengoo_str_eq(i8*, i8*)\n");

        self.declarations.push_str("\n");



        self.declarations
            .push_str("; Sengoo async runtime functions\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_frame_alloc(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_frame_free(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_frame_store(i64, i64, i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_frame_load(i64, i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_run_main_i64(i64)\n");
        self.declarations.push_str("\n");

        self.declare_user_extern_functions();

    }



    fn declare_user_extern_functions(&mut self) {

        if self.ffi.extern_decls.is_empty() {

            return;

        }



        self.declarations

            .push_str("; User-declared extern FFI functions\n");

        let mut seen = HashSet::new();

        let extern_decls = self.ffi.extern_decls.clone();



        for decl in extern_decls {

            if !seen.insert(decl.name.clone()) {

                continue;

            }



            if let Some(link_name) = &decl.link_name {

                self.declarations

                    .push_str(&format!("; link(name = \"{}\")\n", link_name));

            }

            self.declarations

                .push_str(&format!("; ABI: {}\n", decl.abi.as_str()));



            let ret = self.mir_type_to_llvm_cached(&decl.ret);

            let params = decl

                .params

                .iter()

                .map(|p| self.mir_type_to_llvm_cached(p))

                .collect::<Vec<_>>()

                .join(", ");

            self.declarations

                .push_str(&format!("declare {} @{}({})\n", ret, decl.name, params));

        }



        self.declarations.push_str("\n");

    }



    /// Declare the async spawn runtime hook only when the module actually uses it.
    pub(super) fn maybe_declare_spawn_runtime_function(&mut self, mir_fns: &[MirFunction]) {
        let needs_spawn = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_async_spawn_raw",
                _ => false,
            })
        });
        if !needs_spawn
            || self
                .declarations
                .contains("declare i64 @sengoo_async_spawn_raw(i64, i64)\n")
        {
            return;
        }

        self.declarations
            .push_str("declare i64 @sengoo_async_spawn_raw(i64, i64)\n");
    }

    /// Declare the async select runtime hook only when the module actually uses it.
    pub(super) fn maybe_declare_select_runtime_function(&mut self, mir_fns: &[MirFunction]) {
        let mut needed = std::collections::BTreeSet::new();
        for mir_fn in mir_fns {
            for inst in &mir_fn.instructions {
                if let mir::Instruction::Call { func, .. } = inst {
                    if func
                        .strip_prefix("sengoo_async_select_")
                        .is_some_and(|suffix| matches!(suffix, "bool" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"))
                    {
                        needed.insert(func.clone());
                    }
                }
            }
        }

        for func in needed {
            let suffix = func
                .strip_prefix("sengoo_async_select_")
                .expect("filtered select runtime function should keep prefix");
            let ty = match suffix {
                "bool" => MIRType::Bool,
                "i8" => MIRType::Int(8),
                "i16" => MIRType::Int(16),
                "i32" => MIRType::Int(32),
                "i64" => MIRType::Int(64),
                "f32" => MIRType::Float(32),
                "f64" => MIRType::Float(64),
                _ => continue,
            };
            let decl = select_runtime_declaration(&ty)
                .expect("needed select runtime declaration should exist");
            if !self.declarations.contains(&decl) {
                self.declarations.push_str(&decl);
            }
        }
    }

    pub(super) fn maybe_declare_sleep_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let needs_sleep = mir_fns.iter().any(|mir_fn| {
            let has_sleep_call = mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => {
                    matches!(
                        func.as_str(),
                        "sengoo_async_sleep__start"
                            | "sengoo_async_sleep__poll"
                            | "sengoo_async_sleep__result"
                            | "sengoo_async_sleep__cancel"
                            | "sengoo_async_sleep__drop"
                    )
                }
                _ => false,
            });
            let has_sleep_suspend = mir_fn.basic_blocks.iter().any(|bb| match &bb.terminator {
                Some(mir::Terminator::Suspend { poll_func, .. }) => {
                    poll_func == "sengoo_async_sleep__poll"
                }
                _ => false,
            });
            has_sleep_call || has_sleep_suspend
        });

        if !needs_sleep
            || self
                .declarations
                .contains("declare i64 @sengoo_async_sleep__start(i64)\n")
        {
            return;
        }

        self.declarations
            .push_str("declare i64 @sengoo_async_sleep__start(i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_sleep__poll(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_sleep__result(i64)\n");
        self.declarations
            .push_str("declare i1 @sengoo_async_sleep__cancel(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_sleep__drop(i64)\n");
    }

    pub(super) fn maybe_declare_timeout_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let needs_timeout = mir_fns.iter().any(|mir_fn| {
            let has_timeout_call = mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => {
                    matches!(
                        func.as_str(),
                        "sengoo_async_timeout_bool__start"
                            | "sengoo_async_timeout_bool__poll"
                            | "sengoo_async_timeout_bool__result"
                            | "sengoo_async_timeout_bool__cancel"
                            | "sengoo_async_timeout_bool__drop"
                    )
                }
                _ => false,
            });
            let has_timeout_suspend = mir_fn.basic_blocks.iter().any(|bb| match &bb.terminator {
                Some(mir::Terminator::Suspend { poll_func, .. }) => {
                    poll_func == "sengoo_async_timeout_bool__poll"
                }
                _ => false,
            });
            has_timeout_call || has_timeout_suspend
        });

        if !needs_timeout
            || self
                .declarations
                .contains("declare i64 @sengoo_async_timeout_bool__start(i64, i64, i64)\n")
        {
            return;
        }

        self.declarations
            .push_str("declare i64 @sengoo_async_timeout_bool__start(i64, i64, i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_timeout_bool__poll(i64)\n");
        self.declarations
            .push_str("declare i1 @sengoo_async_timeout_bool__result(i64)\n");
        self.declarations
            .push_str("declare i1 @sengoo_async_timeout_bool__cancel(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_timeout_bool__drop(i64)\n");
    }

    pub(super) fn maybe_declare_async_task_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let needs_task_runtime = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => {
                    matches!(
                        func.as_str(),
                        "sengoo_async_cancel_task" | "sengoo_async_task_status"
                    )
                }
                _ => false,
            })
        });

        if !needs_task_runtime
            || self
                .declarations
                .contains("declare i1 @sengoo_async_cancel_task(i64)\n")
        {
            return;
        }

        self.declarations
            .push_str("declare i1 @sengoo_async_cancel_task(i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_task_status(i64)\n");
    }
}

#[cfg(test)]
mod tests {
    use crate::mir::{MIR_BOOL, MIR_UNIT};
    use super::*;

    #[test]
    fn maybe_declare_select_runtime_function_adds_bool_decl_once() {
        let mut cg = Codegen::new();
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let dest = mir_fn.add_local(LocalKind::Temp, MIR_BOOL);
        mir_fn.push_inst_to_block(
            mir_fn.start_block,
            mir::Instruction::Call {
                destination: dest,
                func: "sengoo_async_select_bool".to_string(),
                args: vec![],
            },
        );

        cg.maybe_declare_select_runtime_function(&[mir_fn.clone()]);
        cg.maybe_declare_select_runtime_function(&[mir_fn]);

        let needle = "declare i1 @sengoo_async_select_bool(i64, i64, i64, i64)\n";
        assert_eq!(cg.declarations.matches(needle).count(), 1);
    }
}
