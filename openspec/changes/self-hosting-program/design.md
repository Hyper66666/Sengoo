# Architecture, ownership and integration gates

## Shared architecture

The SG implementation lives under `selfhost/`. Source buffers and typed
integer IDs back source positions, symbols, AST, type, HIR and MIR arenas.
The four tools share analysis and diagnostics rather than maintaining parser
or type-checker copies. Compiler implementation initially uses already
supported SG forms; a seed defect requires an isolated regression before a
documented seed refresh.

The target data flow is source -> AST -> type checking -> HIR -> MIR -> x64
machine code -> COFF -> SG linker -> EXE/DLL. Layout and calling conventions
belong to an explicit Windows target ABI layer. The linker supports the object,
archive/import-library, relocation, DLL, unwind and debug features required by
the frozen support corpus. Native debug completion includes CodeView/PDB and
actual debugger breakpoint, step and value inspection evidence.

A versioned internal MIR exchange adapter may compare the reference frontend
with the SG backend. It is a bootstrap/testing boundary, not a new published
application ABI. SG LLVM-text export retains `--emit-llvm`; any LLVM reference
execution is explicitly selected and is excluded from the native self-build
gate. Native failures must not silently fall back to a reference compiler.

Runtime algorithms migrate to SG with one implementation/owner per exported
symbol and resource. Runtime ABI 1 and generic collections descriptor ABI 1
remain in force. Minimal C operating-system/memory adapters and necessary
third-party native libraries are permitted SDK inputs with source provenance,
target, versions, ABI and content hashes. The SDK must not hide Rust tool or
compiler logic. SDK source rebuilding and existing native-C development remain
explicit external-toolchain procedures outside the no-external-tools gate.

## Single implementation owners

| Work | Requirement owner |
| --- | --- |
| Program sequence, evidence ledger and final integration gates | `self-hosting-program` |
| Frozen corpus and daily performance reference qualification | `freeze-selfhost-baseline` |
| Seed identity, stage0 driver, source/span/symbol/node arenas | `add-selfhost-foundations` |
| Stage0 Windows synchronous Unicode process-launch correction | `fix-windows-process-utf8-launch` |
| SG lexer/parser/macro/attribute pipeline and formatter integration | Future syntax child |
| SG semantic analysis, borrowing/Drop, HIR/MIR and reference comparison | Future semantic child |
| Windows x64 code generator, target ABI and optimization | Future native-backend child |
| COFF/PE linker, CodeView and PDB | Future native-linker child |
| Remaining SG runtime, package manager, LSP and compatibility ports | Future toolchain-port children |

Future rows are reservations, not approved completed implementations. Each
future child receives concrete deltas and verification before its work begins;
the umbrella links to them rather than copying their behavioral requirements.

## Milestones

| Milestone | Exit evidence |
| --- | --- |
| M0: freeze baseline | Published checkpoint; explicit seed/source/tool identity; supported corpus and repeatable baseline measurement inputs |
| M1: bootstrap foundation | Seed builds the SG entry and foundation tests; malformed inputs fail clearly; bounded source/arena and resource tests pass |
| M2: SG syntax | Self-host source parses; macro/attribute/spans compare with reference; formatting is idempotent and preserves semantics |
| M3: SG semantics/MIR | Self-host corpus type-checks and lowers; supported accepted/rejected behavior compares with reference |
| M4: SG x64 backend | Native execution and bidirectional C ABI tests; external linking permitted only as a transitional verification path |
| M5: SG linker/debug | Native backend plus SG linker produce runnable EXE/DLL and verified unwind/CodeView/PDB artifacts |
| M6: first compiler fixed point | Same SG compiler sources produce identical successive native compiler generations; remaining full-compatibility gaps remain visible |
| M7: full toolchain | All four tools and SG runtime build from SG; all documented Windows supported behavior and protocol ports pass |
| M8: default cutover | Full-toolchain fixed point, clean installation, compatibility, performance, two consecutive candidates and rollback proof |

M0's checkpoint publication is already performed. The measurement freeze and
other M0 exits remain open until their named checks are recorded. M1 does not
claim M2-M8, a minimal platform SDK, or a self-hosted compiler.

## Final self-build proof

With the same SG source revision, target, SDK and flags:

1. S0 (the locked Rust seed/reference toolchain) produces the SG toolchain S1.
2. S1's SG native backend and SG linker produce S2.
3. S2 produces S3 through the same SG native path.
4. Compare unsigned S2 and S3 tool executables, linker, SG runtime objects and
   PDBs byte for byte. S1 need not equal S2 because S1 may be LLVM-generated.

Deterministic symbol order, timestamps, path mapping and debug identifiers are
part of this proof. Repeat with different working directories and empty
caches. Record actual subprocesses; the final environment provides no reference
compiler, Rust, clang/LLVM, external assembler or external linker. Fixed-point
equality supplements semantic/reference tests and does not establish correctness
by itself.

## Compatibility and performance

Freeze the documented Windows Stable/Supported-subset corpus and classify
experimental features separately. Keep edition 2026 and published diagnostic,
test, manifest, lockfile, package, reflection and runtime versions. Cover native
FFI, ownership/Drop, async/concurrency, Python bridges, reflection, CLI workflows,
package resolution, formatting and versioned/cancellable LSP snapshots.

The package port must preserve current TOML, semver, tar and general DEFLATE/gzip
behavior; existing restricted stdlib helpers do not by themselves establish
package compatibility. Cache identity includes source bytes, backend, options
and SDK contents. Existing source-runtime cache correctness remains required.

Before default cutover, compare frozen representative startup, check,
full/incremental build, formatting, LSP and generated-program workloads on the
same host with equivalent optimization levels. Warm up then collect at least
ten measured samples with reference/candidate interleaving. Record raw timings,
median latency and peak RSS per workload. Candidate median latency must be at
most 110% and peak RSS at most 115% of the frozen reference; existing resource,
artifact and compile-scale gates also remain required.

Changing this workload set or baseline requires explicit evidence rather than
rebaselining a failure away. Keep two consecutive complete candidates; supported
behavior fixes restart that sequence. A Windows preview does not replace the
existing four-host stable publication gates.

Before M8, freeze installed reference and candidate runtime/SDK bundles
independently. Do not feed changed candidate runtime sources to the frozen
reference compiler and call it an unchanged performance baseline. M0's local
source-development sampler qualifies the current reference snapshot; final
installed/no-external-tools qualification remains a separate integration gate.

## Evidence rules

For each named gate record the full source revision, local source fingerprints
where needed, host, seed/tool/SDK hashes, exact commands, exit status, raw output,
limitations and result. A configured check, old report, source mismatch, skipped
required host/debugger or preflight success is not a pass for another artifact.
Keep reference artifacts and a tested rollback path through cutover.
