## Context

Sengoo has broad unit/integration coverage and realworld fixtures, but the first
mainstream release needs explicit adversarial, longevity, compatibility, and
installed-toolchain evidence.

## Decisions

### Decision 1: Use layered hardening gates

- Per-commit: deterministic unit/integration tests and bounded fuzz smoke.
- Nightly: longer fuzzing, sanitizers, leak checks, async stress, and performance
  trend collection.
- Release: supported-host matrix, installed-toolchain realworld loop,
  compatibility corpus, signed artifacts, and zero unresolved release blockers.

### Decision 2: Fuzz public input boundaries first

Initial fuzz targets cover source lex/parse, type checking without codegen,
MIR lowering, manifest/lockfile parsing, registry metadata, package archive
extraction, JSON, and selected FFI decoders. Every fixed crash adds a retained
regression corpus entry or deterministic test.

### Decision 3: Treat native boundaries as unsafe until proven

C runtime sources, Rust `unsafe`, generated ABI calls, dynamic FFI, TLS, and
handle tables receive sanitizer/invalid-input coverage. A skip is recorded but
does not satisfy a supported-host release gate.

### Decision 4: Version source and runtime compatibility separately

The toolchain publishes:

- language/source version or edition;
- package manifest/lockfile schema versions;
- runtime ABI version embedded or checked at link/run boundaries;
- test/diagnostic JSON schema versions.

Compatibility tests compile a retained corpus with both current and previous
supported toolchains where the policy promises compatibility.

### Decision 5: Budgets use stable reference scenarios

Budgets are attached to committed benchmark inputs and reference runners. CI
uses percentage regression thresholds plus absolute safety ceilings. A budget
change requires recorded evidence, not silent snapshot replacement.

### Decision 6: Ecosystem proof uses released artifacts

Official package and flagship gates install the packaged toolchain into a clean
temporary prefix, resolve locked dependencies, then check/test/doc/build/run.
Workspace path leakage is a failure.

### Decision 7: Native runtime bundles carry one whole-runtime ABI version

`runtime_shared.h` is the source of truth for
`SENGOO_RUNTIME_ABI_VERSION`. `sgc` carries the ABI version it requires and
reads the selected runtime bundle's header before compiling cached runtime
objects or linking a native program. A missing declaration or mismatch fails
before unsafe execution and reports both required and available versions.

The shared header participates in the runtime bundle fingerprint, including
when a temporary `runtime.c` uses the canonical split runtime siblings. The C
runtime exposes `sengoo_runtime_abi_version()` for direct native probes. This
v1 gate versions the complete C/native bundle independently of narrower
descriptor schemas such as the collections ABI.

## Required host matrix

- Windows x64.
- Linux x86_64.
- macOS x86_64 and arm64 for release smoke.
- Additional ARM/Linux jobs may be added when runner/toolchain availability is
  stable; unsupported hosts remain explicit.

## Security boundaries

- registry authentication/name ownership/yank authorization;
- checksum/signature verification and replay behavior;
- archive path traversal, symlink, size, and decompression limits;
- TLS certificate/hostname validation without insecure success fallback;
- FFI pointer/length, generation handles, panic/unwind, and thread boundaries.

## Exit criteria

Production hardening archives only when release-host gates are green, known
skips are outside the declared support matrix, performance budgets are enforced,
and compatibility/security policies are published.
