use crate::typeck::ty::{Ty, TyKind, TypeckError};

const SUPPORTED_ABIS: &[&str] = &["C", "cdecl", "system"];

pub fn validate_abi(abi: &str) -> Result<(), TypeckError> {
    if SUPPORTED_ABIS.contains(&abi) {
        Ok(())
    } else {
        Err(TypeckError::ffi_signature(
            "ffi::unsupported_abi",
            format!(
                "unsupported ABI `{}` for FFI (supported: {})",
                abi,
                SUPPORTED_ABIS.join(", ")
            ),
            0,
            0,
        ))
    }
}

pub fn validate_signature(
    abi: &str,
    params: &[Ty],
    ret: &Ty,
    is_unsafe: bool,
) -> Result<(), TypeckError> {
    validate_abi(abi)?;
    for param in params {
        validate_ffi_type(param)?;
    }
    validate_ffi_type(ret)?;

    let has_raw_pointer = params.iter().any(contains_raw_pointer) || contains_raw_pointer(ret);
    if has_raw_pointer && !is_unsafe {
        return Err(TypeckError::ffi_signature(
            "ffi::unsafe_boundary",
            "unsafe boundary violation: raw-pointer FFI signatures must be marked `unsafe`"
                .to_string(),
            0,
            0,
        ));
    }

    Ok(())
}

pub fn validate_ffi_type(ty: &Ty) -> Result<(), TypeckError> {
    match &ty.kind {
        TyKind::Unit
        | TyKind::Bool
        | TyKind::Char
        | TyKind::Byte
        | TyKind::Int(_)
        | TyKind::Float(_) => Ok(()),
        TyKind::Ptr(inner) => validate_ffi_type(inner),
        TyKind::Ref(false, inner) if matches!(inner.kind, TyKind::Str) => Ok(()),
        TyKind::Ref(_, _) => Err(TypeckError::ffi_signature(
            "ffi::unsupported_type",
            format!(
                "FFI type is not supported: only immutable &str references are FFI-safe (`{}`)",
                ty
            ),
            0,
            0,
        )),
        _ => Err(TypeckError::ffi_signature(
            "ffi::unsupported_type",
            format!("FFI type is not supported in C ABI signatures: `{}`", ty),
            0,
            0,
        )),
    }
}

pub fn contains_raw_pointer(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Ptr(_) => true,
        TyKind::Tuple(items) => items.iter().any(contains_raw_pointer),
        TyKind::Array(inner, _) | TyKind::Slice(inner) | TyKind::Ref(_, inner) => {
            contains_raw_pointer(inner)
        }
        TyKind::Fn { params, ret, .. } => {
            params.iter().any(contains_raw_pointer) || contains_raw_pointer(ret)
        }
        TyKind::Adt { args, .. } => args.iter().any(contains_raw_pointer),
        _ => false,
    }
}
