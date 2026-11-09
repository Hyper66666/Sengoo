# Tasks

- [x] Reproduce the seven canonical disarm sites and prove the target shape, HIR5 poison/roundtrip behavior, exact MIR flag trace and actual native resource transfer.
- [x] Verify all four production base hashes, apply the approved bounded action/codec implementation, and preserve existing ownership/ABI facts.
- [x] Require a direct ordered AST block sibling extraction/zero assignment and a single audited free destructor; reject extra payloads, real consuming calls, conditional moves and nonzero writes.
- [x] Add durable fifty-round ownership and twenty-round HIR tests, old-schema rejection, pure-validation poisons and exact MIR op25/store/drop assertions.
- [x] Re-run the complete M3 ownership/HIR/raw-MIR suite and the actual one-hundred-transfer C resource test against production sources.
- [x] Rebuild the current source checker, record core 112/open diagnostics and String zero, and preserve the reference zero-handle destructor-call difference.
- [x] Record the source/tool-bound production reports, full revisions and host; validate this child and all OpenSpec items strictly.

Historical target evidence is not production qualification. Full M3 remains open.


## Production checkpoint evidence

All four approved production-base file SHA-256 values were verified before
applying the strict proposal; the same base bytes were independently verified
from HEAD before freezing this batch. The implementation adds no runtime or
collections changes. Existing builtin_calls six-word and deferred-equality
four-word tables, ownership action eight/nine, raw MIR, and parameter-profile
behavior are preserved; schema four now explicitly rejects with 5502.

The final reports were produced on Windows 11 AMD64 with Python 3.12.14 at full
revision `3eeefa14f2ae6112053c404b753a4d2b468b3f92` before and after execution:

- `target/selfhost/m3-handle-disarm/final-frontend-checkpoint/evidence.json`, SHA-256 `a8f9e957acd3555f6e1214ad3ff92a9007eac3c8ae8bd0aa01489f9f1811aa22`: complete ownership fifty-round suite and HIR/raw-MIR tests passed; four commands, 178 unchanged source inputs, tools unchanged. UTC interval `2026-11-09T21:56:13.019445+00:00` to `2026-11-09T21:57:55.879319+00:00`.
- `target/selfhost/m3-handle-disarm/production-checkpoint/evidence.json`, SHA-256 `d9d9ba79a48c37b4bc2d41b44ef497934a07a985fdf41984634dda446f7e4521`: nineteen commands, 178 producer inputs and 152 immutable snapshots, with source/snapshot/checkout/tool/Cargo-configuration changes all empty. UTC interval `2026-11-09T22:00:14.915328+00:00` to `2026-11-09T22:03:58.174563+00:00`. Twenty resource-exact HIR rounds cover lossless schema-five roundtrip/decoded lowering, old-four rejection, mutability/action/path/conditional/prior-move controls and coordinated foreign-field/resolution plus root/path/node type poisons, without adding type IDs. The MIR trace proves source init once, extraction clear once, zero-site flags zero, one zero store, and source Drop zero.
- That same production report executes one hundred actual runtime.c/runtime_string.c Buffer transfers with all three live registries zero after every scope and exactly one destination release per allocation. Its frozen-reference LLVM control intentionally exits 75 with `created=1 freed=1 source_drops=1 live=0 round=1`: the reference invokes the original source free(0). This is a destruction-call-policy difference, not a leak, and is not claimed as semantic parity. The report result therefore remains `passed-with-reference-zero-drop-policy-difference`.
- The rebuilt source checker reports actual core ownership diagnostics **119 → 112**, and String **0**; the complete current diagnostic ledger is `target/selfhost/m3-handle-disarm/production-checkpoint/actual-core-checking.stdout.log`. This is seven disarm sites, not a claim of seven independent general ownership fixes or a complete compiler closure.
- `target/selfhost/m3-handle-disarm/projection-checkpoint/evidence.json`, SHA-256 `73582356833ca87b160e8c11785cc9172ba7ef518efdc73e8b1e09488c05e537`: existing source/native projection and borrowed runtime-pointer cases all passed candidate/reference execution, three cases times two compilers times one hundred scopes, including two observations per scope. Twenty-two commands, 200 source inputs and tools unchanged.
- `target/selfhost/m3-handle-disarm/identity-negative/evidence.json`, SHA-256 `59146463074b4f3174fbde62a33b7ebe081d51d195784e875b8d5744b9d4e894`: an incorrect executable identity replaces a preexisting passed report with failed, before any selected executable is run; zero tool steps. Failure is the expected negative-control result.
- Parent integration `target/selfhost/m4-hir5-disarm-source-integrated/evidence.json`, SHA-256 `252e59dff12ce4eb6fcdeeeaa6aac9bdf4272990a3ae167f68bdc81c9b200129`: 346 commands, 49 native positives and five negatives; 46 frozen-reference equivalents and the same three explicitly named historical reference gaps. Source/tool/checkout guards unchanged.
- Parent SG-link integration `target/selfhost/m4-hir5-modules-sg-link-qualified/evidence.json`, SHA-256 `d176f8be99a0cb36f7798812e81328f740cebbe8a809ac21230743ed5c29521d`: 83 commands, 819 sources, nineteen tools, ten linked graphs and empty-PATH execution. Source/tool/external/configuration changes empty, with the qualified historical linker chain preserved.

The runner excludes only this post-run evidence ledger from its producer
inputs, avoiding a circular report fingerprint. Proposal, design and normative
delta remain guarded. Cargo bytes are hashed and parsed from one read; missing
configuration files retain explicit absent records.

Earlier evidence remains historical: the first snapshot-path runner failure
and the new generic test's `>=` token-boundary failure were preserved, fixed
and not reused as passes. The first complete production runtime pass used the
older runner and is retained at `final-checkpoint/evidence.json`; only the
read-once/config/ledger-corrected `production-checkpoint` above qualifies the
final runner. Strict child and all OpenSpec validation pass with 103 items,
zero failed. Full M3, remaining core ownership, general origin/SDK admission,
closures/owned patterns/async and final self-build/cutover remain open.
