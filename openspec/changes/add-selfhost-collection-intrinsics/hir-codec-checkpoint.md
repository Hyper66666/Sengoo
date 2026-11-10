# HIR6 owning candidate checkpoint

This document owns the candidate-codec design and evidence ledger. Generic HIR
snapshot/serialization fidelity belongs to `add-selfhost-semantics`; provider,
constructor, callback-body and optional-borrow compiler authority belong to the
other requirements of this collection child. No requirement is duplicated.

`ShHirOrigins` has schema one, explicit presence, mandatory unlowerable one and
the complete original `ShOriginFacts` and `ShBackingLoans` models. Its strings
and vectors are deeply copied. The original candidate models and their table
widths are unchanged: origin nodes eight, returns/obligations/calls five,
factories six and diagnostics three; loan nodes eight, leases ten, effects/
returns/calls six, diagnostics/admissions five and parameter-indexed summary
words. The summary bits remain effect one and required-origin two. Candidate
source diagnostics are data, while fatal input/arena failures remain errors.
Validation zero proves fidelity, not source safety. A future authority consumer
still needs a clean relevant function and independent provider, backing-loan,
ownership and optional-payload checks.

Before constructing a witness, each module's canonical snapshot path/identity,
actual copied UTF-8 bytes and source ID must agree with its physical source
row. The joined source and five-word source-map rows are reconstructed from
the actual unique order with the loader's BOM normalization and two-newline
separators. Disk snapshot matches alone cannot authenticate a replaced source
table. This module-aware subset caps physical units at 100,000 and the graph/
physical-source byte configuration at 16 MiB; smaller actual wire/arena budgets
remain mandatory. These are explicit admission caps of the new HIR witness,
not a retroactive change to the general module loader's API.

The module witness encodes complete graph source snapshots, physical paths,
identities and hashes, joined text and source map, graph configuration/features,
all six checked module maps, front-end mapping/deprecation metadata and scalar
state. Module lift appends an independent serialization of the actual supplied
ownership state. The enclosing HIR already owns the actual expanded source,
AST and all `ShChecked` arenas/scalars/symbol bytes, so the witness does not
duplicate those again. Module validation compares them all word for word to
the independently supplied actual graph, checked input and ownership. It does
not rerun `Sh_check` or type interners. External collection identity/hash,
raw-new prototype and Option identity/hash are parameters; they are never
read from the decoded candidate to authorize itself.

Legacy lifting has no physical module authority. It conservatively recognizes
actual bound intrinsic placeholder shapes/references and rejects 5008 when
such candidates require pins. A module-aware absent capture uses actual
standard/nonstandard source IDs and checks real binding references, including
function aliases. Unused standard declarations can remain absent. This HIR
acceptance does not prove full standard-library code generation: the current
lowerer creates jobs for nongeneric bodies, including unused standard helpers.
The ordinary lowering entry rejects every present candidate and every module
witness, so a forged nonempty absent witness cannot execute placeholders.
Future certified lowering must use a separate fresh authority entry.

The wire and global owning-arena budget remain 4,000,000 i64 words.
Lift and clone perform a read-only total scalar/word preflight before copying
output arenas; the appended ownership witness is charged to the same total.
All additions use remaining-capacity checks. Counting candidate wire words
strictly checks UTF-8 and Unicode scalar counts without encoding a second full
payload merely to compute its size. The enclosing schema-six fixed header and
length metadata consume 78 words, plus actual vector entries, diagnostic
records, symbol text headers/scalars and the complete candidate payload. HIR
error/start/end are explicitly encoded; only a successful HIR is encodable.
Caller maxima use this same exact wire-word unit for clone and encode, with
exact-total and total-minus-one controls. The ownership witness is counted
before its output serialization and is included in the same total.
This per-object budget does not describe
input/output overlap or runtime temporary peak RSS; those remain M8 concerns. Current
origin.input, loan.origin_input and module witness contain overlapping state;
the real standard collections candidate is about 2.55 million words before
the enclosing HIR. These costs are recorded explicitly. This checkpoint does
not raise the cap, discard raw fields, qualify the full compiler closure, or
claim M8 memory/performance results. A future deduplicated immutable arena
would require its own checked range/codec design.

The positive module fixture is a constructed, independently caller-pinned set
of real physical source files with canonical generic/specialized iterator
factory shapes and Option fields. Its source bytes are preserved by the gate.
Its old ownership pass succeeds; source loan diagnostics are deliberately
present and faithfully roundtrip. It is not the official SDK provider or a
qualification of runtime collection algorithms. The actual complete standard
collections input remains a separate boundary probe: the old ownership pass
returns 6101 with 112 diagnostics, and successful module HIR rejects. Those
diagnostics are neither cleared nor represented as an actual-core reduction.

