## Design and target proof

- [x] Identify the actual borrowed iterator intrinsic and explain why existing source scalar/projected summaries cannot authorize it by name.
- [x] Document precise typed receiver, caller-pinned provider, fixed physical signature and per-call observation contracts, including cleanup and SDK/program identity separation.
- [x] Implement target-only admission of a real helper provider and exact checked declaration binding, with concrete VecIter<i64/bool> nominal/field provenance; semantic producer origin remains open below.
- [x] Execute a clear-side-table ownership control and distinguish typed-reference source effects from authority-table effects.
- [x] Execute positive and malformed-source/type/provider/call-fact cases and exact resource checks in the target proof; retain the failed canonical reconstruction rejection as an incomplete requirement.
- [x] Rebuild and measure actual core with the typed proof, preserving the baseline/done/helper counts and remaining diagnostic ledger.
- [x] Record complete source-bound evidence and strict child/all-spec validation.

## Production integration after review

- [ ] Admit the reviewed helper declarations through the native SDK provider map without changing ordinary C declaration rules.
- [ ] Implement checked call facts and read-only validation; preserve real SDK/module/checked binding identities.
- [ ] Integrate call-local observation with source action-eight inference and independent certificate revalidation.
- [ ] Demonstrate that reviewed native effect certificates are required where the behavior depends on them; successful table validation alone is not contract-driven borrow integration.
- [ ] Reject a canonical VecIter<i64> reconstructed from a RawVecIter<Owner> handle using verified semantic producer origin, with explicit stage-zero compatibility.
- [ ] Version HIR provider/call tables, clone and serialize them, and test corruption/old-schema rejection without rerunning semantic analysis.
- [ ] Revalidate concrete MIR calls, preserve the receiver lifetime and original exactly-once owning cleanup, and execute actual native programs.
- [ ] Qualify the selected runtime implementation and resource/lifetime rejection cases before changing production stdlib/runtime/borrow sources.
- [ ] Keep collection algorithm migration, final SDK and full M3/self-hosting tasks open until their independent gates pass.

## Executed partial target evidence

The source-bound run at `target/selfhost/m3-iterator-typed/final-evidence.json`
has SHA256 `b2b15b3c4b094862ee938813bfb5c6d5d6d25fa13f1a9b9b922a7dd36c2487b0`.
It records actual revision interval
`3eeefa14f2ae6112053c404b753a4d2b468b3f92` through
`bb647a7db33f3ff706b72b1d55dcad88b23323ed` on
`Windows-11-10.0.26300-SP0`, with 527 source inputs, unchanged source/tool
hashes, an actual native SG driver build and exactly 21 completed fixture IDs.
Each invocation checks String, Buffer, opaque and file-snapshot handle/byte
counts against its first-call baseline. This is 21 independent cold invocations,
not a claimed repeated 50-round strict-bridge resource test.

`remaining-ledger.json` has SHA256
`ba84b0b9b9c6229ad49e70fc3a3b68f28218d1da5556a76b4b2429ececdda440`.
It independently verifies each fixture's expected error, including the owned
unknown-C case's exact root record `[6101, 0, 235, 240]`. The frozen done-only
target baseline is 119 diagnostics. Typed-reference source rewriting yields
75; clearing both call records before ownership yields the same 75 records,
including every code, physical source and byte span. Therefore the measured
reduction is attributable to the typed-reference rewrite, and the
contract-driven ownership task remains unchecked. This old frozen target is
separate from later production HIR-five/detach counts.

All remaining diagnostics are 6101: seven detach sites, thirteen raw-done sites
after a consuming next call, twelve legacy bool-next sites, fourteen legacy
i64-next sites, five returned-self sites, twelve source-done sites and twelve
source-next sites. HashMapIter's extra backing values storage remains unchanged;
its separate lifecycle is not authorized by a matching handle layout.

The target maps two real C declarations in physical collections module one to
checked runtime bindings 474 and 475, source-wrapper bindings 1632 and 1633,
nominal iterator binding 624 and genuine handle-field binding 1897. These IDs
belong to this checked program only. Its private twelve-word call table and
custom caller-pinned target catalog are not a production native SDK map, the
production six-word String table or production HIR five.

The canonical `RawVecIter<Owner>` to `VecIter<i64>` reconstruction is actually
accepted with zero root diagnostics. Its required rejection is **not passed**,
and the run explicitly records `production_implementation = false` and
`requirements_complete = false`. No production runtime, collections, ownership,
semantic or HIR source is changed by this child. The next review must resolve
semantic producer origin and old stage-zero compatibility before enabling the
bridge; slot width, descriptor flags and non-null move/drop callbacks are not
that proof.

The incorrectly expected move from an explicit shared-reference field is kept
with its original source, stdout and nonzero runner result in
`failed-hypothesis-review.json` (SHA256
`df59bc99c52349170b088bb2b47fdfa2c80bda4d2b2946728f3f6b5bceca2030`).
It is failed historical evidence, not a passing rejection test. The separate
earlier physical helper proof remains a development C runtime dependency; it
does not establish semantic Copy or port collection algorithms to SG.
