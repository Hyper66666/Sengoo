//! Audited runtime ABI observation contracts, not source-wrapper names.
//!
//! These native functions neither retain nor release their handle/buffer
//! inputs. Constructors allocate an independent String; scalar accessors do
//! not return storage borrowed from the input. Unknown externs are consuming.
//! See docs/runtime-native-borrow-contracts.md for implementation provenance.

pub(crate) fn observes_argument(name: &str, index: usize, arity: usize) -> bool {
    let (expected, observed): (usize, &[usize]) = match name {
        "sengoo_string_eq" | "sengoo_string_compare" | "sengoo_string_copy_to_buffer" => {
            (2, &[0, 1])
        }
        "sengoo_string_len"
        | "sengoo_string_is_empty"
        | "sengoo_ffi_buffer_len"
        | "sengoo_ffi_buffer_capacity"
        | "sengoo_ffi_buffer_used_len" => (1, &[0]),
        "sengoo_string_from_buffer" | "sengoo_string_from_utf8" => (2, &[0]),
        "sengoo_hash_sha256" => (4, &[0, 2]),
        "sengoo_hash_sha256_bounded" => (5, &[0, 3]),
        "sengoo_ffi_buffer_copy_in" => (3, &[0, 1]),
        "sengoo_ffi_buffer_copy_out" => (3, &[0, 1]),
        "sengoo_ffi_buffer_copy_range" => (4, &[0, 3]),
        "sengoo_ffi_buffer_get_u8" => (2, &[0]),
        "sengoo_ffi_buffer_is_utf8" => (1, &[0]),
        "sengoo_ffi_buffer_commit_used_len" | "sengoo_ffi_buffer_consume" => (2, &[0]),
        "sengoo_arg_copy" => (3, &[1]),
        "sengoo_env_var_copy" | "sengoo_file_read_into" => (3, &[0, 1]),
        "sengoo_net_stream_read" => (3, &[0, 1]),
        "sengoo_ffi_buffer_append" => (3, &[0, 1]),
        "sengoo_ffi_buffer_index_of_bytes" => (3, &[0, 1]),
        "sengoo_string_from_bytes_copy" => (2, &[0]),
        _ => return false,
    };
    arity == expected && observed.contains(&index)
}

pub(super) fn projects_argument(name: &str, index: usize, arity: usize) -> bool {
    arity == 1 && index == 0 && name == "sengoo_ffi_buffer_ptr"
}

pub(super) fn declared_observers(items: &[super::HIRItem]) -> std::collections::HashSet<String> {
    use crate::hir::{HIRExternItem, HIRItem, HIRType, HIRTypeKind, IntKind};
    fn i64_type(ty: &HIRType) -> bool {
        matches!(ty.kind, HIRTypeKind::Int(IntKind::I64))
    }
    let mut observers = std::collections::HashSet::new();
    for item in items {
        if let HIRItem::ExternBlock(block) = item {
            if block.abi != "C" {
                continue;
            }
            for item in &block.items {
                if let HIRExternItem::Function(function) = item {
                    if i64_type(&function.return_type)
                        && function.params.iter().all(|param| i64_type(&param.ty))
                        && (0..function.params.len()).any(|index| {
                            observes_argument(&function.name, index, function.params.len())
                                || projects_argument(&function.name, index, function.params.len())
                        })
                    {
                        observers.insert(function.name.clone());
                    }
                }
            }
        }
    }
    observers
}
