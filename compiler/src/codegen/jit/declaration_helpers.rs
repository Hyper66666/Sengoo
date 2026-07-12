use super::JITCodegen;
use crate::codegen::IntegerOverflowMode;
use crate::mir::async_dispatch_synthesis_helpers::{
    select_cancel_n_winner_runtime_declaration, select_cancel_n_winner_runtime_function_name,
    select_cancel_winner_runtime_declaration, select_cancel_winner_runtime_function_name,
    select_n_winner_runtime_declaration, select_n_winner_runtime_function_name,
    select_runtime_declaration, select_runtime_function_name, select_runtime_return_type,
    select_winner_runtime_declaration, select_winner_runtime_function_name,
};
use crate::mir::MIRType;

fn select_n_winner_signature() -> (Vec<MIRType>, MIRType) {
    (vec![MIRType::Int(64); 17], MIRType::Int(64))
}

impl JITCodegen {
    pub(super) fn declare_runtime_functions(&mut self) {
        self.extern_decls.push_str("declare i32 @puts(i8*)\n");
        self.extern_decls
            .push_str("declare i32 @printf(i8*, ...)\n");
        self.extern_decls
            .push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8*, i8*, i64)\n");
        for float_width in [32_u8, 64] {
            let float_ty = if float_width == 32 { "float" } else { "double" };
            for int_width in [8_u8, 16, 32, 64] {
                for operation in ["fptosi", "fptoui"] {
                    self.extern_decls.push_str(&format!(
                        "declare i{int_width} @llvm.{operation}.sat.i{int_width}.f{float_width}({float_ty})\n"
                    ));
                }
            }
        }
        if self.integer_overflow_mode == IntegerOverflowMode::DebugChecked {
            for width in [8_u8, 16, 32, 64] {
                for signedness in ["s", "u"] {
                    for op in ["add", "sub", "mul"] {
                        self.extern_decls.push_str(&format!(
                            "declare {{ i{width}, i1 }} @llvm.{signedness}{op}.with.overflow.i{width}(i{width}, i{width})\n"
                        ));
                    }
                }
            }
            self.extern_decls
                .push_str("declare void @sengoo_panic_integer_overflow(i64)\n");
            self.extern_decls
                .push_str("declare void @sengoo_panic_division_by_zero(i64)\n");
        }
        self.extern_decls.push_str("declare i8* @malloc(i64)\n");
        self.extern_decls.push_str("declare void @free(i8*)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_string_concat_str(i64, i8*)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_async_spawn_raw(i64, i64)\n");
        self.extern_decls
            .push_str(select_winner_runtime_declaration());
        self.extern_decls
            .push_str(select_n_winner_runtime_declaration());
        self.extern_decls
            .push_str(select_cancel_winner_runtime_declaration());
        self.extern_decls
            .push_str(select_cancel_n_winner_runtime_declaration());
        for ty in [
            MIRType::Int(8),
            MIRType::Int(16),
            MIRType::Int(32),
            MIRType::Int(64),
            MIRType::Bool,
            MIRType::Float(32),
            MIRType::Float(64),
        ] {
            if let Some(decl) = select_runtime_declaration(&ty) {
                self.extern_decls.push_str(&decl);
            }
        }
        self.extern_decls
            .push_str("declare i64 @sengoo_async_sleep__start(i64)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_async_sleep__poll(i64)\n");
        self.extern_decls
            .push_str("declare void @sengoo_async_sleep__result(i64)\n");
        self.extern_decls
            .push_str("declare i1 @sengoo_async_sleep__cancel(i64)\n");
        self.extern_decls
            .push_str("declare void @sengoo_async_sleep__drop(i64)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_async_timeout_bool__start(i64, i64, i64)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_async_timeout_bool__poll(i64)\n");
        self.extern_decls
            .push_str("declare i1 @sengoo_async_timeout_bool__result(i64)\n");
        self.extern_decls
            .push_str("declare i1 @sengoo_async_timeout_bool__cancel(i64)\n");
        self.extern_decls
            .push_str("declare void @sengoo_async_timeout_bool__drop(i64)\n");
        self.extern_decls
            .push_str("declare i1 @sengoo_async_cancel_task(i64)\n");
        self.extern_decls
            .push_str("declare i64 @sengoo_async_task_status(i64)\n");
        self.function_signatures.insert(
            "sengoo_string_concat_str".to_string(),
            (
                vec![MIRType::Int(64), MIRType::Ptr(Box::new(MIRType::Int(8)))],
                MIRType::Int(64),
            ),
        );
        self.function_signatures.insert(
            "sengoo_async_spawn_raw".to_string(),
            (vec![MIRType::Int(64), MIRType::Int(64)], MIRType::Int(64)),
        );
        self.function_signatures.insert(
            select_winner_runtime_function_name().to_string(),
            (
                vec![
                    MIRType::Int(64),
                    MIRType::Int(64),
                    MIRType::Int(64),
                    MIRType::Int(64),
                ],
                MIRType::Int(64),
            ),
        );
        self.function_signatures.insert(
            select_n_winner_runtime_function_name().to_string(),
            select_n_winner_signature(),
        );
        self.function_signatures.insert(
            select_cancel_winner_runtime_function_name().to_string(),
            (
                vec![
                    MIRType::Int(64),
                    MIRType::Int(64),
                    MIRType::Int(64),
                    MIRType::Int(64),
                ],
                MIRType::Int(64),
            ),
        );
        self.function_signatures.insert(
            select_cancel_n_winner_runtime_function_name().to_string(),
            select_n_winner_signature(),
        );
        for ty in [
            MIRType::Int(8),
            MIRType::Int(16),
            MIRType::Int(32),
            MIRType::Int(64),
            MIRType::Bool,
            MIRType::Float(32),
            MIRType::Float(64),
        ] {
            if let (Some(name), Some(ret_ty)) = (
                select_runtime_function_name(&ty),
                select_runtime_return_type(&ty),
            ) {
                self.function_signatures.insert(
                    name,
                    (
                        vec![
                            MIRType::Int(64),
                            MIRType::Int(64),
                            MIRType::Int(64),
                            MIRType::Int(64),
                        ],
                        ret_ty,
                    ),
                );
            }
        }
        self.function_signatures.insert(
            "sengoo_async_sleep__start".to_string(),
            (vec![MIRType::Int(64)], MIRType::Int(64)),
        );
        self.function_signatures.insert(
            "sengoo_async_sleep__poll".to_string(),
            (vec![MIRType::Int(64)], MIRType::Int(64)),
        );
        self.function_signatures.insert(
            "sengoo_async_sleep__result".to_string(),
            (vec![MIRType::Int(64)], MIRType::Unit),
        );
        self.function_signatures.insert(
            "sengoo_async_sleep__cancel".to_string(),
            (vec![MIRType::Int(64)], MIRType::Bool),
        );
        self.function_signatures.insert(
            "sengoo_async_sleep__drop".to_string(),
            (vec![MIRType::Int(64)], MIRType::Unit),
        );
        self.function_signatures.insert(
            "sengoo_async_timeout_bool__start".to_string(),
            (
                vec![MIRType::Int(64), MIRType::Int(64), MIRType::Int(64)],
                MIRType::Int(64),
            ),
        );
        self.function_signatures.insert(
            "sengoo_async_timeout_bool__poll".to_string(),
            (vec![MIRType::Int(64)], MIRType::Int(64)),
        );
        self.function_signatures.insert(
            "sengoo_async_timeout_bool__result".to_string(),
            (vec![MIRType::Int(64)], MIRType::Bool),
        );
        self.function_signatures.insert(
            "sengoo_async_timeout_bool__cancel".to_string(),
            (vec![MIRType::Int(64)], MIRType::Bool),
        );
        self.function_signatures.insert(
            "sengoo_async_timeout_bool__drop".to_string(),
            (vec![MIRType::Int(64)], MIRType::Unit),
        );
        self.function_signatures.insert(
            "sengoo_async_cancel_task".to_string(),
            (vec![MIRType::Int(64)], MIRType::Bool),
        );
        self.function_signatures.insert(
            "sengoo_async_task_status".to_string(),
            (vec![MIRType::Int(64)], MIRType::Int(64)),
        );
    }
}
