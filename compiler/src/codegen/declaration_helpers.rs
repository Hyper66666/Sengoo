use super::*;
use crate::mir::async_dispatch_synthesis_helpers::{
    select_cancel_n_winner_runtime_declaration, select_cancel_n_winner_runtime_function_name,
    select_cancel_winner_runtime_declaration, select_cancel_winner_runtime_function_name,
    select_n_winner_runtime_declaration, select_n_winner_runtime_function_name,
    select_runtime_declaration, select_winner_runtime_declaration,
    select_winner_runtime_function_name,
};

impl Codegen {
    /// Declare external runtime functions used by generated LLVM IR.
    pub(super) fn declare_runtime_functions(&mut self) {
        self.declarations
            .push_str("; External C library functions\n");

        self.declarations.push_str("declare i32 @puts(i8*)\n");

        self.declarations
            .push_str("declare i32 @printf(i8*, ...)\n");

        self.declarations.push('\n');

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

        self.declarations.push('\n');

        // Sengoo runtime string functions

        self.declarations
            .push_str("; Sengoo runtime string functions\n");

        self.declarations
            .push_str("declare i64 @sengoo_str_len(i8*)\n");

        self.declarations
            .push_str("declare i8* @sengoo_str_concat(i8*, i8*)\n");

        self.declarations
            .push_str("declare i64 @sengoo_str_eq(i8*, i8*)\n");

        self.declarations
            .push_str("declare i64 @sengoo_str_compare(i8*, i8*)\n");

        self.declarations.push('\n');

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
            .push_str("declare i64 @sengoo_async_run_main_i64()\n");
        self.declarations.push('\n');

        if self.integer_overflow_mode == IntegerOverflowMode::DebugChecked {
            self.declarations
                .push_str("; LLVM integer overflow check intrinsics\n");
            for width in [8_u8, 16, 32, 64] {
                for signedness in ["s", "u"] {
                    for op in ["add", "sub", "mul"] {
                        self.declarations.push_str(&format!(
                            "declare {{ i{width}, i1 }} @llvm.{signedness}{op}.with.overflow.i{width}(i{width}, i{width})\n"
                        ));
                    }
                }
            }
            self.declarations
                .push_str("declare void @sengoo_panic_integer_overflow(i64)\n");
            self.declarations
                .push_str("declare void @sengoo_panic_division_by_zero(i64)\n");
            self.declarations.push('\n');
        }

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

        self.declarations.push('\n');
    }

    /// Declare only the saturating conversion intrinsics referenced by this module.
    pub(super) fn maybe_declare_saturating_float_to_int_intrinsics(
        &mut self,
        mir_fns: &[MirFunction],
    ) {
        let mut needed = std::collections::BTreeSet::new();
        for mir_fn in mir_fns {
            for inst in &mir_fn.instructions {
                let mir::Instruction::Cast { value, to, .. } = inst else {
                    continue;
                };
                let Some((_, source_ty)) = mir_fn.locals.get(value.index()) else {
                    continue;
                };
                let (operation, int_width, float_width) = match (source_ty, to) {
                    (MIRType::Float(float_width), MIRType::Int(int_width)) => {
                        ("fptosi", *int_width, *float_width)
                    }
                    (MIRType::Float(float_width), MIRType::UInt(int_width)) => {
                        ("fptoui", *int_width, *float_width)
                    }
                    _ => continue,
                };
                needed.insert((operation, int_width, float_width));
            }
        }

        if needed.is_empty() {
            return;
        }

        self.declarations
            .push_str("; LLVM saturating float-to-integer intrinsics\n");
        for (operation, int_width, float_width) in needed {
            let float_ty = if float_width == 32 { "float" } else { "double" };
            self.declarations.push_str(&format!(
                "declare i{int_width} @llvm.{operation}.sat.i{int_width}.f{float_width}({float_ty})\n"
            ));
        }
        self.declarations.push('\n');
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

    pub(super) fn maybe_declare_eprint_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let declarations = [
            (
                "sengoo_print_string",
                "declare void @sengoo_print_string(i64)\n",
            ),
            (
                "sengoo_eprint_i64",
                "declare void @sengoo_eprint_i64(i64)\n",
            ),
            (
                "sengoo_eprint_bool",
                "declare void @sengoo_eprint_bool(i64)\n",
            ),
            (
                "sengoo_eprint_f64",
                "declare void @sengoo_eprint_f64(double)\n",
            ),
            (
                "sengoo_eprint_str",
                "declare void @sengoo_eprint_str(i8*)\n",
            ),
            (
                "sengoo_eprint_string",
                "declare void @sengoo_eprint_string(i64)\n",
            ),
        ];

        let mut needed = std::collections::BTreeSet::new();
        for mir_fn in mir_fns {
            for inst in &mir_fn.instructions {
                if let mir::Instruction::Call { func, .. } = inst {
                    needed.insert(func.as_str());
                }
            }
            for block in &mir_fn.basic_blocks {
                if let Some(mir::Terminator::Call { func, .. }) = &block.terminator {
                    needed.insert(func.as_str());
                }
            }
        }

        for (name, decl) in declarations {
            if needed.contains(name) && !self.declarations.contains(decl) {
                self.declarations.push_str(decl);
            }
        }
    }

