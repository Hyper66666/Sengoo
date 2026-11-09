# Stage0 foundation design

## Inputs and identity

Start from `cc70247ec5bd9531bea751a08288980da40ecc68` and record the exact seed
binary SHA-256, source reference, target, toolchain/runtime identities and
bootstrap options in `selfhost/bootstrap.lock`. Local paths are supplied by
the caller rather than treated as a universally installed compiler location.
Runtime/source fingerprint normalization must be explicit so line-ending
conversion cannot silently qualify different source contents.

The initial integration exposed an ANSI synchronous Windows launch. Its
separate `fix-windows-process-utf8-launch` correction is frozen as
`fc1a686a2953adcbd14731cc6f93270cfb9cf34d` in the source lock, retaining the
same seed binary identity. The failed Unicode fixture remains a required gate.

The stage0 procedure validates required versions/identities before executing a
compiler. Missing inputs and identity mismatches fail clearly; they must not
select another executable from PATH or claim a successful self-build. An
intentional seed refresh updates the lock together with its reason and
regression evidence. Do not publish binaries, machine-specific secrets or build
caches as source inputs.

The stage0 native dependency description records the current source runtime and
external-tool requirements honestly. It must not label the existing complete
C/Rust runtime as the final minimal platform SDK. Later SDK extraction has its
own child and verification.

## SG foundation

Use owned source buffers with file IDs and byte-oriented spans. Span validation
checks the file identity and ordered offsets against the associated source
length. Source storage must not manufacture a valid empty file when reading
fails, and it must support empty input and normal UTF-8 bytes without turning
byte offsets into Unicode character offsets.

Use deterministic symbol interning: repeated byte-identical names receive the
same symbol ID, distinct names retain distinct identities, and resolution
rejects invalid IDs. Stable IDs follow explicit insertion order, independent
of randomized map iteration.

Use append-only node storage addressed by checked integer IDs; no pointer or
borrow to an element may survive a storage growth as an undocumented guarantee.
Existing IDs continue to resolve after growth, invalid IDs fail, and repeated
construction/destruction must return tracked resources to baseline. These
structures initially build on the current runtime-backed SG collections;
their implementation does not prove the runtime algorithms are already SG.

Compiler-specific growth must have explicit bounds and overflow/error handling;
the foundation must not increase global JSON/compression/runtime limits to make
a compiler test pass. A bounded first implementation documents its supported
limit and tests at and beyond that limit. Later scale work may replace its
storage without changing IDs or source-position meaning.

## Entry and verification

The SG stage0 entry exposes a deterministic foundation probe/self-test and a
clear usage/error result for malformed invocation. The surrounding seed runner
is bootstrap tooling only. It may build and execute the SG entry and SG test
programs, but it must not implement the compiler data structures in the host
language or report a wrapper launch as a self-hosted compiler.

The verifier records host, full source revision, relevant source hashes, locked
and actual tool identities, exact commands, exit codes and output. Run the
compiler with the explicit runtime mode when source overrides are present.
Evidence files and native outputs go to ignored build locations. Preserve
negative-test failures separately from expected success so a rejected bad seed
cannot appear as a failing positive build or vice versa.

Required checks cover source reading (including failure), empty and UTF-8 data,
invalid and boundary spans, duplicate/distinct/invalid symbols, arena growth and
invalid indices, bounded overflow, repeated lifecycle resource recovery, bad
CLI input and missing/tampered seed identity. Record implementation limits and
do not infer M2-M8 status from these passes.
