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
## Bounded continuation increment

- [x] Implement fixed 45,056-byte SG state and primitive memory-only adapters.
- [x] Verify all block/tree/repeat/extra/match phases across single-byte and arbitrary input/output cuts.
- [x] Verify exact/prefix consumption, sticky finish, zero buffers, reset/copy/dispose and invalid API no-access controls.
- [x] Execute independent Python/Rust complete-output and malformed/history comparisons with guard pages.
- [x] Qualify bounded peak state and source/native artifacts; record an honest prototype boundary.
- [x] Migrate old raw/prefix/gzip exports to the one shared core and rerun complete original raw/gzip gates.
- [x] Add and independently qualify SG gzip framing/CRC continuation with fixed caller-owned state.
- [ ] Migrate old gzip export to the single framing core and run full gzip/raw/continuation regressions.
- [ ] Add incremental tar/staging integration and private full-validation/atomic publication.

### Continuation prototype exact-source evidence

The new continuation core is separately qualified by
`target/selfhost/runtime-deflate-stream-formal-04/evidence.json`, SHA256
`7470038605a737782d84399768dea83d71bafe1837fc77bffa076c2b18dd59fb`.
All 35 commands actually completed on Windows-11-10.0.26300-SP0 at identical
start/end revision `7416bc9e95abb66fee8cf26a53ca58fcc8d008b5`. The 1,359
source, 263 external-source, 44 tool and 88 artifact manifests all have empty
change guards. Earlier failed qualification attempts (stage name and actual
formatter/stack artifact-field assumptions) are retained without rewriting.

Each instrumented and production executable completes the original 5,290
independent raw fixtures, 17,337 schedule runs and 8,125,565 step calls.
Schedules include one-byte input/one-byte output, fixed-seed independent
chunk sizes, full-input bounded output, and every two-part input cut for
accepted streams at most 64 bytes long. Results include 268,375 need-input
and 7,839,510 need-output responses. Instrumentation records 18,229,227 ring
reads only after an actual emitted byte initializes that history slot; init's
zero fill does not grant initialized-history authority. Production history
read counts are explicitly null. Input is read-only and ends at a guard page;
the 45,056-byte state has guards on both sides, and each reused output chunk
ends at a guard page. Output pointers change between different capacities.

Protocol assertions additionally execute prefix consumption of exactly four
bytes without reading following framing data, exact-tail rejection, waiting
for final EOF, zero-capacity pending literals, sticky finish and unconsumed
input retry, zero-input draining of a 258-byte overlapping match, bytewise
inactive-state cloning with two independent completions, reset counts and
history, zero-output null buffers, complete dispose erasure and fresh/failed/
disposed query behavior. Eleven invalid API variants use null pointers and
perform no state-word accesses. State/input/output pointers are not retained.

The independent pinned miniz_oxide 0.8.9 report is
`target/selfhost/deflate-stream-rust-02/evidence.json`, SHA256
`343a6525b4669c289dc83a0b3080bb0f237b06f728d4ec7c134a09c521d21b88`.
Seven actual commands build offline with locked repository dependency
identities and cached source/archive byte verification, then execute every
fixture. Its actual command/stdout is reparsed and compared with all declared
IDs/results. The independent Python zlib oracle revalidates the complete
original corpus. 528 valid/capacity-source outputs equal Rust output. Eighteen
named differences remain explicit: two unused HDIST31/32 declarations that
miniz rejects, two incomplete alphabets miniz accepts but SG/zlib reject, and
three explicit plus eleven mutated trailing-byte streams that miniz accepts
as prefixes while SG/zlib exact mode rejects. No compatibility rule is widened
to hide these observations.

Two fresh directories emit identical SG core/smoke objects, normalized leaf
C memory/constant-data fixture objects and SG-linked PE bytes. Both empty-PATH
cold executions return 42 with empty stdout/stderr; SG itself drives 259
one-byte-output calls and checks 259 literal bytes and dispose erasure.
The gate consumes the unchanged UTF-8 qualification's complete native,
formatter, historical SG linker and explicit NTDLL stack-probe source/tool
branch. It separately consumes the immutable raw corpus rather than claiming
that old raw/gzip source qualifications were rerun or migrated. Actual C
header closures and actual selected LLD/library inputs are bound.

The persistent state is exactly 45,056 bytes; this is not a whole-process RSS
claim. A read-only inspection of the actual SG object finds only the six
memory adapter imports and __chkstk, with no heap/runtime-algorithm imports.
The current seed backend allocates a fixed 45,104-byte step stack frame;
tree/symbol/fixed helpers additionally use fixed 7,120/5,520/1,600-byte frames.
Those native SSA temporary costs are separate from the persistent state and
remain subject to later performance/RSS qualification. The decoder allocates
no SG heap storage, and its storage does not grow with output length.

The existing production `deflate.sg` and `gzip.sg` bytes are unchanged. This
checkpoint does NOT claim a final single implementation across their old
exports and the new prototype. Shared-core migration and complete raw46/
gzip51 regressions are unchecked next steps, followed by real gzip/tar
continuations and private staging/atomic publication.

### Single-core migration qualification

