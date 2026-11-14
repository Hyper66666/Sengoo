# Checked AsyncFile factory source contracts

Status: implementation plan accepted; the retained-source snapshot foundation
is committed. Private source, HIR and resource controls are being integrated;
production qualification and milestone completion are not yet claimed.

The first current compiler self-input stops at `tools/stdlib/file.sg` bytes
6174..6327. `AsyncFile.wait_readable` has the supported language signature
`fn(&AsyncFile, i64) -> Future<FileReadinessOutcome>` but its provider has the
physical C signature `fn(i64, i64) -> i64`. SG correctly refuses an ordinary
Future return and an integer-to-Future conversion. There is no checked fact
authorizing this particular runtime factory.

The reference's acceptance is insufficient authority: its implementation-method
checker records signatures without checking ordinary method bodies, and its MIR
uses the resolved spelling `AsyncFile_wait_readable` to attach a runtime origin.
The executable evidence includes a forged method returning 99 that emits valid
LLVM which passes 99 to the unsafe readiness poller. Another unchecked method
declares bool and returns i64; clang rejects the generated IR. Preserve these as
named soundness differences, not compatibility requirements.

Add one explicit, bounded factory contract that separates source identity,
language signature, raw provider ABI, checked wrapper body and owned Future
origin. Retain general escape and mismatch diagnostics. All imported method
bodies continue to be checked. This child owns semantic/HIR admission and its
fail-closed lowering boundary. `add-selfhost-native-sdk` continues to own
provider artifact/lifecycle qualification; an independent follow-on child must
implement native async consumption if this first child does not do so.

A compiler that only imports this valid factory may advance past semantic
analysis. A reachable invocation must still fail with a located unsupported
diagnostic until the complete native lowering/cleanup contract exists. Neither
that advance nor the reference native executions prove SG native async support,
M6 fixed point, M7 compatibility, or a Rust-free provider.

Dependencies: `add-selfhost-semantics`, its intrinsic source/ABI facts and the
versioned HIR bridge (this child migrates schema six to seven);
`add-selfhost-native-sdk` entries 11..15; module source-origin validation;
the compiler snapshot stage. Source-selection policy remains in SG. A narrow
read-only query of retained ancestor handles supplies directory case policy;
it does not admit sources or reopen them.
