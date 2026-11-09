## Raw decoding

- [x] Implement bounded SG bit input and canonical Huffman decoding with exact limit failures.
- [x] Implement stored, fixed and dynamic blocks, repeats, length/distance decoding and overlapping/cross-block copies.
- [x] Keep C adapters limited to memory access and verify every SG access against caller extents.
- [x] Execute independent compressor/decoder comparisons and explicit malformed/truncated/extent controls.
- [x] Qualify actual SG object/PE production, two fresh builds and complete source/tool/host/revision evidence.
- [x] Validate this child and all specs strictly.

## Framing and tool integration

- [x] Extend the single raw decoder with exact prefix consumed counts and requalify all raw/64MiB cases without changing the legacy exact-input entry.
- [x] Implement and qualify gzip framing/checksums and explicit first-member/strict-single/strict-concatenation modes against the actual package decoder's policy.
- [x] Execute pinned independent Rust GzDecoder/MultiGzDecoder policy probes and retain Python/RFC acceptance differences.
- [ ] Integrate tar/package workflows with unchanged public limits and failures.
- [ ] Qualify final SDK packaging, self-building and performance before replacing the reference package path.

Root independently rehashed every source, external input, tool and listed
artifact in the gzip, shared raw-prefix and Rust policy qualifications before
committing this phase. The five executed qualification controls are recorded in
`target/selfhost/gzip-qualification-negative-controls/evidence.json`, SHA256
`1746dd76b3f0e5264840098133ee9e4e060e9f6dad5c02bce4f4018235beb81f`.
Wrong raw source, raw compiler artifact, declared linker-parent pin, toggled
Rust observation and Rust output pin each failed with the expected error and
zero tool commands. These are the five actual mutations tested; they are not
a claim that every conceivable malformed report fails at the same stage.

## Executed raw decoder gate

`target/selfhost/runtime-deflate-final-checkpoint/evidence.json` has SHA256
`7896f7263042359b916d3efe2905da312e8563bf60d554891b229445f708c718`.
Its actual start/end revision is
`efed463dc06be086d97718089e5d4cc1322333e0` on
`Windows-11-10.0.26300-SP0`. All 42 commands met their expected exits;
2,462 consumed source files, 126 external inputs and 45 tool identities were
unchanged. The report discloses the actual concurrent dirty checkout; only
consumed fingerprints determine its guard result. The frozen UTF-8 runner was
not modified. The final DEFLATE runner SHA256 is
`5a253cb13ed7b1e5d3d8baa2a1c49c602c66b43be7d28617bfde77fd6c30bd75`.

Two independently generated corpora each contain 5,290 exact fixture IDs, with
SHA256 `6f6ae1cca20bf43074d80892afdcf6db6e813f22815bb9878fa19b91f1f59728`.
The runner independently parses all records, checks zlib EOF/unused bytes and
expected output, validates capacity fixtures against a genuinely valid decoded
stream, and retains exactly two named HDIST acceptance differences. Coverage
includes 144 compressor level/strategy combinations, 80 random streams, all 30
defined distance codes with cross-block history, 4,668 truncations, 121 capacity
fixtures, 200 mutations, all repeat codes, repeat16 across the literal/distance
split with both zero and nonzero prior lengths, a 15-bit code, reserved symbols,
malformed/incomplete trees and exact trailing-byte rejection.

Each instrumented and production-memory executable performs 10,006 actual
calls, including 4,716 head/tail guard-page calls. Every named malformed-tree,
repeat and distance fixture is included in the explicit guarded-ID inventory.
The instrumented aggregate is 849 successes, 8,974 malformed failures and 183
capacity failures; eight additional invalid-limit calls use unreadable ranges
and prove no input/output adapter access. Input reads are sequential exactly
once; history reads refer only to already written output; writes are sequential
and within capacity. Known valid capacity-failure prefixes match the independent
plain bytes. Production-mode counters and unobserved sequence/prefix claims are
explicitly null/false rather than inherited from instrumentation.