    /// Declare the async select runtime hook only when the module actually uses it.
    pub(super) fn maybe_declare_select_runtime_function(&mut self, mir_fns: &[MirFunction]) {
        let winner_decl = select_winner_runtime_declaration();
        let needs_winner = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| {
                matches!(
                    inst,
                    mir::Instruction::Call { func, .. }
                        if func == select_winner_runtime_function_name()
                )
            })
        });
        if needs_winner && !self.declarations.contains(winner_decl) {
            self.declarations.push_str(winner_decl);
        }

        let n_winner_decl = select_n_winner_runtime_declaration();
        let needs_n_winner = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| {
                matches!(
                    inst,
                    mir::Instruction::Call { func, .. }
                        if func == select_n_winner_runtime_function_name()
                )
            })
        });
        if needs_n_winner && !self.declarations.contains(n_winner_decl) {
            self.declarations.push_str(n_winner_decl);
        }

        let cancel_winner_decl = select_cancel_winner_runtime_declaration();
        let needs_cancel_winner = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| {
                matches!(
                    inst,
                    mir::Instruction::Call { func, .. }
                        if func == select_cancel_winner_runtime_function_name()
                )
            })
        });
        if needs_cancel_winner && !self.declarations.contains(cancel_winner_decl) {
            self.declarations.push_str(cancel_winner_decl);
        }

        let cancel_n_winner_decl = select_cancel_n_winner_runtime_declaration();
        let needs_cancel_n_winner = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| {
                matches!(
                    inst,
                    mir::Instruction::Call { func, .. }
                        if func == select_cancel_n_winner_runtime_function_name()
                )
            })
        });
        if needs_cancel_n_winner && !self.declarations.contains(cancel_n_winner_decl) {
            self.declarations.push_str(cancel_n_winner_decl);
        }

        let mut needed = std::collections::BTreeSet::new();
        for mir_fn in mir_fns {
            for inst in &mir_fn.instructions {
                if let mir::Instruction::Call { func, .. } = inst {
                    if func
                        .strip_prefix("sengoo_async_select_")
                        .is_some_and(|suffix| {
                            matches!(
                                suffix,
                                "bool" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"
                            )
                        })
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

    pub(super) fn maybe_declare_timeout_cancel_runtime_functions(
        &mut self,
        mir_fns: &[MirFunction],
    ) {
        let needs_timeout_cancel = mir_fns.iter().any(|mir_fn| {
            let has_call = mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => {
                    matches!(
                        func.as_str(),
                        "sengoo_async_timeout_cancel_i64__start"
                            | "sengoo_async_timeout_cancel_i64__poll"
                            | "sengoo_async_timeout_cancel_i64__result"
                            | "sengoo_async_timeout_cancel_i64__cancel"
                            | "sengoo_async_timeout_cancel_i64__drop"
                    )
                }
                _ => false,
            });
            let has_suspend = mir_fn.basic_blocks.iter().any(|bb| match &bb.terminator {
                Some(mir::Terminator::Suspend { poll_func, .. }) => {
                    poll_func == "sengoo_async_timeout_cancel_i64__poll"
                }
                _ => false,
            });
            has_call || has_suspend
        });

        if !needs_timeout_cancel
            || self
                .declarations
                .contains("declare i64 @sengoo_async_timeout_cancel_i64__start(i64, i64, i64)\n")
        {
            return;
        }

        self.declarations
            .push_str("declare i64 @sengoo_async_timeout_cancel_i64__start(i64, i64, i64)\n");
        self.declarations
            .push_str("declare i64 @sengoo_async_timeout_cancel_i64__poll(i64)\n");
        self.declarations.push_str(Self::sret_or_direct_decl(
            Self::async_result_uses_sret(
                self.targets_windows_msvc(),
                "sengoo_async_timeout_cancel_i64__result",
            ),
            "sengoo_async_timeout_cancel_i64__result",
            "{ i1, i64, i64 }",
        ));
        self.declarations
            .push_str("declare i1 @sengoo_async_timeout_cancel_i64__cancel(i64)\n");
        self.declarations
            .push_str("declare void @sengoo_async_timeout_cancel_i64__drop(i64)\n");
    }

    fn mir_uses_async_origin(mir_fns: &[MirFunction], origin: &str) -> bool {
        mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func.contains(origin),
                _ => false,
            }) || mir_fn.basic_blocks.iter().any(|bb| match &bb.terminator {
                Some(mir::Terminator::Suspend { poll_func, .. }) => poll_func.contains(origin),
                Some(mir::Terminator::Call { func, .. }) => func.contains(origin),
                _ => false,
            })
        })
    }

    fn mir_uses_concurrent_async_runtime(mir_fns: &[MirFunction]) -> bool {
        const MARKERS: &[&str] = &[
            "sengoo_async_runtime_enable_thread_pool",
            "sengoo_async_spawn_blocking_i64",
            "sengoo_async_channel_bounded_i64",
            "sengoo_async_channel_send_i64",
            "sengoo_async_channel_recv_i64",
            "sengoo_async_mutex_lock_i64",
        ];
        mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => {
                    MARKERS.iter().any(|marker| func.contains(marker))
                }
                _ => false,
            })
        })
    }

    pub(super) fn maybe_declare_concurrent_async_runtime_functions(
        &mut self,
        mir_fns: &[MirFunction],
    ) {
        if !Self::mir_uses_concurrent_async_runtime(mir_fns)
            && !Self::mir_uses_async_origin(mir_fns, "sengoo_async_spawn_blocking_i64")
            && !Self::mir_uses_async_origin(mir_fns, "sengoo_async_channel_send_i64")
            && !Self::mir_uses_async_origin(mir_fns, "sengoo_async_channel_recv_i64")
            && !Self::mir_uses_async_origin(mir_fns, "sengoo_async_mutex_lock_i64")
            && !Self::mir_uses_async_origin(mir_fns, "sengoo_http_server_next_request_async")
        {
            return;
        }

        let targets_windows_msvc = self.targets_windows_msvc();

        Self::maybe_declare_async_runtime_lifecycle(
            &mut self.declarations,
            mir_fns,
            "sengoo_async_spawn_blocking_i64",
            &[
                (
                    "poll",
                    "declare i64 @sengoo_async_spawn_blocking_i64__poll(i64)\n",
                ),
                (
                    "result",
                    "declare i64 @sengoo_async_spawn_blocking_i64__result(i64)\n",
                ),
                (
                    "cancel",
                    "declare i1 @sengoo_async_spawn_blocking_i64__cancel(i64)\n",
                ),
                (
                    "drop",
                    "declare void @sengoo_async_spawn_blocking_i64__drop(i64)\n",
                ),
            ],
        );
        Self::maybe_declare_async_runtime_lifecycle(
            &mut self.declarations,
            mir_fns,
            "sengoo_async_channel_send_i64",
            &[
                (
                    "poll",
                    "declare i64 @sengoo_async_channel_send_i64__poll(i64)\n",
                ),
                (
                    "result",
                    Self::sret_or_direct_decl(
                        Self::async_result_uses_sret(
                            targets_windows_msvc,
                            "sengoo_async_channel_send_i64__result",
                        ),
                        "sengoo_async_channel_send_i64__result",
                        "{ i1, i64 }",
                    ),
                ),
                (
                    "cancel",
                    "declare i1 @sengoo_async_channel_send_i64__cancel(i64)\n",
                ),
                (
                    "drop",
                    "declare void @sengoo_async_channel_send_i64__drop(i64)\n",
                ),
            ],
        );
        Self::maybe_declare_async_runtime_lifecycle(
            &mut self.declarations,
            mir_fns,
            "sengoo_async_channel_recv_i64",
            &[
                (
                    "poll",
                    "declare i64 @sengoo_async_channel_recv_i64__poll(i64)\n",
                ),
                (
                    "result",
                    Self::sret_or_direct_decl(
                        Self::async_result_uses_sret(
                            targets_windows_msvc,
                            "sengoo_async_channel_recv_i64__result",
                        ),
                        "sengoo_async_channel_recv_i64__result",
                        "{ i1, i64, i64 }",
                    ),
                ),
                (
                    "cancel",
                    "declare i1 @sengoo_async_channel_recv_i64__cancel(i64)\n",
                ),
                (
                    "drop",
                    "declare void @sengoo_async_channel_recv_i64__drop(i64)\n",
                ),
            ],
        );
        Self::maybe_declare_async_runtime_lifecycle(
            &mut self.declarations,
            mir_fns,
            "sengoo_async_mutex_lock_i64",
            &[
                (
                    "poll",
                    "declare i64 @sengoo_async_mutex_lock_i64__poll(i64)\n",
                ),
                (
                    "result",
                    Self::sret_or_direct_decl(
                        Self::async_result_uses_sret(
                            targets_windows_msvc,
                            "sengoo_async_mutex_lock_i64__result",
                        ),
                        "sengoo_async_mutex_lock_i64__result",
                        "{ i1, i64, i64 }",
                    ),
                ),
                (
                    "cancel",
                    "declare i1 @sengoo_async_mutex_lock_i64__cancel(i64)\n",
                ),
                (
                    "drop",
                    "declare void @sengoo_async_mutex_lock_i64__drop(i64)\n",
                ),
            ],
        );
        Self::maybe_declare_optional_async_runtime_lifecycle(
            &mut self.declarations,
            mir_fns,
            "sengoo_http_server_next_request_async",
            &[
                (
                    "poll",
                    "declare i64 @sengoo_http_server_next_request_async__poll(i64)\n",
                ),
                (
                    "result",
                    Self::sret_or_direct_decl(
                        Self::async_result_uses_sret(
                            targets_windows_msvc,
                            "sengoo_http_server_next_request_async__result",
                        ),
                        "sengoo_http_server_next_request_async__result",
                        "%HttpServerNextRequestOutcome",
                    ),
                ),
                (
                    "cancel",
                    "declare i1 @sengoo_http_server_next_request_async__cancel(i64)\n",
                ),
                (
                    "drop",
                    "declare void @sengoo_http_server_next_request_async__drop(i64)\n",
                ),
            ],
        );
    }

    fn maybe_declare_optional_async_runtime_lifecycle(
        declarations: &mut String,
        mir_fns: &[MirFunction],
        origin: &str,
        lifecycle_decls: &[(&str, &str)],
    ) {
        if !Self::mir_uses_async_origin(mir_fns, origin) {
            return;
        }
        for (_, decl) in lifecycle_decls {
            if !declarations.contains(decl) {
                declarations.push_str(decl);
            }
        }
    }

    fn maybe_declare_async_runtime_lifecycle(
        declarations: &mut String,
        mir_fns: &[MirFunction],
        origin: &str,
        lifecycle_decls: &[(&str, &str)],
    ) {
        if !Self::mir_uses_async_origin(mir_fns, origin)
            && !Self::mir_uses_concurrent_async_runtime(mir_fns)
        {
            return;
        }
        for (_, decl) in lifecycle_decls {
            if !declarations.contains(decl) {
                declarations.push_str(decl);
            }
        }
    }

    pub(super) fn async_result_uses_sret(targets_windows_msvc: bool, func: &str) -> bool {
        if targets_windows_msvc {
            return matches!(
                func,
                "sengoo_async_timeout_cancel_i64__result"
                    | "sengoo_async_channel_send_i64__result"
                    | "sengoo_async_channel_recv_i64__result"
                    | "sengoo_async_mutex_lock_i64__result"
                    | "sengoo_http_server_next_request_async__result"
            );
        }

        // SysV returns aggregates larger than two eightbytes through a hidden
        // result pointer. The send outcome is only 16 bytes and remains direct.
        matches!(
            func,
            "sengoo_async_timeout_cancel_i64__result"
                | "sengoo_async_channel_recv_i64__result"
                | "sengoo_async_mutex_lock_i64__result"
                | "sengoo_http_server_next_request_async__result"
        )
    }

    fn sret_or_direct_decl(use_sret: bool, func: &str, ret_ty: &str) -> &'static str {
        if !use_sret {
            return match func {
                "sengoo_async_timeout_cancel_i64__result" => {
                    "declare { i1, i64, i64 } @sengoo_async_timeout_cancel_i64__result(i64)\n"
                }
                "sengoo_async_channel_send_i64__result" => {
                    "declare { i1, i64 } @sengoo_async_channel_send_i64__result(i64)\n"
                }
                "sengoo_async_channel_recv_i64__result" => {
                    "declare { i1, i64, i64 } @sengoo_async_channel_recv_i64__result(i64)\n"
                }
                "sengoo_async_mutex_lock_i64__result" => {
                    "declare { i1, i64, i64 } @sengoo_async_mutex_lock_i64__result(i64)\n"
                }
                "sengoo_http_server_next_request_async__result" => {
                    "declare %HttpServerNextRequestOutcome @sengoo_http_server_next_request_async__result(i64)\n"
                }
                _ => unreachable!("unsupported async result declaration"),
            };
        }

        match (func, ret_ty) {
            ("sengoo_async_channel_send_i64__result", "{ i1, i64 }") => {
                "declare void @sengoo_async_channel_send_i64__result({ i1, i64 }* sret({ i1, i64 }) align 8, i64)\n"
            }
            ("sengoo_async_timeout_cancel_i64__result", "{ i1, i64, i64 }") => {
                "declare void @sengoo_async_timeout_cancel_i64__result({ i1, i64, i64 }* sret({ i1, i64, i64 }) align 8, i64)\n"
            }
            ("sengoo_async_channel_recv_i64__result", "{ i1, i64, i64 }") => {
                "declare void @sengoo_async_channel_recv_i64__result({ i1, i64, i64 }* sret({ i1, i64, i64 }) align 8, i64)\n"
            }
            ("sengoo_async_mutex_lock_i64__result", "{ i1, i64, i64 }") => {
                "declare void @sengoo_async_mutex_lock_i64__result({ i1, i64, i64 }* sret({ i1, i64, i64 }) align 8, i64)\n"
            }
            (
                "sengoo_http_server_next_request_async__result",
                "%HttpServerNextRequestOutcome",
            ) => {
                "declare void @sengoo_http_server_next_request_async__result(%HttpServerNextRequestOutcome* sret(%HttpServerNextRequestOutcome) align 8, i64)\n"
            }
            _ => unreachable!("unsupported async result declaration"),
        }
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

    pub(super) fn maybe_declare_coverage_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let mut needed = std::collections::BTreeSet::new();
        for mir_fn in mir_fns {
            for inst in &mir_fn.instructions {
                if let mir::Instruction::Call { func, .. } = inst {
                    if matches!(
                        func.as_str(),
                        "sengoo_coverage_register" | "sengoo_coverage_hit"
                    ) {
                        needed.insert(func.as_str());
                    }
                }
            }
        }

        for (name, declaration) in [
            (
                "sengoo_coverage_register",
                "declare void @sengoo_coverage_register(i64)\n",
            ),
            (
                "sengoo_coverage_hit",
                "declare void @sengoo_coverage_hit(i64)\n",
            ),
        ] {
            if needed.contains(name) && !self.declarations.contains(declaration) {
                self.declarations.push_str(declaration);
            }
        }
        if !needed.is_empty() {
            self.declarations.push('\n');
        }
    }

    pub(super) fn maybe_declare_rc_runtime_functions(&mut self, mir_fns: &[MirFunction]) {
        let needs_rc_copy = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_rc_new_copy",
                _ => false,
            })
        });
        let needs_rc_borrow = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_rc_borrow_ptr",
                _ => false,
            })
        });
        let needs_raw_vec = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_new_parts",
                _ => false,
            })
        });
        let needs_raw_zero_bytes = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_zero_bytes",
                _ => false,
            })
        });
        let needs_raw_vec_push = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_push",
                _ => false,
            })
        });
        let needs_raw_vec_set = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_set",
                _ => false,
            })
        });
        let needs_raw_vec_insert = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_insert",
                _ => false,
            })
        });
        let needs_raw_vec_get = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_get",
                _ => false,
            })
        });
        let needs_raw_vec_pop = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_pop",
                _ => false,
            })
        });
        let needs_raw_vec_remove = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_remove",
                _ => false,
            })
        });
        let needs_raw_vec_iter_next = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_vec_iter_next",
                _ => false,
            })
        });
        let needs_raw_hashmap_new = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_new_parts",
                _ => false,
            })
        });
        let needs_raw_hashmap_insert = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_insert",
                _ => false,
            })
        });
        let needs_raw_hashmap_get = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_get",
                _ => false,
            })
        });
        let needs_raw_hashmap_contains = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_contains",
                _ => false,
            })
        });
        let needs_raw_hashmap_remove = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_remove",
                _ => false,
            })
        });
        let needs_raw_hashmap_remove_string = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_hashmap_remove_string",
                _ => false,
            })
        });
        let needs_raw_btreemap_new = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_btreemap_new_parts",
                _ => false,
            })
        });
        let needs_raw_map_key_iter_next = mir_fns.iter().any(|mir_fn| {
            mir_fn.instructions.iter().any(|inst| match inst {
                mir::Instruction::Call { func, .. } => func == "sengoo_raw_map_key_iter_next",
                _ => false,
            })
        });
        let copy_decl = "declare i64 @sengoo_rc_new_copy(i8*, i64, i8*)\n";
        let borrow_decl = "declare i8* @sengoo_rc_borrow_ptr(i64)\n";
        let raw_vec_decl = "declare i64 @sengoo_raw_vec_new_parts(i64, i64, i8*, i8*)\n";
        let raw_zero_bytes_decl = "declare void @sengoo_raw_zero_bytes(i8*, i64)\n";
        let raw_vec_push_decl = "declare i64 @sengoo_raw_vec_push(i64, i8*)\n";
        let raw_vec_set_decl = "declare i64 @sengoo_raw_vec_set(i64, i64, i8*)\n";
        let raw_vec_insert_decl = "declare i64 @sengoo_raw_vec_insert(i64, i64, i8*)\n";
        let raw_vec_get_decl = "declare i8* @sengoo_raw_vec_get(i64, i64)\n";
        let raw_vec_pop_decl = "declare i64 @sengoo_raw_vec_pop(i64, i8*)\n";
        let raw_vec_remove_decl = "declare i64 @sengoo_raw_vec_remove(i64, i64, i8*)\n";
        let raw_vec_iter_next_decl = "declare i8* @sengoo_raw_vec_iter_next(i64)\n";
        let raw_hashmap_new_decl =
            "declare i64 @sengoo_raw_hashmap_new_parts(i64, i64, i8*, i8*, i8*, i8*, i64, i64, i8*, i8*)\n";
        let raw_hashmap_insert_decl = "declare i64 @sengoo_raw_hashmap_insert(i64, i8*, i8*)\n";
        let raw_hashmap_get_decl = "declare i8* @sengoo_raw_hashmap_get(i64, i8*)\n";
        let raw_hashmap_contains_decl = "declare i64 @sengoo_raw_hashmap_contains(i64, i8*)\n";
        let raw_hashmap_remove_decl = "declare i64 @sengoo_raw_hashmap_remove(i64, i8*, i8*)\n";
        let raw_hashmap_remove_string_decl =
            "declare i64 @sengoo_raw_hashmap_remove_string(i64, i8*)\n";
        let raw_btreemap_new_decl =
            "declare i64 @sengoo_raw_btreemap_new_parts(i64, i64, i8*, i8*, i8*, i64, i64, i8*, i8*)\n";
        let raw_map_key_iter_next_decl = "declare i8* @sengoo_raw_map_key_iter_next(i64)\n";
        let needs_raw_vec_values = needs_raw_zero_bytes
            || needs_raw_vec_push
            || needs_raw_vec_set
            || needs_raw_vec_insert
            || needs_raw_vec_get
            || needs_raw_vec_pop
            || needs_raw_vec_remove
            || needs_raw_vec_iter_next
            || needs_raw_hashmap_new
            || needs_raw_hashmap_insert
            || needs_raw_hashmap_get
            || needs_raw_hashmap_contains
            || needs_raw_hashmap_remove
            || needs_raw_hashmap_remove_string
            || needs_raw_btreemap_new
            || needs_raw_map_key_iter_next;
        if (needs_rc_copy || needs_rc_borrow || needs_raw_vec || needs_raw_vec_values)
            && !self
                .declarations
                .contains("; Sengoo generic Rc runtime functions\n")
        {
            self.declarations
                .push_str("; Sengoo generic Rc runtime functions\n");
        }
        if needs_rc_copy && !self.declarations.contains(copy_decl) {
            self.declarations.push_str(copy_decl);
        }
        if needs_rc_borrow && !self.declarations.contains(borrow_decl) {
            self.declarations.push_str(borrow_decl);
        }
        if needs_raw_vec && !self.declarations.contains(raw_vec_decl) {
            self.declarations.push_str(raw_vec_decl);
        }
        if needs_raw_zero_bytes && !self.declarations.contains(raw_zero_bytes_decl) {
            self.declarations.push_str(raw_zero_bytes_decl);
        }
        if needs_raw_vec_push && !self.declarations.contains(raw_vec_push_decl) {
            self.declarations.push_str(raw_vec_push_decl);
        }
        if needs_raw_vec_set && !self.declarations.contains(raw_vec_set_decl) {
            self.declarations.push_str(raw_vec_set_decl);
        }
        if needs_raw_vec_insert && !self.declarations.contains(raw_vec_insert_decl) {
            self.declarations.push_str(raw_vec_insert_decl);
        }
        if needs_raw_vec_get && !self.declarations.contains(raw_vec_get_decl) {
            self.declarations.push_str(raw_vec_get_decl);
        }
        if needs_raw_vec_pop && !self.declarations.contains(raw_vec_pop_decl) {
            self.declarations.push_str(raw_vec_pop_decl);
        }
        if needs_raw_vec_remove && !self.declarations.contains(raw_vec_remove_decl) {
            self.declarations.push_str(raw_vec_remove_decl);
        }
        if needs_raw_vec_iter_next && !self.declarations.contains(raw_vec_iter_next_decl) {
            self.declarations.push_str(raw_vec_iter_next_decl);
        }
        if needs_raw_hashmap_new && !self.declarations.contains(raw_hashmap_new_decl) {
            self.declarations.push_str(raw_hashmap_new_decl);
        }
        if needs_raw_hashmap_insert && !self.declarations.contains(raw_hashmap_insert_decl) {
            self.declarations.push_str(raw_hashmap_insert_decl);
        }
        if needs_raw_hashmap_get && !self.declarations.contains(raw_hashmap_get_decl) {
            self.declarations.push_str(raw_hashmap_get_decl);
        }
        if needs_raw_hashmap_contains && !self.declarations.contains(raw_hashmap_contains_decl) {
            self.declarations.push_str(raw_hashmap_contains_decl);
        }
        if needs_raw_hashmap_remove && !self.declarations.contains(raw_hashmap_remove_decl) {
            self.declarations.push_str(raw_hashmap_remove_decl);
        }
        if needs_raw_hashmap_remove_string
            && !self.declarations.contains(raw_hashmap_remove_string_decl)
        {
            self.declarations.push_str(raw_hashmap_remove_string_decl);
        }
        if needs_raw_btreemap_new && !self.declarations.contains(raw_btreemap_new_decl) {
            self.declarations.push_str(raw_btreemap_new_decl);
        }
        if needs_raw_map_key_iter_next && !self.declarations.contains(raw_map_key_iter_next_decl) {
            self.declarations.push_str(raw_map_key_iter_next_decl);
        }
        if needs_rc_copy || needs_rc_borrow || needs_raw_vec || needs_raw_vec_values {
            self.declarations.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{MIR_BOOL, MIR_UNIT};

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

    #[test]
    fn maybe_declare_select_runtime_function_adds_winner_decl_once() {
        let mut cg = Codegen::new();
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let dest = mir_fn.add_local(LocalKind::Temp, crate::mir::MIR_I64);
        mir_fn.push_inst_to_block(
            mir_fn.start_block,
            mir::Instruction::Call {
                destination: dest,
                func: "sengoo_async_select_winner".to_string(),
                args: vec![],
            },
        );

        cg.maybe_declare_select_runtime_function(&[mir_fn.clone()]);
        cg.maybe_declare_select_runtime_function(&[mir_fn]);

        let needle = "declare i64 @sengoo_async_select_winner(i64, i64, i64, i64)\n";
        assert_eq!(cg.declarations.matches(needle).count(), 1);
    }

    #[test]
    fn async_result_sret_rules_match_supported_native_abis() {
        assert!(Codegen::async_result_uses_sret(
            false,
            "sengoo_async_channel_recv_i64__result"
        ));
        assert!(!Codegen::async_result_uses_sret(
            false,
            "sengoo_async_channel_send_i64__result"
        ));
        assert!(Codegen::async_result_uses_sret(
            true,
            "sengoo_async_channel_send_i64__result"
        ));
        assert!(Codegen::async_result_uses_sret(
            false,
            "sengoo_http_server_next_request_async__result"
        ));
        assert!(Codegen::async_result_uses_sret(
            true,
            "sengoo_http_server_next_request_async__result"
        ));
    }
}
