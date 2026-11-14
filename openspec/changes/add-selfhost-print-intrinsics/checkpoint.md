# Checked i64/shared-str print checkpoint

Status: bounded production implementation and integrated gate passed. Other
printable types, std::file execution and full self-hosting remain open.

The production lowerer recognizes resolved builtin identities for print,
println and eprintln, checks their source/AST/callable facts, and creates distinct
fixed C ABI runtime signatures. Signed i64 and shared str calls pass; bool/u64
remain explicit 5008 refusals. Incompatible or source-defined reserved runtime
symbols reject 5009; wrong arity rejects 5105.

The print subreport is `target/selfhost/async-factory-qualified03/print/evidence.json`,
SHA-256 `d501fc8b71d6db11f8320a3ba55d7be38984e9b03134fc10171f188933c3487c`:

- 392 command receipts; ten cases, twenty-one HIR corruption kinds across three
  scopes, independent clone/codec and first-scope String/Buffer/opaque recovery.
- Streams, empty strings, uncalled functions and real std::option import execute
  42 with byte-identical stdout/stderr against the frozen reference. Streams
  include Unicode/emoji, embedded NUL and signed-i64 extrema. Existing ABI NUL
  truncation and newline behavior are preserved, not generalized formatting.
- Each of the five rejected forms preserves a distinct preexisting output file
  byte-for-byte. Repeated successful emissions produce equal object bytes.
- A user `print(x:i64)->i64{x+1}` runs as the resolved user function, yielding
  42 without output. The frozen reference prints 41 and returns zero. This is a
  named reference difference retained in `selfhost/REFERENCE_GAPS.md`.
- Forty-nine ordinary native programs execute 42; forty-six match the frozen
  reference, with two exact reference link failures and one exact source
  failure retained separately. Failed reference outputs remain absent.
- Actual runtime/runtime_string C header closures, selected linker and six
  actual linked library identities are bound to prebuild inputs and receipts.

The initial formal02 MIR negative exposed diagnostic preemption: print facts
ran before generic call validation. The final implementation checks generic
calls first, restoring the existing missing-signature 5503 without changing
fixture expectations; all print-specific checks still run afterward. The
isolated causal/fix report is `target/selfhost/mir-print-order-diagnosis03/evidence.json`,
SHA-256 `35226b22431ca683a2be78518e8de8ef0e94986f186d6e8b34e062e290bdba71`.
Formal03 then passed the complete original MIR fixture first. Failed formal02
and interrupted formal01 remain separate historical evidence.

## Bound production evidence

The integrated gate captured production bytes at full revision
`0e112a64d393f219a8f9669f24f8728de8f3aff8` with the factory/print working-tree changes.
The checkout was not identical to that commit. Host: `Windows-11-10.0.26300-SP0`.
Run: `2026-11-11T05:56:48.785968+00:00` through `2026-11-11T06:36:54.285633+00:00`.

- Report: `target/selfhost/async-factory-qualified03/evidence.json`.
- SHA-256: `09aeacc234089da105bef9db07e69b38de8a47aadfef353210a8189ef8c6fe4f`.
- Result: `passed-bounded-contracts-with-recorded-native-frontier`; 469 terminal command receipts.
- Current independent audit: `target/selfhost/async-factory-qualified03-durable-audit.json`,
  SHA-256 `71e5a50f8724e54a6b32f9b19dff79a4ec9063c9b5b3ecf9ce8242e626fc775f`.
  On 2026-11-14 it rehashed 5,167 files, 20 configuration candidates (including
  absence), 469 coordinator and 454 nested command receipts. No recorded input,
  producer origin, tool, header, library, receipt or artifact drift was found.
  Historical dependency checkout origins are not a new producer qualification.
- Seed and frozen reference: `target/selfhost/seed-reference/release/sgc.exe`,
  SHA-256 `c07e02be19ebb82a33927851dab6224f444477f0b8dc0aab93f1c83adbdde279`.
- HIR production source: `selfhost/src/hir.sg`, SHA-256
  `6d30a8a03ad613adf739f9124f051df1d71892f1201028e54fb0dc800ed5515f`.
