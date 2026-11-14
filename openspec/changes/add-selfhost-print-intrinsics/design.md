# Design

## Admission

Authorize compiler builtin IDs 1, 2 and 3 only after validating the unique
synthetic builtin binding: kind 13, root scope, no source declaration or owner,
compiler flag 4096, exact builtin name/code pair, and the synthetic variadic
language declaration returning unit. Validate direct identifier syntax against
source bytes and the complete freshly parsed AST, the call/callee resolution
pair, fixed concrete language callable signature, argument type and unit result.
Ordinary functions and methods with matching spelling retain ordinary dispatch.
Tests also change callable, callee and argument types together, and swap the
name/code/resolution fields of two builtins together. Literal token kinds and
resolved identifier declarations must remain consistent with typed facts;
self-consistent mutable rows alone cannot change the source call's meaning.
HIR validation and lowering both enforce these facts. This derives authorization
from existing HIR fields; it does not reuse the unrelated six-word borrowed
String pointer-bridge record or silently change its wire meaning.

## Lowering and ABI

An admitted call has language convention zero and one argument. Its runtime
declaration has external C convention one, fixed arity, matching physical
argument type, and unit result. Signed i64 calls use sengoo_print_i64 or
sengoo_eprint_i64. Shared str calls use sengoo_print_str or sengoo_eprint_str.
Do not repurpose the language signature as a C signature or convert arbitrary
integer values to safe references. Reserve these runtime symbols against source
definitions or incompatible declarations; reuse only an identical external
declaration. Generated instructions retain the call source span.

Both print and println currently append a newline in the frozen reference.
str ABI is a NUL-terminated pointer, so embedded NUL truncation is measured and
preserved for this ABI; the prototype does not claim length-preserving text I/O.
Windows redirected stdout/stderr bytes, Unicode UTF-8 and integer boundaries
must be compared with the frozen reference. Owned String, bool/floats, aggregate
Display/Debug and format are outside this first prototype and fail explicitly.

## Evidence and limits

Keep compiler snapshots, runtime source, executables, commands and log hashes
bound. Preserve the original native01 failures and independent instrument output.
Run called and uncalled print bodies, real std::option+main42, same-name shadows,
wrong types/arity, source/AST and HIR binding/signature mutations. Pure HIR
validation must reject forged authorization without rerunning type checking.
Do not skip uncalled bodies to obtain an import success. A later failure in
another stdlib function remains a real frontier, not a passed file import gate.

The frozen reference has a measured same-name shadow difference: a user
`def print(x:i64)->i64 { x + 1 }` is incorrectly dispatched as the builtin by
that reference. The SG candidate keeps the resolved user function and returns
42 without printing for `print(41)`. Preserve this as an explicit reference
difference, not an equivalence pass or a reason to copy the reference's dispatch.
