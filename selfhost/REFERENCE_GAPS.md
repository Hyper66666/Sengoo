# Frozen reference gaps

The qualified Rust compiler in `reference-baseline.lock` is immutable. A source
accepted by its type checker is not an executable result oracle unless its
generated LLVM IR also compiles and runs. These entries retain the failing
phase; they do not count as reference-equivalent cases or complete a milestone.

## Result try block: invalid LLVM error-field value

`source_codegen_gate.py` case `try-result` constructs a named
`Result<i64,i64>`, extracts its successful payload using `?` inside `try`, and
returns the resulting value. The independently specified result is 42.
The SG frontend/MIR/native path emits deterministic objects and executes 42.
The frozen reference's source-to-LLVM command exits zero, but clang rejects an
`insertvalue` of a pointer into the i64 error field:
`defined with type 'ptr' but expected 'i64'`.

The gate requires exactly that failed reference link, retains its stderr and
LLVM hash, and marks `result_equal=false` and `reference_link_gap=true`.
An unexpected reference success or a different failure fails the gate. The
adjacent Option try/success, Result propagation/failure and Option-none cases
still require both native executables to run and match. The original failure
is preserved in `target/selfhost/m4-source-cleanup-checked/`.
