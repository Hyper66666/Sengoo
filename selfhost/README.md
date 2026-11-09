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

`source_codegen_gate.py` builds `tools/native.sg` and runs 49 exact source
programs. Forty-six execute with exit status 42 and identical stdout/stderr
through both the SG native and frozen qualified Rust/LLVM paths. The remaining
three fixtures execute 42 through SG and preserve the reviewed reference
source/invalid-LLVM failures in `REFERENCE_GAPS.md`; they are not counted as equivalent.
Fixtures cover calls, control flow, generics, aggregate/float/numeric operations,
class/impl methods, try propagation, user Drop, arrays and readonly string data.
Repeated SG objects must have identical bytes; five deliberate failures preserve
existing output. The gate accepts `--seed`, `--formatter`, `--clang`, `--reference`
and `--output`; the reference identity comes from `reference-baseline.lock`.

After the gate, the development CLI can be invoked as:

```text
native.exe INPUT.sg OUTPUT.obj [--wrap]
```

The default checks integer add/subtract/multiply overflow. `--wrap` selects
wrapping arithmetic. Output diagnostics use JSON schema version 1. The driver
loads physical import graphs, checks the combined program and writes AMD64
COFF directly. Module failures include the original file path and byte span;
failed compilation preserves the previous output. Module lookup uses relative
files, `SENGOO_STDLIB` and `SENGOO_MODULE_MAP` with the qualified resolver's
precedence and physical identity rules. Full namespaces and public module
compatibility remain open.
Before emission, combined MIR source metadata is validated and mapped back to
physical file IDs and original UTF-8 byte intervals. This keeps expansion and
specialization origins available to later consumers. `link_multi_debug_gate.py`
qualifies multiple physical files in one SG object/compile unit, including real
debugger source switching and local values. Independent COFF compile-unit
merging is qualified separately by `link_units_gate.py`: it preserves each
object's source registry and type-index namespace, merges PDB module streams,
and verifies real cross-object calls, same-named static functions and debugger
stepping. `mixeddebug_gate.py` additionally qualifies explicitly classified
nondebug SDK objects with their actual sections, relocations and address-only
public symbols. Such objects receive no invented source, type or line records;
at least one checked SG debug unit is required. Archive debug units and
automatic native symbol internalization remain open. The pinned LLDB 19 reader
does not resolve address-only C publics by name; exact C-address stops and
independent clang/lld controls record that debugger boundary separately.
It is not yet the public `sgc` command interface. Resolved root and structural
cleanup, stack probing and unwind records have focused execution gates; pending
runtime glue, conditional field cleanup, async, full module/tool compatibility,
CodeView and complete native behavior remain open. Unsupported code reports a
failure; there is no automatic LLVM fallback.

The source gate copies the compiler's complete SG import closure and C/stdlib
inputs before building. Its seed still discovers the development Rust runtime
in the current workspace, so the gate separately guards those Cargo/runtime
inputs. `module_codegen_gate.py --native-evidence REPORT --output DIRECTORY`
consumes the passed source report and its immutable snapshots to qualify
multi-file diagnostics, deterministic objects and repeated in-process resource
recovery. This development evidence does not establish compiler self-rebuild.
Its optional `--linker-evidence REPORT` revalidates a complete pinned SG linker
qualification, including copied historical source manifests when the checkout
has moved on. It then compiles and SG-links the five module graphs in two fresh
directories with an empty PATH, comparing object and PE bytes and execution.
This consumes the qualified linker binary rather than rebuilding current linker
sources or completing the runtime SDK.

`linker_gate.py` separately qualifies SG COFF/archive/PE linking, real Windows
SDK imports and independently observed Windows execution. `native_link_gate.py`
consumes passed source/linker evidence and rechecks every input/tool/artifact
hash before copying the qualified implementation sources into an immutable
test snapshot and running SG source emission, SG linking and generated programs with
an empty PATH. It compares object and PE bytes across two fresh directories.
The SG numeric SDK fixture only provides verification exit hooks; this gate does
not rebuild the compiler itself or qualify the full runtime SDK.

## SG runtime text foundation

`runtime/utf8.sg` provides allocation-free UTF-8 validation and validated byte
boundary queries through fixed C exports. The unsafe caller supplies a readable
byte range; lengths are bounded to 64 MiB. Empty, negative, oversized and
out-of-range requests follow the explicitly checked no-read rules. The C
adapter in `runtime/memory_read.c` performs only a raw byte load.

`verify_runtime_text.py --native-evidence REPORT --formatter-evidence REPORT
--linker-evidence REPORT --output DIRECTORY` qualifies the consumed SG compiler,
SG formatter and historical SG linker before testing immutable source copies.
Its strict Python decoder supplies scalar, malformed-sequence and byte-boundary
expectations. The C probe checks protected pages, input immutability and read
coverage, including the 64 MiB limit. Two fresh builds must produce identical
SG objects, normalized C SDK objects and SG-linked PE bytes and execute the
actual memory adapter with exit 42.

The development gate uses qualified clang to build the small adapter and its
independent probes. Its explicit C SDK metadata conversion preserves machine
code, data and relocation targets and records raw objects separately. A pinned
NTDLL `__chkstk` import supplies the SG compiler's large-frame stack probe;
this local OS dependency is outside final SDK packaging qualification.
The gate does not replace installed String storage or qualify
final SDK packaging, concurrency, performance or a self-building runtime.
The locked Rust formatter rejects these unsafe C export sources; this gate
uses the separately qualified SG formatter preview and records that boundary.
The local checkpoint is `target/selfhost/runtime-text-final-checkpoint/evidence.json`;
its source identities, corpus counts and remaining integration tasks are
recorded in `openspec/changes/add-selfhost-runtime-text-core/tasks.md`.

## SG raw DEFLATE foundation

`runtime/deflate.sg` implements stored, fixed and dynamic Huffman raw DEFLATE,
including overlapping copies and history across blocks. Its unsafe C export
`sengoo_sg_raw_inflate(source, input_length, target, capacity)` requires separate
readable/writable caller ranges, each bounded to 64 MiB. Success returns the
decoded byte count; failures are -1 for malformed/truncated/trailing streams,
-2 for exhausted output capacity and -3 for invalid limits. Failure may leave
an output prefix. Successful decoding consumes exactly the supplied raw stream
bytes, allowing unused padding in its final byte.

`verify_runtime_deflate.py --text-evidence REPORT --output DIRECTORY` consumes
the complete qualified UTF-8 tool/SDK branch and independently checks zlib
expectations, explicit format edge cases, protected ranges and actual 64 MiB
input/output limits. Two fresh artifact directories must produce identical
unmodified SG objects, normalized C memory/fixture objects and SG-linked PE
bytes. The recorded checkpoint is
`target/selfhost/runtime-deflate-final-checkpoint/evidence.json`.

The corpus retains the explicit HDIST 31/32 format/oracle differences and the
duplicate length-258 encoding compatibility case. Gzip framing, tar/package
integration, final SDK packaging, self-building and performance remain open;
this raw decoder does not replace the installed package manager.

Other focused gates are `syntax_gate.py`, `transform_gate.py`, `format_gate.py`,
`native_gate.py`, `layout_gate.py`, `object_gate.py` and `codegen_gate.py`.
Their exact local evidence paths and source/tool hashes are recorded in the
owning OpenSpec changes. Historical failed trials remain beside successful runs.
