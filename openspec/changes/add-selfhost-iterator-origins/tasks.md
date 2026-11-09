# Independent stage-one implementation

- [x] Implement caller-pinned canonical module/prototype admission and the real Vec constructor plus three iter factories.
- [x] Implement bounded whole-move/reborrow/inert/unknown origin flow, conditional parameter obligations and returned backing relations.
- [x] Reject new origin eligibility for forged cross-T reconstruction, unknown C handles, empty caller origin and source shadows while preserving ordinary typing.
- [x] Execute real source fixtures, owned/non-Copy T, zero placeholders, read-only type/producer/source poisons and exact resource checks.
- [x] Freeze source/tool-bound evidence and validate this child/all specs strictly.

## Later integration, not claimed by this stage

- [ ] Implement general CFG/join and recursive summary obligations, alias/mutation invalidation and full returned-loan lifetime checking.
- [ ] Review and integrate ShChecked/module facts, HIR6 codec and verified borrowed-next MIR/native bridge.
- [ ] Execute actual native backing lifetime/Drop gates and remeasure current compiler closure; do not infer completion from the old typed-reference 75 report.


## Executed independent stage-one checkpoint

The final source-bound report is
`target/selfhost/m3-iterator-origins/checkpoint/evidence.json`, SHA-256
`8ff8d7b35671f230bbb9650a20eb130c0dfa858eddcec03f96e1603c8c7304e1`.
It records actual full revision interval
`836777a0fd1e7083d59d80af8053d33691959e63` through
`11f4cb75a0eac9baac4e1209bb8607215b104dd4`, Windows
`Windows-11-10.0.26300-SP0`, Python 3.12.14, and UTC interval
`2026-11-09T23:04:34.228865+00:00` through
`2026-11-09T23:10:58.639389+00:00`. Eight commands passed; 182 source inputs,
130 immutable producer snapshots and the selected seed/clang/real Rust tools
were unchanged. Source, snapshot, producer-checkout, tool and Cargo configuration
change arrays are all empty. Concurrent commits are retained as their real
revision interval, not rewritten as a single starting/ending commit.

The real pinned collections module supplies the canonical Vec constructor and
all three actual generic/i64/bool iterator factory declarations and bodies.
The ordinary typed program remains accepted when a RawVecIter<Owner> handle is
rewrapped into VecIter<i64>, or an unknown C handle is put in a Vec record;
those values receive no new concrete origin. Whole iterator moves, reborrows,
i64 constructor/iterator flow, generic and owned-T iterator origins, nested
forwarding and eight zero placeholders execute source assertions. The bool
factory is admitted, but a separate positive bool caller query is not claimed
by this first fixture; it is a named first addition for the following loan phase.

Conditional parameters remain state two, never unconditional Good. Scalar
return functions and functions with two origin parameters but one returned
origin both discharge all obligations at actual calls. Missing caller origins
fail eligibility; a conditional actual transfers a conditional caller obligation.
A concrete-return/multiple-conditional relation that is not represented remains
unknown/partial. An iterator returned from a dead local backing has no return
authority. Branch, loop and raw-handle-to-unknown-C boundaries conservatively
remove subsequent eligibility, with actual source assertions.

The main source and each of constructor/type shadow sources run five exact
first-call resource scopes: fifteen total, comparing String, Buffer, opaque,
file-snapshot handle and snapshot-byte counts after every scope with no warmup.
No existing semantic/borrow/HIR/MIR/native/runtime/stdlib source was modified.
The old stage-zero compiler actually builds the SG origin layer and native tests.

Pure-origin replay (not parsing or Sh_check) rejects altered factory rows,
concrete state, source/module binding, source bytes/length, type words/node
facts, obligations, a cleared node-origin table and nonempty binding workspace.
Original type words and argument words remain byte-for-byte identical, in
addition to count checks. A tiny limit fails 5700; record caps bound call,
obligation and diagnostic growth. Invalid identity/tool controls are rejected
before executing the selected binary. The stale-passed report is replaced by
failed with zero tool steps; its expected negative report is
`target/selfhost/m3-iterator-origins/identity-negative/evidence.json`, SHA-256
`f8f47e4bbe2d8ca2aac41c7e3ad2e83fb3baef56fd7c8e4960f6eeb044af9295`.

The earlier `development1` through `development4` failures remain preserved;
they are not substituted for final evidence. They exposed seed scalar Vec API
ambiguity, borrowed temporary implementation issues, the incorrect returned-role
parameter obligation and an omitted exact source-length check. `development5`
is a prior complete pass (SHA-256
`733bafc57bc0e6c718fe47dd54e0f9a1cc5362b5f40a3b736e08ab602ac8d459`),
separate from the final additional loop/unknown-call/clear-table/cap controls.
The post-run tasks evidence ledger is deliberately excluded from producer guards;
proposal, normative delta and design remain bound. Strict child and all OpenSpec
validation passed: 106 items, zero failed.

## Scope and next owned interface

The admitted `vec_new<T>` source body is a Vec{0,0} placeholder. A state-one
record on its exact canonical call means a trusted compiler-intrinsic semantic
producer origin, depending on the stage-zero lower_vec_new_call/new_parts
semantics verified by the earlier narrow LLVM/native probe. It is NOT evidence
that the literal body allocated memory. Ordinary Vec{0,0} values remain inert
state three. The future SG constructor HIR/MIR/runtime bridge must separately
verify actual descriptor generation, allocation and backing identity; matching
the placeholder body alone cannot authorize a live reference.

`target/selfhost/m3-iterator-origins/loan-interface.md` gives the exact node,
return, conditional-obligation and call-discharge table fields. All getters
require prior validation with independent caller pins and the actual graph and
checked input. This is independent schema one, not HIR6, not native authorization
and not a claim that the original core 112 diagnostics changed. The following
phase must implement backing-loan/effect checking (including shared-receiver
native mutation and raw handle escape to unknown C) before one coordinated
HIR6/MIR optional-borrow integration. The remaining tasks above stay open;
partial stage-one success does not complete M3 or self-hosting.
