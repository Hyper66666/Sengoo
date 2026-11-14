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

## Array loop with break: missing LLVM terminator

Case `array-for` sums 20 and 22, then breaks when its third element is 99.
SG executes 42. The frozen reference emits LLVM successfully, but clang reports
`expected instruction opcode` at the closing brace of a function containing an
unterminated block. The exact failed link is retained and marked unequal.
Case `array-for-simple` independently requires the simple array loop to execute
through both compilers. Original failure:
`target/selfhost/m4-source-hir3-arrays-strings/`.

## Destructured array-loop binding: undeclared variable

Case `array-tuple-for` binds `(a,b)` from two tuple elements, sums their fields
and subtracts three. SG executes 42. The frozen reference rejects the source
with the named undeclared-variable diagnostic for `a`, before writing LLVM.
The gate requires that exact UTF-8 diagnostic and the absence of an IR artifact,
and records `reference_source_gap=true` and `result_equal=false`. The adjacent
`array-tuple-read` case retains a single loop binding and reads `.0/.1`; it must
execute 42 through both compilers. Independent original reference failures are
retained under `target/selfhost/m4-new-reference-development/`.

## User-defined print: builtin selected instead of resolved function

The print gate's `shadow` case declares `def print(x:i64)->i64{x+1}` and
returns `print(41)` from `main`. The resolved user function must return 42
without output. The SG candidate does so; the frozen Windows reference instead
prints `41\r\n` and returns zero. This is a named reference difference, not an output
parity pass. The gate requires both exact outcomes and fails if either changes.
The original observation and input hashes are retained in
`target/selfhost/print-intrinsics-prototype/run07/outputs04/evidence.json`.
The production verification owner is `add-selfhost-print-intrinsics`;
`selfhost/async_factory_print_checks.py` reruns the case against the integrated
compiler. This does not qualify unsupported print argument types or close M6.
