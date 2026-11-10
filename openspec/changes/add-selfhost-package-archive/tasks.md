## Implementation and evidence

- [x] Implement and run a small source-to-native SG tar scan with a C memory probe.
- [x] Cover GNU/ustar/old framing, checksums, numeric overflow and effective sizes.
- [x] Cover GNU long names and local PAX path/size, malformed and duplicate metadata.
- [x] Verify UTF-8, Windows path rules, duplicate and both-direction prefix conflicts.
- [x] Execute independent Python tarfile and pinned Rust tar 0.4.46 observations.
- [x] Add guard pages, invalid extents, exact entry/content/metadata limits and wrong-oracle controls.
- [x] Qualify two fresh SG objects/linked executables, tools, headers and all consumed sources.
- [ ] Implement gzip streaming/chunked 256 MiB cumulative-content integration.
- [ ] Implement staging, manifest identity/checksum validation and atomic package publication.
- [ ] Verify complete package flow and reconcile the parent M7 checklist.

The bounded source-only tar scan is qualified below. Package integration,
256 MiB streaming decompression and filesystem publication remain separate
incomplete obligations.

## Initial executable development proof

`target/selfhost/runtime-tar-proof-02/evidence.json` is a four-command development
run, SHA256 `a1549a70be218763f3a6deee69f651c7a2d66a9fdea999a108070b1ece740858`,
on Windows-11-10.0.26300-SP0 at actual start/end revision
`f8019ce94db24b241071ee9ff0156a8b4a120e69`. It executed 186 fixture IDs (113
successes/73 rejections), six invalid extents with no read/write/OS comparison,
and unchanged failed-call header canaries. Source/tool guards were empty.
This earlier development run did not yet instrument initialized output bytes;
subsequent probes add that distinct check rather than rewriting its evidence.

The independent `tar-rust-policy-04` report actually built pinned tar 0.4.46
offline and executed all 186 fixtures; its SHA256 is
`ef2e1dd4db19fb0bc7fb811cc88eba1ff78f9df3f7be20c5a04bb59c7e9eafe9`.
Windows read-only parsing disables only the unused Unix xattr extraction
feature, whose archives were absent from the local cache. Every compiled
dependency uses repository lock identities and cached source/archive byte
verification. Earlier offline metadata failures are retained separately.

The fixed-seed Windows cases use actual whole-path CompareStringOrdinal as
the independent oracle. In particular the selected Windows implementation
does not identify Deseret upper/lower, final sigma/uppercase sigma, Turkish
dotless I/ASCII I, Kelvin K/ASCII K or sharp-s/SS. The first development
probe's incorrect Deseret oracle was rejected and retained as a failed report;
it was corrected to actual OS behavior, not forced into a Unicode-casefold
assumption. New source-only safety rejections and parser-format acceptance
differences remain distinct from existing sgpm resolver policy.

The expanded development proof
`target/selfhost/runtime-tar-proof-07/evidence.json`, SHA256
`fd07a1d83e00f0419225ecbe9fbb8c4edfdb2d367d1303f27b91430069b09d6b`,
ran twelve commands at actual start/end revision
`e4f30e28dea25211ed1aadd143dd8c88645feb3c`, same Windows host. Each
instrumented and production executable ran all 210 fixtures three times
(plain, head guard and tail guard): 630 calls, 369 accepted/261 rejected,
420 guard calls, plus six no-access invalid extents. The initialized-output
bitmap negative control actually rejected an unwritten-byte read; the
incorrect expected-result control actually rejected fixture zero. Every
failed call retained all original 32 header bytes. Input was read-only in
both guarded placements; table and path outputs had independent guard pages.

Exact positive/one-over boundaries cover 10,000 entries, 1 MiB per extension,
16 MiB aggregate metadata and 65,535-byte paths. Separate actual memory
extents prove 256 MiB file content, content plus one rejection, and a 512 MiB
raw tar input. Payload skipping is allowed; this does not claim every input
byte was read. All source/tool guards were empty. The independent source-bound
Rust `tar-rust-policy-06` run executed the same corpus and all 123 accepted SG
cases have equal normalized paths, effective sizes, offsets and content.
Its remaining accepted-parser versus rejected-package-profile cases are
listed individually in the main proof's comparison table, not called parity.