The preceding paragraph describes the immutable prototype checkpoint; the
following new-source reports qualify its subsequent migration. The exact
current one-shot wrapper is `deflate.sg`, whose old LZ/Huffman implementation
has been removed. All bit/tree/repeat/length/distance/history policy lives in
`deflate_stream.sg`. Its source bytes and `gzip.sg` framing bytes are unchanged
from the pre-migration barrier. The historical reports above are not edited
or reused as current-source qualifications.

The new raw qualification is
`target/selfhost/runtime-deflate-shared-final/evidence.json`, SHA256
`cd8f88f5e2d56f6e9b7a715061ad1d780d1442b3817705656ae8543e3a5fe672`.
It actually ran **70 commands**, including every original raw/prefix/corpus/
64 MiB/oracle check plus the full new object closure and state-resource
controls. Actual full revision starts
`52ca3dcbaa9d6360007b7ebadfb1991e6ea0d919` and ends
`0b797a88391fe8e28a7bb35d2255ab79b3da02ca`; independent root commits occurred
outside the consumed closure, whose guards are all empty. The report does not
claim one clean revision. Host is Windows-11-10.0.26300-SP0. It binds 2,484
sources, 133 external sources, 45 tools and 35 object/executable/library
artifacts. Each 5,290-fixture mode retains the original 10,006 calls and exact
old statuses, access counts and known failure prefixes; all three actual
64 MiB cases and prefix/exact/no-read controls are unchanged in meaning.

The gzip qualification is
`target/selfhost/runtime-gzip-shared-final/evidence.json`, SHA256
`e4ad46613228f4558eff960e8d5b1301067ea6e4407cb0a5adb3930fcfdaba18`.
It actually ran **73 commands**, including every original 8,061-fixture,
22,731-call, optional-header/checksum/tail-policy, wrong-oracle and four actual
64 MiB tests in both modes. All original expected JSON fields remain equal.
Actual start/end revision is
`0b797a88391fe8e28a7bb35d2255ab79b3da02ca`. Its 2,719 sources, 320 external
sources, 66 tools and 38 artifacts have empty source/tool/checkout/config
change guards. It binds the new raw report and checks the actual raw wrapper
and core object hashes against that qualification, not against old code.

The complete continuation regression is
`target/selfhost/runtime-deflate-stream-shared-final/evidence.json`, SHA256
`820138aa04902a00a585af9240b0c9b9b39c4a0e061a7f50001c2ba7b5bd7f9b`.
Its **35 commands** run at the same actual start/end revision above, with
2,712 sources, 263 external sources, 89 tools and 88 artifacts and empty
source/external/tool/artifact guards. Both modes retain 17,337 schedules,
8,125,565 calls and all protocol/cut/history results from the prototype. The
emitted core object must equal the one actually used by the new one-shot raw
qualification. Only this later report marks `old_exports_migrated=true`.

All three reports execute two fresh qualified SG-linker deployments with
identical SG objects, normalized leaf C objects and PE bytes, and cold empty-
PATH execution returns 42 without stdout/stderr. Raw deployment explicitly
contains wrapper/core SG objects and stream-memory/OS-storage C providers;
gzip adds framing SG and its memory adapters. A missing core object really
returns exit 65 / unresolved diagnostic 7204 and publishes no executable.
Normal one-file compilation is not represented as automatic dependency
resolution. The actual C driver selects the qualified LLD (independent -###
path check), and actual archive reads/header dependencies are hashed.

SG alone requests exactly 45,056 bytes, initializes/drains/queries/disposes
the core and releases the state on every acquired-owner exit. Minimal generic
C acquisition/release uses VirtualAlloc/VirtualFree, with no decoding policy
or global state. Actual C instructions are tail-jump stack-free leaf wrappers;
the gate preserves code/data/symbol/relocation identity while removing metadata
and recording raw timestamps. Kernel32 and the actual loaded OS memory
provider paths/hashes/RVAs are bound. Instrumented tests reuse the actual
provider source, query a 45,056-byte committed region, inspect complete SG
zero erasure and confirm MEM_FREE after release. Each corpus mode has exactly
10,006 raw / 13,692 gzip state lifetimes, large cases 3 raw / 130 gzip and
prefix 7, with created=freed, zero live bytes and peak committed state 45,056.

OOM controls inject one failed acquisition and observe attempt=1, created=
freed=0, live=0, peak=0 and old -2. Exact raw and prefix perform zero input/
output accesses. The real gzip OOM path first reads exactly its ten framing-
header bytes, then performs no raw DEFLATE reads or output writes; this is not
a claim of zero reads for the entire gzip call. Production access counters
remain unobserved/null. Invalid limits still reject before allocation.

Initial target migration trials retained all original raw/gzip error priorities
and outputs, proved actual SG linking and included allocator fault injection;
their reports remain separate from these final exact-source qualifications.
Synchronous state allocation is separate from the caller-owned continuation
API's zero heap allocation. These checkpoints do not implement gzip framing
continuation, incremental tar, 256 MiB package-content budgets, private staging
or atomic publication, and do not close final SDK/performance/self-build work.

### Gzip framing continuation qualification