The large gate actually decodes 64 MiB of output, rejects capacity 64 MiB minus
one, and consumes exactly 64 MiB of stored input made of 1,024 blocks whose
decoded bytes follow index modulo 251. Both modes use read-only tail-guarded
input and tail-guarded output. Instrumentation records 67,239,330 sequential
input loads, 134,217,500 initialized-history loads and 201,321,248 bounded
writes across those three calls. Independent zlib decoding verifies both large
artifacts and output hashes. This is actual memory execution, not a length-only
boundary fixture.

Two fresh directories produce identical untouched SG decoder/smoke COFF bytes,
metadata-normalized C adapter/verification COFF bytes and SG-linked PE bytes.
Both cold programs return 42 with empty stdout/stderr and exercise stored,
fixed and dynamic decoding. C compilation retains actual consumed header
dependencies, code/data/relocation and relative symbol addresses; no compression
policy is delegated to those adapters. The existing qualified historical SG
linker, pinned Kernel32 and explicit actual-NTDLL stack-probe import remain
documented development SDK dependencies, not final SDK packaging.

The real wrong-result oracle control changes only the first expected result
and exits one with `result-oracle expected=-1 actual=0`. Root's independent
qualification controls additionally reject wrong source, native executable,
stack import library and declared linker-parent pins before any tool command.
Failed earlier FFI-helper and runner experiments are retained under target;
they are not promoted to successful qualification. The actual oracle here is
Python's built-in zlib in its loaded Python DLL; the generator and runner both
record that origin and DLL hash instead of inventing a separate extension file.

## Executed shared-prefix and gzip increment

The previous raw report remains historical evidence for its original source.
The shared-inner/prefix revision was freshly qualified by
`target/selfhost/runtime-deflate-prefix-final/evidence.json`, SHA256
`1cf6bf6a9c681d4491611828b5fdbfffd2db51f37046e5ac0773e04e143929a1`.
All 46 commands passed at actual start/end revision
`11f4cb75a0eac9baac4e1209bb8607215b104dd4`. It reruns the complete 5,290 raw
fixtures and both real 64 MiB modes, plus four prefix cases, three legacy exact
tail rejections and eight invalid-limit no-access checks. Prefix success checks
both packed counts, unread following bytes and a null zero-capacity empty
output; no second LZ/Huffman decoder is introduced.

The pre-EOF-trim gzip qualification is intentionally retained at its executed path,
`target/selfhost/runtime-gzip-trial-02/evidence.json`, SHA256
`567da8ea5c204b67562c0c293113338f83ce3575b07de77b746b80d34a1094b6`.
Its label does not denote a skipped or partial run: all 51 commands passed on
`Windows-11-10.0.26300-SP0` at actual start/end revision
`32c70b734e05f9101b914334e4fd00dd147276e5`. Its 2,686 source fingerprints,
313 external inputs, 66 tool identities and Cargo configuration candidates were
unchanged. Runner SHA256 is
`337f72f2db55e1810378b9dd5ed2be17ffd6a73bbd965abfd4a82a119d0edced`.
Two fresh directories produce byte-identical untouched raw/gzip/smoke SG COFFs,
three metadata-verified C fixture/adapter objects and SG-linked PE images. Both
programs cold-execute 42 with empty stdout/stderr. Their raw object bytes must
also equal the independently fully requalified raw-prefix object.

The 8,061-fixture gzip corpus regenerates identically with SHA256
`539da730964da46cccae92e1adfe2e588acc67e192131b7b3a38921d2bd583a7`.
Each instrumented and actual-adapter executable completes 22,731 calls,
including 14,670 named head/tail guard-page calls, plus ten no-access bad
extent/mode checks and a real null empty-output call. Its aggregate is 1,093
successes, 21,323 malformed results and 315 capacity failures. Tests exercise
all optional flag combinations, header CRC, trailer CRC/ISIZE corruption,
truncation at every selected boundary, exact first-member consumption,
concatenated/strict/zero-tail policy, 65,535-byte name/comment limits and maximum
extra length. Cuts at complete member boundaries are independently classified
as valid when the selected policy permits them; they are not falsely counted
as malformed. Invalid mutation capacities use a format-derived output upper
bound so an earlier capacity failure cannot masquerade as the intended
malformed-stream oracle.

