# Sengoo self-hosting

The target is a Windows x64 SG implementation of `sgc`, `sgpm`, `sgfmt` and
`sglsp`, with its own x64 backend and PE/COFF linker. The approved milestones
and compatibility gates live in `openspec/changes/self-hosting-program/`.

## Current boundary

M0, M1 and M2 have passed their recorded local Windows gates. SG now implements
the frontend, partial semantic/ownership checking, an owned typed HIR bridge,
typed MIR, target layout, x64 instruction encoding and COFF object writing.
The initial scalar native path executes real generated code. This is still a
development toolchain: M3-M8 remain open, and there is no compiler self-build
fixed point yet. Containers, strings, files and processes still use the existing
runtime; the minimal native SDK and SG runtime port remain required.

`bootstrap.sg` remains the original stage0 process driver which invokes the
frozen Rust reference compiler. The newer `tools/native.sg` path implements SG
source-to-object compilation in SG. Rust/clang are still used to build that
development executable, and clang independently links verification artifacts.

The prior Agent SDK/ownership implementation was committed and pushed to
`main` as `cc70247ec5bd9531bea751a08288980da40ecc68` before this work started.
The local seed and formatter still report embedded revision `ea803f1913f9`.
The lock records those embedded versions and the source checkpoint separately;
neither is presented as proof of a new clean source rebuild.

The M1 Unicode-path integration test exposed an ANSI synchronous process launch
in the source runtime. `fc1a686a2953adcbd14731cc6f93270cfb9cf34d` fixes that
boundary and adds native/SG regressions. The lock therefore pins source inputs
at that correction, with the same seed binary. The failed pre-fix M1 report is
retained under `target/selfhost/m1-run1/`; it is not promoted to a pass.

## Run the gate

Use a functioning Python 3.11+ interpreter and pass the tools explicitly. This
host's verified interpreter is shown below (its WindowsApps `python` alias does
not execute the gate):

```powershell
& 'C:/Users/admin/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' selfhost/verify.py `
  --seed target/agent-conditional-compiler/bin/sgc.exe `
  --formatter target/agent-formatter/debug/sgfmt.exe `
  --clang 'C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'