The full gate retains the existing typed HIR/MIR, canonical-disarm, projection
and 49-positive/five-negative source-native behavior regressions. Development
compilation still uses its recorded C/Rust runtime. Nested legacy clang-link
commands are behavior evidence, not complete installed SDK/child-linker
provenance. Final compiler self-building and M3/M4/M6/M8 remain open.

Qualification ledger (2026-11-10, Windows-11-10.0.26300-SP0):
`target/selfhost/m3-hir6-owning-budget-qualified3/evidence.json`, SHA-256
`87a96fcb6e9a91eafa7ccab82d287710573a95eaa1e11512516fa316f07f08f4`,
ran from full revision `0b797a88391fe8e28a7bb35d2255ab79b3da02ca` to
`b7867614ce59d4e404c702784df52db6acb9db8a`, UTC
`2026-11-10T01:03:20.375282+00:00` through
`2026-11-10T01:15:13.375010+00:00`. Concurrent commits did not change any
consumed source, tool, configuration or guarded checkout input. All five
source/tool/configuration/checkout/artifact change lists are empty. The
producer records 282 input hashes, 215 original checkout hashes, eight actual
tools, 21 outer commands and 275 retained binary/object/IR artifacts.

The exact command was Python `selfhost/verify_hir_origins.py` with `--seed
D:/Sengoo/target/agent-conditional-compiler/bin/sgc.exe`, `--reference
D:/Sengoo/target/selfhost/seed-reference/release/sgc.exe`, `--clang
C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe`,
`--formatter D:/Sengoo/target/agent-formatter/debug/sgfmt.exe` and `--output
target/selfhost/m3-hir6-owning-budget-qualified3`. The report preserves the
actual resolved tool paths and full hashes, selected cargo/rustc invocations,
versions and present/absent ancestor Cargo configurations. Source compilation
explicitly selected development runtime mode and forced rebuilding.

The first-call resource scopes comprise ten ordinary, three candidate, three
absent-module and one actual-standard-library calls. Actual tests exercised
120 raw-field poisons, thirteen physical-map poisons, replacement of actual
noncanonical source bytes, eight candidate codec header/scalar refusals and
three HIR header refusals. Exact-total encode/clone/decode succeeds and
total-minus-one encode/clone rejects before output copies. A separate caller
cap exceeds every individual vector while rejecting the aggregate. Exhausted
pre-clone input budgets preserve the complete actual source/typed/ownership
state. Both deliberate assertion failures exit one with the fixture in stderr.
Legal nonzero HIR start/end values survive clone and codec; error-bearing wire
headers reject 5502. The mini full HIR occupies 48,813 words. Actual standard
source has 35,325 AST nodes; origin input is 549,798 words, loan origin input
852,198 and module witness 372,931. Its faithful candidate wire occupies
2,551,671 words (20,413,368 bytes), while actual ownership remains 6101 with
112 diagnostics and successful module HIR rejects 5502.

Required legacy behavior reports also pass their expected command exits:
four HIR/MIR commands, nineteen canonical-disarm commands, twenty-two
projection commands and 346 source-native commands covering 49 positive and
five negative programs. Canonical disarm retains the explicit result
`passed-with-reference-zero-drop-policy-difference`; it is not a parity claim.
MIR qualification now discovers and guards nineteen actual recursive frontend
imports instead of a stale fixed list of eleven. Nested report hashes are
preserved in the producer report.

Independent retained-evidence audit
`target/selfhost/m3-hir6-owning-budget-qualified3/audit.json`, SHA-256
`beb39143b3ad1944416ebc63dd46b955b3539881435160a5167eed45abdfffba`,
rehashes 3,709 source/tool/report/artifact/receipt records and 820 receipt
files, including child case outputs and deterministic repeated objects.
The old source-native child retains receipt files without embedding their
hashes in its original report; this audit records their actual retained hashes
without rewriting that historical child schema. The nested clang linker
commands remain behavior checks rather than complete SDK provenance.

Earlier failed trials remain intact. In particular, budget-qualified first
failed source checking because two mutable budget calls shared a logical
expression; they were sequenced. Budget-qualified2 reached and passed exact
budget controls, then exposed a nonzero HIR wire error being propagated as
its raw code rather than schema refusal 5502. The final fresh producer above
contains the explicit rejection and reruns the complete regression set.
