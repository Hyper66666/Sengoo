# Compiler source snapshot qualification, 2026-11-11

The current implementation removes the public FileSnapshot table from compiler
module ownership. The full compiler source input now reaches the next semantic
diagnostic. This checkpoint does not close M3, establish a native compiler fixed
point, qualify the final SDK, or close M6-M8.

The five required gates, cross-report integration audit and additional
collection-effects override replay have passed. Results remain valid only
for their recorded source bytes and development toolchain.

## Recorded source and host

- Base revision: `3a20c17026cdbd099dd13a524e491b3c43cbadb7`.
- Branch: `codex/self-hosting`; the tested source changes were uncommitted at
  this revision and are identified by each report's consumed-input hashes.
- Host: `Windows-11-10.0.26300-SP0`, Windows x64.
- Seed: `target/agent-conditional-compiler/bin/sgc.exe`; formatter:
  `target/agent-formatter/debug/sgfmt.exe`; reference:
  `target/selfhost/seed-reference/release/sgc.exe`.
- Clang: `C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe`.
- Python: `C:/Users/admin/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe`.

These are verified local paths, not installed discovery defaults. The gates
compare tool identities with the selected locks, retain the actual source
closure, record the actual Rust compiler invocations and check input/tool/output
identity again after execution. Source runtime overrides use
`--runtime-mode source-development` before the compiler subcommand.

## Completed gates

Paths below are relative to `target/selfhost/` and contain complete commands,
timings, input manifests, logs, artifacts and final integrity checks.

| Gate | Report | SHA-256 | Result |
| --- | --- | --- | --- |
| Source arena | `compiler-snapshot-formal01/evidence.json` | `a3ceb884c75588b09c5f6836da0bf0168c15a24ca51b940bded1ae2b6c9d8ec1` | 26 commands; passed arena boundary |
| Ordinary compiler | `compiler-snapshot-ordinary-qualified2/evidence.json` | `8c3332c989dd51bba88f8c9d871673f32384abf69a0f59d316c4346e2f6fd962` | 22 outer commands; passed ordinary regressions |
| HIR and nested regressions | `compiler-snapshot-hir-qualified1/evidence.json` | `d02e3a769c5540f38cc16d51d3d35fd643eae967a592373f22346515c0a14fd2` | 21 outer commands; passed |
| Collection provider | `compiler-snapshot-provider-qualified3/evidence.json` | `5986c47f3547447e99091d1ab7bac5e046412c94d5904e88a04bf8c34824b97a` | 19 commands; passed |
| Fresh scalar collection flow | `compiler-snapshot-flow-qualified2/evidence.json` | `124456ee04eed88f744896bd5227942adf3d22a6713f30686dcb9c4c505606c7` | 41 commands; passed |
| Cross-report integration audit | `compiler-snapshot-integration-audit.json` | `d5ddc9152d2ac2266a91add6a4819eb06d62ebb273cb317364c2434b88aa8df2` | 14 reports, 6,459 files, 26 configuration candidates; passed |
| Collection-effects provider override | `compiler-snapshot-effects-qualified1/evidence.json` | `b369397c05c5f1a8a4402def0d36fd7c0866bffd118afe4e898d978d2083fc0f` | 10 commands; passed |

The arena gate ran from `2026-11-11T04:48:46.964019+00:00` to
`2026-11-11T05:04:44.471139+00:00`. It binds 223 original producer files and
their retained copies, 2,113 consumed inputs, 11 tools and 63 artifacts. Its
independent OS oracle builds and executes the exact captured native adapter,
including absolute seek and reread of the retained handle. Nine OS cases close
their handles. The SG tests exercise 1,024 accepted owners, a rejected 1,025th
identity, duplicate/hardlink admission at capacity, caller limits above the
arena limit, malformed and embedded-NUL inputs, physical content/digest/identity/
path mutations, and source-buffer shape rejection before a destination write.

The fault adapter is a test-only wrapper around the exact original C source.
Thirty resource scopes cover early EOF, oversized read counts, changed second
metadata, missing final EOF and positive fragmented reads. The ordinary arena,
module and integrity tests also observe resource restoration, including native
handles. The public FileSnapshot capacity remains 32; the compiler arena has a
separate 1,024-owner bound. The actual compiler's 16 MiB input limit is unchanged.

The real full compiler self-input took 603.91 seconds and exited 65 with
`phase=semantics`, `code=5124`, source 49, standard `file.sg`, bytes 6174..6327.
No object was emitted. This is evidence of successful source admission followed
by a real later rejection, not a self-build success.

Ordinary MIR, projection, disarm and source-native gates passed. The disarm
result retains its named `passed-with-reference-zero-drop-policy-difference`.
Source-native covers 49 programs, including 46 reference comparisons and three
documented reference gaps. HIR6 mutation, codec, physical source and resource
controls passed with the same snapshot producer; its faithful standard-library
ownership rejection remains a separate unfinished semantic boundary.

