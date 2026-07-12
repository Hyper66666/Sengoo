## Context

Most numeric syntax and LLVM-text behavior now exists, while the active tasks
still combine language semantics, cross-target evidence, and complete Cranelift
parity. Requiring two production backends before documenting basic integer
semantics would duplicate work and delay the default path.

## Decisions

### Decision 1: LLVM-text is the production semantic reference

For this change, LLVM-text plus clang is the production backend. It must support
all documented numeric types and behavior on every supported release target.
The Cranelift fast JIT is experimental: it may support a primitive subset, must
reject unsupported programs explicitly, and does not block archive unless it
miscompiles a program it accepts.

### Decision 2: Pointer-sized types follow the target, not the host

`isize`/`usize` width comes from the selected target triple. Parsing, range
checking, MIR types, LLVM layout, casts, and diagnostics use that target width.
Cross-compiling a 32-bit target on a 64-bit host must not retain host limits.

### Decision 3: Cast and checked conversion are separate contracts

`as` is infallible and follows documented truncation, sign-extension, zero-
extension, and float conversion behavior. The v1 checked public API is
`checked_<source>_to_<target>(value) -> Result<Target, i64>`. Success returns the
converted target value; magnitude overflow returns `STATUS_OVERFLOW`, while a
negative-to-unsigned sign violation returns `STATUS_INVALID_ARGUMENT` in
`Result.error`. Checked conversion never silently truncates.
`From`/`Into` is implemented only for lossless conversions. Generic
`TryFrom`/`TryInto` is outside this change.

### Decision 4: Build mode controls implicit overflow only

Debug `+ - *` traps; release wraps modulo width. Explicit wrapping, checked, and
saturating methods are build-mode independent. Integer division/remainder by
zero always traps with a stable diagnostic on the production backend.

### Decision 5: Float behavior follows IEEE-754 with documented formatting

`f32`/`f64` arithmetic and predicates follow IEEE-754. Conversion edge cases
(NaN, infinities, out-of-range float-to-int) are documented and tested rather
than delegated to unspecified backend behavior.

## Archive gate

- production backend covers every integer width, signedness, pointer width,
  overflow mode, division-by-zero, literal form, and conversion family;
- at least one 32-bit and one 64-bit target compile matrix is exercised;
- user-facing documentation is complete;
- experimental Cranelift acceptance/rejection behavior is tested separately.