The new independent continuation qualification is
`target/selfhost/runtime-gzip-stream-formal-02/evidence.json`, SHA256
`aa4df62636ec93fa1a95c33258019fb6e43f9c8620a92d8b36dbd2ff77dc3df5`.
It actually executes **88 commands** on Windows-11-10.0.26300-SP0. The actual
full revision interval is `b7867614ce59d4e404c702784df52db6acb9db8a` through
`e68be33abdf6955b1909dde2e3e8da7440ba12fc`; unrelated concurrent commits are
disclosed rather than represented as one revision. The 1,433 source, 389
external-source, 44 tool and 221 artifact fingerprints have empty source,
external, tool and artifact change guards.

Each instrumented and actual-memory-adapter executable completes all 8,061
original independent gzip fixtures over 38,660 schedule runs and 2,663,697
step calls. Schedules include one-byte input/output, fixed-seed variable
chunks, full-input bounded-output and every two-part cut for successful
streams at most 96 bytes long. Need-input / need-output counts are
684,965 / 1,939,725. Every fixture ID, schedule and terminal status contributes
to checked FNV64 `a5d343516df43dd7`. A Python/zlib-derived sidecar additionally
requires every positive fixture's exact validated-member count. Existing
format/metadata/Rust/Python policy differences are retained through actual
source-bound flate2 command/stdout comparisons; no expected status is derived
from the subject implementation.

Instrumentation observes 10,730,340 initialized raw-history reads and
11,636,249 newly-emitted-current-chunk CRC reads. Production reports those
unobserved counters as null, with every transition/call/result count otherwise
identical. Known output, including capacity and integrity failure prefixes,
is compared per chunk to independent bytes. Read-only input ends at a guard;
state alternates head/tail placement and output chunks end at independent
guards. Explicit protocol tests check all three tail/member policies, empty
input, partial final trailer, sticky finish and retry, zero-capacity pending
output, zero-input draining, inactive-state copy, reset/dispose, member counts
and eleven invalid API cases with no state/offset accesses. Wrong result and
uninitialized-history controls actually exit one for the intended reasons.

The six actual large-range cases reuse only a 32,749-byte output chunk and
46,080-byte state. Sixty-five 1 MiB members produce 68,157,440 bytes, beyond
the old synchronous 64 MiB output limit, with all bytes compared to the
independently constructed repeating pattern and CRC/ISIZE validated. A budget
minus one returns -2 with a correct bounded prefix; first-member mode consumes
exactly 4,424 wire bytes / 1 MiB, strict-single rejects the tail and compressed
budget minus one leaves the last member unvalidated. A separate actual
67,108,864-byte compressed stored stream decodes 67,103,726 bytes in 593,884
calls, with exact consumed/output counts and guard-page bounds. Python gzip
and zlib independently validate generated complete bytes/digests. A crafted
second member that references the previous member's history is rejected,
leaving exactly the first member's one output byte and validated count one.

All full corpus and large C probes link a trapping provider for the imported
old one-shot storage symbols: any allocation/presence/free call exits 99.
A separate forced-allocation control actually exits 99 with no output; all
new framing paths complete normally through that exact same provider. This
proves the new caller-owned algorithm reaches no hidden state allocation.
It does not claim that the test caller, Python oracle or old synchronous API
allocate nothing. SG cold executables instead link the real OS provider as
part of the complete transitional object closure.

Two fresh SG builds compare identical imported framing/core/wrapper/smoke
objects, all normalized leaf C objects and SG-linked PE bytes. Cold executions
have empty PATH/stdout/stderr and return 42. Source snapshots include the
unchanged `gzip.sg` import; its one existing pure-SG CRC helper is reused and
the actual compiled object contains exactly one `sh_gzip_crc_byte` symbol.
The imported old synchronous framing exports are still present; they are not
silently linked as another duplicate object. Full dependency and historical
tool/SDK/linker branch identities are recorded, and old production source
pins are checked separately. Earlier complete qualifications are not rewritten
to pretend their old child-document hashes describe this increment.

The fixed 46,080-byte state is separate from native stack/RSS. Read-only
inspection of the actual new object shows a 29,120-byte gzip step frame,
1,968-byte init, 1,392-byte byte-reader, 592-byte return helper and 1,056-byte
shared CRC frame. The nested raw step retains its separately qualified fixed
45,104-byte frame. These are bounded compiler temporary costs, not a claimed
process-memory or 15-percent performance qualification. C implements only
primitive memory access/offsets and independent fixtures/observation.

The earlier 82-command `runtime-gzip-stream-formal-01` report, SHA256
`922d36467cf1bb88ee1c1ccaf2be621f0ac5369e1497e97ca1555ce8ad822b2d`,
is retained as genuine pre-enhancement evidence. The final increment adds
known-failure-prefix checks, an actually effective allocation trap and the
real 64 MiB input case; its source bytes are qualified by the new report only.
All original `gzip.sg`, `deflate.sg`, `deflate_stream.sg` and memory-provider
bytes remain unchanged. Old gzip export migration, incremental tar, private
staging, full integrity drain and atomic package publication remain unchecked.
