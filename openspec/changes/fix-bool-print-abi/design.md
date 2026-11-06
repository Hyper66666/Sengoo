# Boolean output lowering

MIR booleans remain i1. The print lowering inserts the normal bool-to-i64 cast
before calling `sengoo_print_bool` or `sengoo_eprint_bool`. Runtime ABI 1 accepts
i64 and tests nonzero; changing the runtime signature would break compatibility.
Both native sinks and nested struct field output share this lowering. Regression
asserts generated call types and actual false/true output at O0 and O2.
