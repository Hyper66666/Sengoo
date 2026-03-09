use crate::compile_to_mir;
use std::collections::HashSet;

#[test]
fn default_generic_trait_method_lowers_specialized_mir_function() {
    let source = r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped = 1.wrap(true);
    if wrapped.value {
        1
    } else {
        0
    }
}
"#;

    let mir = compile_to_mir(source).expect("source should lower to MIR");
    let names = mir.iter().map(|f| f.name.as_str()).collect::<HashSet<_>>();

    assert!(names.contains("i64_WrapValue_wrap_bool"));
    assert!(!names.contains("i64_WrapValue_wrap"));
}

#[test]
fn default_generic_and_non_generic_trait_methods_lower_side_by_side() {
    let source = r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def id(self) -> i64 {
        self
    }

    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped = 1.wrap(true);
    if wrapped.value {
        1.id()
    } else {
        0
    }
}
"#;

    let mir = compile_to_mir(source).expect("source should lower to MIR");
    let names = mir.iter().map(|f| f.name.as_str()).collect::<HashSet<_>>();

    assert!(names.contains("i64_WrapValue_wrap_bool"));
    assert!(names.contains("i64_WrapValue_id"));
    assert!(!names.contains("i64_WrapValue_wrap"));
}

#[test]
fn default_generic_trait_method_supports_multiple_instantiations_in_one_program() {
    let source = r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped_bool = 1.wrap(true);
    let wrapped_i64 = 1.wrap(7);
    if wrapped_bool.value {
        wrapped_i64.value
    } else {
        0
    }
}
"#;

    let mir = compile_to_mir(source).expect("source should lower to MIR");
    let names = mir.iter().map(|f| f.name.as_str()).collect::<HashSet<_>>();

    assert!(names.contains("i64_WrapValue_wrap_bool"));
    assert!(names.contains("i64_WrapValue_wrap_i64"));
}