All 8,061 inputs are independently executed through both pinned flate2
`GzDecoder` and `MultiGzDecoder`. The source-bound offline Rust report is
`target/selfhost/gzip-rust-policy-source-bound/evidence.json`; its exact input
corpus, executable command and stdout digest bind every declared observation.
That report's SHA256 is
`be72da268ed97c27adaa6b1b44c0bdbd39ef2ba2382ea968de291872f8795bb6`.
Actual rustc/Cargo binaries and the standard rlibs are hashed; dependency
name/version/source/checksum tuples match this repository's Cargo.lock, each
cached `.crate` archive matches its lock checksum, and every regular archived
source member matches the source actually compiled. Wrapper overrides are
disabled and the actual build uses `--locked --offline`. Failed preliminary
dependency selection and path experiments remain separate target reports.
The gzip gate compares every Rust success/error and successful byte sequence
to the independent Python zlib policy, retaining exactly four classified
observations for the two oversized metadata cases. It also classifies 23
direct Python gzip differences: empty files, skipped FHCRC/reserved flags,
first-versus-multiple-member behavior and stricter zero/trailing policies.

Both modes execute four additional real large-range cases: 64 adjacent 1 MiB
members decode to 64 MiB; capacity minus one fails; first-member mode returns
its exact 1 MiB/1,054-byte counts; and an entire 64 MiB gzip input decodes
67,103,726 bytes following index modulo 251. Input is read-only and ends at a
guard page; output also ends at its authorized guard. Instrumentation observes
67,244,822 sequential input reads, 336,587,438 initialized output reads
(including SG CRC32 validation) and 202,369,968 sequential writes. Production
mode reports unobserved counters and access-sequence claims as null. A real
incorrect expected-result corpus control exits one at the decoder oracle.

This is a bounded framing foundation. The actual package resolver permits
64 MiB compressed input, **256 MiB cumulative uncompressed output** and 10,000
tar entries. This decoder's 64 MiB output limit does not implement that package
budget; streaming/chunked 256 MiB output, tar policy, lazy-consumer checksum
draining and publication safety remain separate unchecked integration work.

## Exact EOF-whitespace requalification

Staged whitespace review found one extra final newline in the new C gzip
fixture, which the earlier working-tree diff check had not covered while that
file was untracked. Removing exactly that one byte retains one final newline
and changes no C token. The original fixture SHA256
`0162bbdd5cde4092eaad3bf828898fb50dd536e78703a114dcd0dcbde599733a`
becomes `594cfea15e923492c39f2b2b730ab5f3ce52e144b8664039eff6e09473e08a9c`.

The current exact-source qualification is
`target/selfhost/runtime-gzip-final-posttrim/evidence.json`, SHA256
`3105dcb6d2ea91e6e0b27f3194992f2e3bb7dc7381bcd12c0398b3a5d12ef71f`.
All 51 commands were actually rerun on
`Windows-11-10.0.26300-SP0`, at actual start/end full revision
`adcb31505a6317cb5a488f243e56ba6e51195302`. Every source, external, tool,
checkout and Cargo-config guard is empty. Both fresh directories' SG objects,
normalized C objects and PE hashes are identical to the pre-trim run, as are
all instrumented/production corpus and 64 MiB probe results. The runner remains
`337f72f2db55e1810378b9dd5ed2be17ffd6a73bbd965abfd4a82a119d0edced`.
The shared raw and Rust input closures were unchanged; their earlier 46- and
10-command qualifications are consumed rather than falsely claimed to have
been rerun. The old `567da8...` report remains the genuine pre-trim historical
record and is not rewritten to qualify the new fixture bytes.
