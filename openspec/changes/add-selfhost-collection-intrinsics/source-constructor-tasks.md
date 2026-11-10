# Bounded fresh source constructor integration

- [x] Qualify fresh source authority, owning HIR6 and concrete constructor/Drop
  job selection through the actual development provider.
- [x] Run the independent native i64/bool/owning move and Drop oracle, repeated
  object emission and first-call success/failure resource checks.
- [x] Run wrong-pin/mode/limit/name/source-shape rejection controls and the
  existing MIR, projection, disarm and source/native regression gates.
- [x] Audit source/tool/artifact/receipt identities, record actual host/revision
  and validate all affected specs before committing this checkpoint.
- [ ] Complete arbitrary source bodies, all collection operations/layouts,
  final SG runtime replacement and compiler fixed points in the umbrella gates.

## Qualified bounded entry

`target/selfhost/m3-source-constructor-qualified4/evidence.json`, SHA256
`592a834746b0ae8deec17476401cc9b2bbd0027a553fc848d6b06df6f762564e`,
passes 38 commands on Windows-11-10.0.26300-SP0. Its UTC interval is
2026-11-10T03:42:06.034919+00:00 through
2026-11-10T03:54:35.086212+00:00. The actual starting revision is
`8ce3e0708a5fc8e62fd696ca4d0eb40414a31832`; the ending revision is
`104064cd3e58820d9ad7048490fe2663312d7c9d`. Unrelated staged work advanced
the branch during qualification; this is not a clean-tree release rerun.
All source, checkout, snapshot, tool, configuration and artifact change arrays
are empty. The 2,250 recorded sources, 236 original producer paths, 236 owned
copies, eleven tools and six artifacts are bound.

Three independent source-to-object processes emit identical COFF bytes. The
independent native caller observes four actual Vec constructors, i64/bool
element preservation and source clearing, callback reuse and 50,000 exactly-once
nominal owning Drop calls over 1,000 rounds, including a separate Drop observer.
Every round recovers opaque handles and releases its real allocated resources.
Twenty-three rejected inputs and three successes each restore String, Buffer,
opaque and FileSnapshot handles/bytes to their first-call values. Rejections
produce no object, including NUL names, direct/nested/array-contained classes
and enum elements. The ordinary lowering entry is not enabled implicitly.

The previous full integration run is
`target/selfhost/m3-source-constructor-qualified3/evidence.json`, SHA256
`ee6d2bda3017239c0b529ed739464ec423e8ad0003617833ca46e833746b7d8d`.
Its 37 main commands and the four ordinary regression children all passed.
The follow-up changes only the separate source entry/profile and its harness
and ledger. It compares 228 unchanged producer/harness/runtime/stdlib inputs
and every tool identity before reusing those immutable children, and validates
their recorded inputs, artifacts and exit results again:

- MIR: four commands, report `6e3a1f8a0687070a7452b4321f7dcdf52ffa4a772956daa3bb9afd625b09569a`.
- Projection: 22 commands, report `77b8c79efa31792d50ac419419fabfb37981c65a77268b7b3d71232debc1178d`.
- Disarm: 19 commands, report `bbc7a945148ffdad796b3fb154266e7e9510384e8488ee787b15d087bcb0782f`, preserving the named reference zero-Drop policy difference.
- Source/native: 346 commands and 54 cases, report `fabb09df760e506b96c8c0977ef5b50be3003bcd07e2f976d5f7c6174965b8dc`. The frozen reference retains its `array-for`/`try-result` LLVM-link failures and `array-tuple-for` source failure; these are not claimed equal.

Root rehashes 4,579 records and 429 commands across the new entry and those
children. It verifies 162 producer-bound receipts and separately retains the
692 source/native logs as terminal observations: that older child schema has
no per-stream producer hash. The limitation is explicit, not repaired by
inventing historical hashes.

The old experimental entry admitted a plain derived-class constructor before
its inherited Drop chain had this checkpoint's proof. A retained private
old/new control observes old result zero and new error 5008. The new recursive
shape barrier is tested from real typed source. This is a conservative profile
boundary, not a claim that the private test executed a bad destructor. Earlier
failed test-build trials and the incomplete module-prefix/ownership-suffix
comparison remain separate retained development evidence.

No public source optional-borrow/push path, complete compiler closure, final SG
collection runtime, self-building fixed point or performance cutover is
qualified by this constructor entry. Those umbrella tasks remain open.

The follow-up `target/selfhost/m3-constructor-text-nul2/evidence.json` changes
only the test's embedded-NUL spelling to Buffer append/push-u8/strict-UTF8
construction. The locked formatter had inserted a literal zero byte into its
char literal; the replacement keeps the test file reviewable UTF-8 text. A
fresh test executable again rejects the NUL name with 5008, restores all five
first-call resource counts, and emits a positive object byte-identical to the
qualified source-0 object. The earlier attempted integer-to-char cast is a
retained failed build; the compiler correctly rejects that unsupported cast.
