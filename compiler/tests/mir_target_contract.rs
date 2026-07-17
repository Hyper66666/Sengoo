use sengoo_compiler::mir::{MIRType, MirFunction};
use sengoo_compiler::{
    compile_to_mir_bundle_for_target, CompileOptions, TargetPointerWidth, MIR_SEMANTIC_ABI_VERSION,
};

fn mir_function<'a>(functions: &'a [MirFunction], name: &str) -> &'a MirFunction {
    functions
        .iter()
        .find(|function| function.name == name)
        .unwrap_or_else(|| panic!("expected MIR function `{name}`"))
}

#[test]
fn wasm32_target_uses_32_bit_pointer_sized_mir_types() {
    let source = r#"
def signed(value: isize) -> isize { value }
def unsigned(value: usize) -> usize { value }
def main() -> isize { signed(unsigned(7usize) as isize) }
"#;

    let bundle = compile_to_mir_bundle_for_target(
        source,
        CompileOptions::default(),
        "wasm32-unknown-unknown",
    )
    .expect("wasm32 target should lower pointer-sized MIR types as 32-bit");

    assert_eq!(bundle.target_pointer_width, TargetPointerWidth::Bits32);

    let signed = mir_function(&bundle.functions, "signed");
    assert_eq!(signed.params, vec![MIRType::Int(32)]);
    assert_eq!(signed.return_type, MIRType::Int(32));

    let unsigned = mir_function(&bundle.functions, "unsigned");
    assert_eq!(unsigned.params, vec![MIRType::UInt(32)]);
    assert_eq!(unsigned.return_type, MIRType::UInt(32));

    let main = mir_function(&bundle.functions, "main");
    assert_eq!(main.return_type, MIRType::Int(32));
}

#[test]
fn wasm32_target_rejects_usize_literals_that_only_fit_64_bit_targets() {
    let error = compile_to_mir_bundle_for_target(
        "def main() -> usize { 4294967296usize }",
        CompileOptions::default(),
        "wasm32-unknown-unknown",
    )
    .expect_err("wasm32 usize should reject 64-bit-only literals")
    .to_string();

    assert!(error.contains("exceeds range of `usize`"), "{error}");
}

#[test]
fn mir_bundle_records_semantic_abi_version_target_width_and_ffi_metadata() {
    let source = r#"
extern "C" {
    pub fn host_len(value: usize) -> usize;
}

#[export_name = "wasm_entry"]
pub extern "C" fn exported(value: usize) -> usize {
    value
}

def main() -> usize {
    exported(7usize)
}
"#;

    let bundle = compile_to_mir_bundle_for_target(
        source,
        CompileOptions::default(),
        "wasm32-unknown-unknown",
    )
    .expect("bundle contract should preserve MIR metadata");

    assert_eq!(bundle.semantic_abi_version, MIR_SEMANTIC_ABI_VERSION);
    assert_eq!(bundle.target_pointer_width, TargetPointerWidth::Bits32);

    let decl = bundle
        .ffi_codegen
        .extern_decls
        .iter()
        .find(|decl| decl.name == "host_len")
        .expect("extern decl metadata should be present");
    assert_eq!(decl.abi, "C");
    assert_eq!(decl.params, vec![MIRType::UInt(32)]);
    assert_eq!(decl.ret, MIRType::UInt(32));

    let export = bundle
        .ffi_codegen
        .export_symbols
        .iter()
        .find(|export| export.internal_name == "exported")
        .expect("export metadata should be present");
    assert_eq!(export.export_name, "wasm_entry");
}
