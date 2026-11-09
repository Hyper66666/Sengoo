# Tasks

- [x] Reproduce nonempty managed-copy loss against the actual pre-fix runtime and wrapper, with an independent C observation.
- [x] Implement the handle-based non-clearing runtime operation and route the managed wrapper through it.
- [x] Add the exact fixed-signature non-retaining input proof without permitting raw projected outputs.
- [x] Verify the four C runtime scenarios over repeated resource scopes and the real SG wrapper against actual registered storage.
- [x] Check invalid/closed handles, capacity failure and successful used_len updates without weakening source-error assertions.
- [x] Record full revision, host, input/tool hashes and a failing old-wrapper control; validate this change and all OpenSpec items strictly.

This child fix is implemented and verified. Full M3 and SDK admission remain open.


Target-only provider proof: target/selfhost/ffi-last-error-managed-copy/target-evidence.json,
SHA-256 deedb57ba2f8c624d7d225bb5860c85ff93b6a8fabf70a1c1cec1ca2b1ea60bc,
full start/end revision 3e70fd54feb5ff0e6c3f4144889b6a7845000ec3,
Windows 11 AMD64. Five C compile/link commands and five executions pass:
the old wrapper control returns 73 and independently reports original
code -2002 / length 23 becoming code 0 / length 0 / copied 0; each of four
candidate cases runs one hundred scopes with exact registered-resource
restoration. The candidate preserves exact bytes/code, resets empty used_len,
retains storage on a capacity error and rejects invalid/closed handles.
Production C/SG sources have not yet been changed, so implementation and SG
wrapper validation tasks remain open.


A separate target public-wrapper development probe uses the exact SG source
importing std::ffi, a verified compiler's LLVM output and actual registered C
storage. Its report target/selfhost/ffi-last-error-managed-copy/public-development-evidence.json
has SHA-256 e80e8a50063d8a117024e3b12aa75cb2e25b0f6d0361da0c2f52e5a05934196e.
The old wrapper returns 5 after observing copied=0; the target replacement
returns 42 over one hundred scopes, with independent C verification of the
23 diagnostic bytes, code, used_len and zero live resources after explicit
Buffer release. This development probe does not substitute for a final
unchanged-input production wrapper gate or claim to repair legacy automatic
Drop behavior in the frozen compiler.


## Final production evidence

The durable runtime/public-wrapper gate is
tools/stdlib/tests/verify_ffi_last_error_copy.py. Report:
target/selfhost/ffi-last-error-managed-copy/checkpoint/evidence.json,
SHA-256 cc617251aed234a4e9e9d3300b682984f9567262daa92fc212087ac283ee29dc.
It records full start/end revision
d5144a0fbce78d9d5fc01b9b72195604131fb852, Windows 11,
Python 3.12.14, fifteen passing commands and empty source/snapshot/tool
changes. The inventory includes the complete declared stdlib, Rust runtime,
Cargo locks/toolchain, both borrowing source/test files, the C probe and SG
fixture. Seed and clang identities are checked before execution. An independent
wrong-seed run fails before tool invocation in identity-negative/evidence.json.

The actual C provider has four one-hundred-round scenarios: preserve nonempty
error code/bytes and set used_len, clear used_len for an empty error, retain
storage and used_len on insufficient capacity, and reject null/closed handles.
The old wrapper control must return 73 with the exact diagnostic loss
before=-2002/23 after=0/0 copied=0. The real public SG wrapper is compiled to LLVM
by the verified compiler and linked against freshly compiled actual runtime C.
Its saved old-wrapper snapshot returns 5; the fixed wrapper returns 42 over
one hundred registered Buffer scopes. C independently validates exact bytes,
error state, initialized length, untouched tail bytes and live resource counts.
The SG fixture explicitly releases its Buffer; this child does not claim to
repair the frozen compiler's separate automatic-cleanup differences.

The new input-only ownership proof is verified by the full frontend report
target/selfhost/ffi-last-error-managed-copy/frontend-checkpoint/evidence.json,
SHA-256 5c042602582e331e32b61ca41d0e7b529cf1c2eda86bc9e4a6a5a16b2d2d6a93.
It records the same full revision on Windows 11 AMD64, four successful
build/native commands and empty source/tool changes. Fifty resource rounds
include a genuine exact prototype, source-name shadow and wrong-arity cases,
plus signature variadic-flag poison/restore checks. All previous HIR4 roundtrip,
poison, ownership and raw MIR validation/resource regressions remain enabled.
No raw-pointer-output exception, ordinary FFI signature change or SDK
authorization is introduced.