```

These are the verified local development paths, not portable installation
defaults. The runner never guesses a compiler from another application.
`--preflight-only` verifies inputs without building and records
`preflight-passed`, which is not a passed foundation gate. `--output` chooses
an evidence directory inside this checkout's ignored `target/`.

The complete gate verifies binary hashes before executing them, source Git
objects and tracked source equality, embedded tool versions and runtime build
dependencies. It then checks formatting, builds the SG driver and core tests,
executes native positive/boundary/lifecycle tests, verifies child error
propagation and missing/tampered seed rejection, exercises spaces/Unicode/`&`
in paths without a shell, and uses the SG driver to rebuild its own entry.
That last check is a driver rebuild through stage0, not the compiler S2/S3
fixed-point gate.

Each run writes `evidence.json` and separate stdout/stderr logs. It records
the actual host, full revision, dirty state, source/tool SHA-256, command
arguments, exit statuses and executable hashes. Environment values are not
serialized. Source/tool changes during verification invalidate the result.
Build caches beside SG inputs are ignored.

## SG foundation API

Import `selfhost_core` using the fixed module map in the driver. IDs start at
zero and belong to their owning collection; they are not globally unique.

| API | Contract |
| --- | --- |
| `sh_sources_new`, `sh_source_add/load/len/count` | Owned immutable source copies, explicit source-count and per-source byte limits; strict UTF-8 file snapshots; failed loads never append a source |
| `sh_span_make/text` | Half-open UTF-8 byte offsets, validated source ID, ordered bounds and code-point boundaries; text lookup returns an owned copy |
| `sh_symbols_new`, `sh_intern`, `sh_symbol_text/count` | Stable first-insertion IDs, duplicate reuse even when full, checked lookup returning an owned copy |
| `sh_nodes_new`, `sh_node_add/get/count` | Append-only six-word records, stable IDs through growth, checked source spans, backward child IDs or `-1`, rollback of partial append failure |

Errors are 1001 (source ID), 1002 (span range), 1003 (UTF-8), 1004
(capacity/allocation), 1005 (node), 1006 (symbol) and 1007 (file read).
Caller limits do not increase existing runtime resource limits. The initial
symbol table deliberately uses bounded linear lookup; performance work remains
required before compiler cutover.

Native core tests cover a 256 KiB source, multibyte boundaries, 4096 nodes,
file failures and 100 complete lifecycles. String, Buffer, opaque handle,
FileSnapshot handle and snapshot-byte counts must recover exactly.

## Lock and seed availability

`bootstrap.lock` is bootstrap-only JSON schema 1, separate from `Sengoo.lock`.
`source_objects` are Git tree/blob IDs from the frozen commit: Git applies
`.gitattributes` normalization, so Windows working-tree CRLF does not create a
false source mismatch. Per-run SHA-256 values identify the actual disk bytes
used by the build. Backend/runtime source edits require a reviewed seed lock
update; editing a path in the lock does not bless an unchecked binary.
The Rust toolchain is resolved explicitly through rustup and its actual native
`rustc.exe`/`cargo.exe` hashes are checked, rather than hashing only their
launcher shims. MSVC linker and Windows SDK files are still host dependencies
which M1 does not fully pin; this is not complete M0 reproducible qualification.

The current seed, formatter and raw historical reports are local ignored
artifacts. They have **not been published** as downloadable seeds. A fresh
clone therefore needs those exact independently obtained binaries to pass this
lock. A source rebuild can start from the frozen commit with the pinned Rust
toolchain (`cargo build --locked -p sgc -p sgfmt`), but it is a *new seed
candidate*: verify it and create a reviewed lock update instead of accepting
different bytes as the existing seed. No byte-identical source rebuild or
clean-machine qualification is claimed here.

The SG driver provides:

```text
bootstrap build SEED ROOT SOURCE OUTPUT
bootstrap check SEED ROOT SOURCE
bootstrap --version
```

Paths supplied to the runner are resolved before launch. Direct driver users
should supply absolute paths, create the output directory first, and expose
the verified clang directory on PATH plus `SENGOO_CLANG`. The driver validates
the seed hash itself, sets the named source runtime and fixed module map,
forwards child diagnostics and exit status, and applies a five-minute process
deadline. Exit 64 is invalid invocation, 65 is seed mismatch, 66 is input or
process failure, and 124 is timeout. Compiler failures keep their own status.

## Next gates

M0's local source-release reference and eight daily workload baselines are
frozen in `BASELINE.md` and `reference-baseline.lock`. Raw reports remain under
`target/selfhost/m0-frozen/` and `target/selfhost/reference-qualification/`.
Public seed distribution, installed SDKs and clean-machine qualification remain
separate work. M2's lexer/parser, macro/attribute/derive, protocol and formatter
gates passed, including 310 source cases and 1,748 transformation comparisons.
M3 completes semantic analysis and HIR/MIR; M4/M5 complete native code generation
and linking/debug symbols; M6 proves compiler self-rebuild; M7 finishes all
supported Windows tool/runtime behavior; M8 requires reproducible S2/S3 outputs,
two candidates and the installation/compatibility/performance gates. Default
cutover allows at most 10% time and 15% peak-memory regression on frozen daily
workloads, while preserving the existing performance gates. Other release
hosts continue to require their own evidence.

## Native development preview

`source_codegen_gate.py` builds `tools/native.sg` and runs 34 exact source
programs. Thirty-three execute with exit status 42 and identical stdout/stderr
through both the SG native and frozen qualified Rust/LLVM paths. The remaining
Result try fixture executes 42 through SG and preserves the reviewed reference
invalid-LLVM failure in `REFERENCE_GAPS.md`; it is not counted as equivalent.
Fixtures cover calls, control flow, generics, aggregate/float/numeric operations,
class/impl methods, try propagation, user Drop and Unicode source paths.
Repeated SG objects must have identical bytes; four deliberate failures preserve
existing output. The gate accepts `--seed`, `--formatter`, `--clang`, `--reference`
and `--output`; the reference identity comes from `reference-baseline.lock`.

After the gate, the development CLI can be invoked as:

```text
native.exe INPUT.sg OUTPUT.obj [--wrap]
```

The default checks integer add/subtract/multiply overflow. `--wrap` selects
wrapping arithmetic. Output diagnostics use JSON schema version 1. This preview
handles the qualified single-source subset and writes AMD64 COFF directly.
It is not yet the public `sgc` command interface. Resolved root and structural
cleanup, stack probing and unwind records have focused execution gates; pending
runtime glue, conditional field cleanup, async, full module/tool compatibility,
CodeView and complete native behavior remain open. Unsupported code reports a
failure; there is no automatic LLVM fallback.

`linker_gate.py` separately qualifies SG COFF/archive/PE linking, real Windows
SDK imports and independently observed Windows execution. `native_link_gate.py`
consumes passed source/linker evidence and rechecks every input/tool/artifact
hash before running SG source emission, SG linking and generated programs with
an empty PATH. It compares object and PE bytes across two fresh directories.
The SG numeric SDK fixture only provides verification exit hooks; this gate does
not rebuild the compiler itself or qualify the full runtime SDK.

Other focused gates are `syntax_gate.py`, `transform_gate.py`, `format_gate.py`,
`native_gate.py`, `layout_gate.py`, `object_gate.py` and `codegen_gate.py`.
Their exact local evidence paths and source/tool hashes are recorded in the
owning OpenSpec changes. Historical failed trials remain beside successful runs.
