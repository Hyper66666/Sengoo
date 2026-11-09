## Raw decoding

- [x] Implement bounded SG bit input and canonical Huffman decoding with exact limit failures.
- [x] Implement stored, fixed and dynamic blocks, repeats, length/distance decoding and overlapping/cross-block copies.
- [x] Keep C adapters limited to memory access and verify every SG access against caller extents.
- [x] Execute independent compressor/decoder comparisons and explicit malformed/truncated/extent controls.
- [x] Qualify actual SG object/PE production, two fresh builds and complete source/tool/host/revision evidence.
- [x] Validate this child and all specs strictly.

## Framing and tool integration

- [ ] Implement and qualify gzip framing/checksums and the actual package consumer's framing policy.
- [ ] Integrate tar/package workflows with unchanged public limits and failures.
- [ ] Qualify final SDK packaging, self-building and performance before replacing the reference package path.

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
