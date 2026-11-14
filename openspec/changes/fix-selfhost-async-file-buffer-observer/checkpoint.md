# Restricted AsyncFile buffer observer checkpoint

Status: restricted source observation and associated gates passed; no native
AsyncFile language execution or SG runtime-port completion is claimed.

`sh_b_async_file_read_contract` admits only parameter one of the exact bodyless,
fixed extern-C `sengoo_async_file_read_into(i64,i64,i64)->i64` source bridge.
Position zero/two, wrong width/arity/result, SG body, retaining calls and raw
pointer escape remain rejected. The formal `observer` mode passes declaration
and eight ownership-use controls; actual canonical std::file analysis also
passes with this correction in the integrated factory graph.

The implementation's source bridge pointer is i64; the real provider's C ABI
uses a mutable byte pointer. The captured actual Rust provider does not retain
the slice/pointer beyond synchronous `File::read` under its registry mutex.
The rehashed provider dependency is:

- `target/selfhost/native-readiness-provider/qualified04/evidence.json`.
- SHA-256 `77c51f9c587dc8b72959c4b8b6e9cfa286d115a0500e2fffd3bc83dc6a76ab9e`.
- Six commands, sixteen cases with 1,600 measured lifecycle scopes, including
  real five-byte reads, invalid pointer/capacity cases and C ABI checks.
- The current integrated gate rehashes the retained provider inputs/artifacts/
  configurations; it does not rerun the old 1,600-scope experiment.
- Development Rust provider only: final_sdk, runtime_algorithms_ported and
  sg_lowering_qualified remain false.

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

The restricted observer entry, canonical method ownership analysis, wrong
position/declaration/retention controls, actual nonretaining provider pin and
ownership/HIR regression items have bounded evidence above. Provider execution
is the qualified Rust dependency, not SG-native readiness execution. Strict
child/all validation remains a separate precommit check. Existing task
checkboxes stay frozen to preserve report pins.


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