The flow replay ran from `2026-11-11T05:06:52.058989+00:00` to
`2026-11-11T05:22:52.597341+00:00`, with 254 original producer inputs and their
retained copies, 973 consumed inputs, 11 tools and seven artifacts. Three fresh
SG objects are byte-identical (SHA-256
`e98e43c4b7f4ab97807b6c8be2cc48b9fecf51784dcac1dcc13d3f68a094b4f6`).
Seven functions execute 7,000 times through the independent C consumer; all
22 source/provider/budget/ownership rejection cases pass without an output
object. All final input, snapshot, tool, configuration and artifact guards
are empty.

The additional effects replay executes the new explicitly selected provider
argument branch against provider-qualified3. All six selected functions have
ownership coverage, with 30 operations, 37 old-NLL supersessions, 22 old-loan
supersessions and nine lowering rows. Fifty-three effect mutations and 28 outer
mutations reject; three scopes restore String, Buffer, opaque, public snapshot
handle and retained-byte counters. Both intentional assertion failures exit one
and the complete native scope exits 42. All 720 consumed inputs, 199 original
producer files, tools, artifacts and configuration guards remain unchanged.
The report retains the same starting/ending base revision. Separate independent
rehashing checked 890 input/original/artifact/tool/log paths plus configurations;
this supplementary gate is not silently folded into the earlier five-report
audit's counts.

## Provider rebuild and retained failed experiment

`compiler-snapshot-flow-qualified1/evidence.json` failed with 5008 on its first
success case and is retained. Independently swapping old/new compiler and
provider pairs isolated the cause to the provider object pin. The new and
qualified old COFF objects have 48,457 bytes; only the timestamp bytes differ.
An actual `SOURCE_DATE_EPOCH` experiment did not change this clang's behavior.

The provider runner now requires explicit opt-in to a reviewed timestamp-only
conversion, with the original report independently pinned to
`a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3`.
It preserves the new raw object, compares the complete C/header closure and all
non-timestamp bytes, and writes a separate object with the exact original hash
`6186b302533aa26588534a0062ccbaa488b58df8561c692a8c0257c65193575a`.
Every one of the 48,453 non-timestamp single-byte mutations and both length
mutations is rejected. The actual C consumer and five SG resource scopes then
execute against this selected object. The strict SG admission contract is
unchanged. This is a recorded build transformation, not raw reproducibility.

## Integration audit and limits

`selfhost/audit_compiler_source_snapshots.py` requires the arena, ordinary, HIR,
provider and collection-flow reports to pass. It rehashes their consumed files,
retained copies, tools, artifacts and nested reports; compares each actual
import closure with the current producer; checks the fault wrapper derivation
and provider conversion; and repeats every observed hash at the end of its own
audit window. It requires all seven collection functions, 7,000 executions and
three byte-identical generated objects before reporting success.

The final audit is `passed-snapshot-integration-audit`. Its 14 reports include
the five required top-level reports, eight nested ordinary/HIR regressions and
the independent OS oracle. It rechecks 6,459 distinct files and 26 present/absent
configuration candidates, with 1,384 legacy stream observations explicitly
identified as described below. The auditor source SHA-256 is
`44562b5774739926ce2055c9113b0f71bdbbed838a689b732ddb40f666f49ef5`.

Strict child, collection-intrinsics and all-spec validation passed; all-spec
validation covered 116 items with zero failures. Exact command/log/tool receipts
are in `target/selfhost/compiler-snapshot-strict-ledger.json`, SHA-256
`ca08ea550803a1dca8254c879cece707b8638f805c76136ef99ecf199cea91eb`.

The independent four-report pre-audit checked 13 reports, including the nested
OS oracle, and 6,091 files; it deliberately remained preliminary while flow
coverage was pending. Its isolated controls exposed two audit implementation
gaps: an absent Cargo configuration created during the audit was not checked
again, and a cached report could bypass a later expected-result check. Both
are corrected in the production auditor. The exact-function negative controls
in `target/selfhost/snapshot-audit-controls02/evidence.json` (SHA-256
`d48182aa255fccad1216b347acc94b6fc5b041f1ffe92e18fc3b50d8177ebea1`)
reject both cases and retain a passing unchanged baseline. No actual source
report or Cargo configuration was modified by these controls.

Some legacy nested source-native reports did not originally hash each stdout/
stderr stream. The audit records those retained logs as audit-time observations;
it cannot retroactively establish original per-stream receipts. These behavior
regressions do not provide complete MSVC child-linker/SDK bootstrap provenance.
The source arena's conservative system-library manifest and current Windows
OS checks likewise do not qualify a final SG SDK. Performance/RSS, all-tool
fixed point, clean installation and two final release candidates remain open.

Live observation of the flow gate's seed child found Windows resolving clang
through the app's LocalCache path. Its executable hash at observation time was
identical to the explicitly selected clang (`56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3`).
This observation does not extend the historical gate receipts into a complete
child-process/toolchain provenance claim.
