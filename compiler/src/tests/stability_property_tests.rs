use proptest::prelude::*;

use crate::{compile_to_ir, Parser};

fn conditional_expression_strategy() -> impl Strategy<Value = String> {
    (
        0i64..=200,
        0i64..=200,
        0i64..=200,
        0i64..=200,
        prop_oneof![
            Just(">"),
            Just("<"),
            Just(">="),
            Just("<="),
            Just("=="),
            Just("!="),
        ],
    )
        .prop_map(|(left, right, then_value, else_value, op)| {
            format!(
                "def main() -> i64 {{ let result = if {} {} {} {{ {} }} else {{ {} }}; result }}",
                left, op, right, then_value, else_value
            )
        })
}

fn struct_literal_strategy() -> impl Strategy<Value = String> {
    (0i64..=200, 0i64..=200).prop_map(|(x, y)| {
        format!(
            "struct Point {{ x: i64, y: i64 }}\n\
             def main() -> i64 {{\n\
                 let p = Point {{ x: {}, y: {} }};\n\
                 p.x\n\
             }}",
            x, y
        )
    })
}

fn match_pattern_strategy() -> impl Strategy<Value = String> {
    (0i64..=5, 0i64..=5, 0i64..=5).prop_map(|(input, p0, p1)| {
        format!(
            "def main() -> i64 {{\n\
                 let x = {};\n\
                 match x {{\n\
                     {} | {} => 1,\n\
                     _ => 0,\n\
                 }}\n\
             }}",
            input, p0, p1
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    #[test]
    fn prop_conditional_expression_compiles(source in conditional_expression_strategy()) {
        let ir = compile_to_ir(&source);
        prop_assert!(
            ir.is_ok(),
            "conditional-expression source failed to compile:\nSource: {}\nError: {:?}",
            source,
            ir.err()
        );

        let ir = ir.unwrap();
        prop_assert!(
            ir.contains("icmp"),
            "expected conditional comparison in IR.\nSource: {}\nIR: {}",
            source,
            ir
        );
        prop_assert!(
            ir.contains("phi"),
            "expected phi node for if-expression result.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    #[test]
    fn prop_struct_literal_compiles(source in struct_literal_strategy()) {
        let ir = compile_to_ir(&source);
        prop_assert!(
            ir.is_ok(),
            "struct-literal source failed to compile:\nSource: {}\nError: {:?}",
            source,
            ir.err()
        );

        let ir = ir.unwrap();
        prop_assert!(
            ir.contains("insertvalue"),
            "expected struct construction in IR.\nSource: {}\nIR: {}",
            source,
            ir
        );
    }

    #[test]
    fn prop_match_pattern_parses(source in match_pattern_strategy()) {
        let parsed = Parser::parse(&source);
        prop_assert!(
            parsed.is_ok(),
            "match-pattern source failed to parse:\nSource: {}\nError: {:?}",
            source,
            parsed.err()
        );
    }
}