- Verifier: `selfhost/verify_async_factory.py`, SHA-256
  `bd7e5b3382b61017d7872e50dac5efe91d6c03f230b34e7cd13c7e05b2ea5733`.

The gate runs `sgc --runtime-mode source-development build` with captured
stdlib/runtime/module-map inputs and forced rebuilding at optimization zero.
Its `joint-build` compiles the dedicated test wrapper; ordinary native commands
still enter the captured production native driver. The full self-input uses a
strictly derived test-only stage-observation wrapper whose original driver,
lift and lower bodies differ only in names and stderr markers. It preserves
all production checks; marker equality is checked against the ordinary
`std::file` failure. All command arguments and stream hashes are in the report.

Existing semantic, module, borrow, typed MIR, HIR-origin and full body-coverage
fixtures passed against this producer. MIR includes ten roundtrip/lowering/
cleanup and thirty CFG/SSA/ABI resource rounds. The full body-coverage fixture
uses the rehashed qualified collection provider; the historical dedicated
body-coverage runner was not relabeled as current evidence.

## Remaining native frontiers

The real `std::file` import plus main42 passes semantics, then fails lowering
5008 at canonical `string.sg` bytes 17287..17536 (`Option<String>::unwrap_or`).
No object is emitted. The current full compiler self-input returns 65 with
lowering code 5501 after 1,671.438 seconds; its last stage marker is `hir-lift`.
It completes module loading, semantic analysis and ownership analysis before
that rejection. The reported option.sg 0..0 site is the bounded rejection's
reported location, not proof that Option syntax caused it. No self-input object
is emitted. The previous private 1,800-second timeout is separately retained.

M3/M6/M7, native AsyncFile execution, the final SG runtime and full self-build
remain open. The current source-flow qualification is a separate gate; this
checkpoint must not turn its interrupted or unfinished report into a pass.

## Task completion mapping

Independent identity/runtime-call implementation, exact outputs, mutation
refusals, same-name user behavior, next real frontier, integrated semantic/HIR/
MIR/ownership and ordinary native cases, first-call resource recovery and prior
output preservation have named evidence above. The complete source-flow/native
integration and child/all strict precommit checks remain separate pending
items. Exact HIR codec bounds are checked by the joint factory/body coverage
fixtures; no full formatting or native AsyncFile capability is implied. Existing
task checkboxes stay frozen to preserve report pins.


## Current source-flow and precommit integration

The current HIR7 source-flow gate completed on Windows x64 on 2026-11-14,
08:09:26 through 08:23:52 UTC, capturing revision
`0e112a64d393f219a8f9669f24f8728de8f3aff8` plus the current working-tree changes.
Report `target/selfhost/async-factory-flow-qualified03/evidence.json`, SHA-256
`e8dcb19a66995b55a7c1d2fb5d8fdb612c3f78b07bc176e0438a2172b5e7014d`,
passed 41 commands, 22 source rejections, seven functions with 7,000 real native
calls, 25 first-call resource scopes and three cleanup scopes. Three objects
were byte identical (SHA-256
`e98e43c4b7f4ab97807b6c8be2cc48b9fecf51784dcac1dcc13d3f68a094b4f6`).
The independent audit at
`target/selfhost/async-factory-flow-qualified03/independent-audit.json`, SHA-256
`e3f0c45ed9f0a52a6c28bdd880eeddf40fe88e756377b1a4d610b9c3236d9b6f`,
rechecked 3,026 bindings and fourteen configuration candidates. All 58 production
compiler dependencies match the integrated factory gate. Ordinary execution
parity uses that gate's 49-program subreport; three named reference failures
remain explicit. The legacy flow runner's library hashes are post-run binding,
not a claim of prebuild capture; its old HIR6 prose does not change the captured
HIR7 source identity. Interrupted earlier flow runs remain unqualified.

All four affected child changes and all 117 OpenSpec items passed strict
validation. Ledger `target/selfhost/async-factory-precommit-strict01/evidence.json`,
SHA-256 `df8271849740346a47562ee737ad61559fbfcf51da7d29f47300cf7d3f02de9d`.
This adds the current source-flow integration evidence; native AsyncFile
execution, conditional field cleanup, full self-build and M3-M8 remain open.
Captured normative/task files have not been rewritten after the gate.