The earlier development duplicate loop completed but performed 7.85 billion
instrumented reads. The final producer-only optimization uses already-written
bounded lengths/contiguous paths plus an ASCII first-byte filter, retaining
the actual Windows comparison for relevant names; its full read count is
970,587,330. No arbitrary caller table or type/layout authority is inferred.

## Formal bounded scanner qualification

The exact current source qualification is
`target/selfhost/runtime-tar-formal-05/evidence.json`, SHA256
`b369fbc4ce5bed3ea58eb264d2f96eb4eabe6b5b97a99da6f072f1421151b677`.
It actually executed 57 commands on Windows-11-10.0.26300-SP0, at identical
start/end full revision `f91b98639ba67d800a83468d45c89356b808e911`.
All 3,012 source, 804 external-source and 121 tool fingerprints were unchanged;
the report binds 145 terminal artifacts. This consumes the actual historical
gzip/native/formatter/linker qualification branch rather than rebuilding or
silently requalifying unrelated compiler sources. Clang's actual selected
LLD path and every archive read in its verbose link trace are bound, including
CRT archives used by independent C test executables. Actual C headers are
captured before compilation and rechecked against emitted dependency files.

The independently regenerated corpus has 216 fixtures, SHA256
`0f278172b121eadb991deba1789dcf30ad2502e216bc0a776aa941776b6b464e`.
Each instrumented and production executable completes 648 calls, with 432
head/tail guard placements, 369 successes, 279 rejections, six invalid extents
and unchanged failed-call header bytes. An FNV digest of every completed
fixture ID/result is checked against the manifest. Wrong-result and
uninitialized-output controls actually exit one for their expected reasons.
Instrumented reads/writes are 970,608,396 / 3,823,983; production reports these
unobserved counters as null. The three large actual-extent cases retain the
256 MiB content and 512 MiB raw-input scope above, not an all-byte-read claim.

The source-bound independent Rust report is
`target/selfhost/tar-rust-policy-07/evidence.json`, SHA256
`af2a74f9813291ee943d0f03d67e93cedbea0686962050d54a283a9e82f72969`.
Its ten actual commands execute the same corpus. All 123 accepted SG fixtures
match normalized paths, effective sizes, offsets and content. The main report
names 71 Rust tar parser acceptance versus SG package-profile rejections,
including strict framing/PAX, caller capacities, type policy, Windows safety
and normalized duplicate/prefix conflicts; these are not described as full
resolver acceptance parity. Windows comparisons include both-order ancestor
conflicts for actual OS-equal scalars with different UTF-8 widths (Ⱥ/ⱥ and
Ɫ/ɫ), in addition to named Greek/Turkish/Deseret and seeded full-path pairs.

Both fresh directories have identical SG tar/smoke objects, normalized leaf
fixture, timestamp-only normalized non-leaf SDK object, actual provider DLL,
import library and SG-linked PE bytes. Eleven fixed smoke fixtures compare
table/path bytes in SG, including a real non-ASCII OS comparison. Both cold
executions have empty PATH, empty stdout/stderr and actual exit 42. C supplies
only memory/OS operations and constant test data; no C tar/path/checksum parser
implements the algorithm.

The DLL retains its real .pdata/.xdata. Independent inspection, a live LLDB
backtrace after `sub rsp,56`, and RtlLookupFunctionEntry all verify the actual
non-leaf exported range RVA 4,288..4,377 and unwind RVA 8,496. Actual loaded
Kernel32 CompareStringOrdinal and NTDLL lookup provider file hashes/RVAs are
recorded. The raw SDK object time is preserved in evidence and only COFF
header bytes 4..7 are zeroed; every other byte, including all unwind, symbol
auxiliaries and relocations, is proved identical. Raw object equality is not
claimed. The stack-free fixture removes metadata only after complete section,
symbol/address and relocation equality checks.

Earlier formal runs are retained as failed development evidence: dependency
target parsing, archive-member physical-path tracing, a raw timestamp byte
inequality, and inappropriate use of a leaf-only COFF reader on non-leaf
unwind relocations. Their successful algorithm probes are not rewritten into
new-source qualifications. All 18 owned files pass a direct trailing-space
and exact single-final-newline inspection. OpenSpec child and whole-tree
strict validation pass (108 items at the recorded check).

The next integration uses genuine bounded continuation state (32 KiB history,
bit/Huffman/header/CRC state in SG), rather than allocating a 512 MiB package
buffer by widening the old decoder interfaces. Existing raw/gzip exports and
their historical qualifications remain unchanged by this scanner checkpoint.
