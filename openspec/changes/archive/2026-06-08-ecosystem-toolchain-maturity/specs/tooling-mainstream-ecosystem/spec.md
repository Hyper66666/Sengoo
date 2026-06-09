## ADDED Requirements

### Requirement: sgc documents and supports cross-compilation for reference targets

`sgc build` SHALL accept an explicit `--target <triple>` flag for supported host
pairs and SHALL document required SDK/sysroot environment variables.

#### Scenario: Windows host builds Linux gnu triple with documented sysroot

- **WHEN** a developer runs `sgc build main.sg --target x86_64-unknown-linux-gnu`
  with the documented sysroot environment on the reference Windows host
- **THEN** the build produces a runnable Linux artifact or a documented linker error
  with remediation steps
- **AND** `docs/cross-compilation.md` lists supported triples and env vars

#### Scenario: Unsupported triple fails with actionable diagnostic

- **WHEN** a developer passes an unsupported `--target` triple
- **THEN** `sgc` exits non-zero with a diagnostic naming the triple and pointing to
  `docs/cross-compilation.md`

### Requirement: sgc emits machine-readable compile timings

`sgc build` SHALL support `--timings-json <path>` exporting schema-version-1 phase
timings aligned with `frontend-compile-perf` phase names.

#### Scenario: Timings JSON includes frontend sub-phases

- **WHEN** `sgc build --timings-json out.json` completes a native build
- **THEN** `out.json` contains per-phase milliseconds for parse, typeck, hir_lower,
  mir_lower, mir_opt, and codegen
- **AND** the schema version field is integer `1`

### Requirement: sglsp resolves symbols across dependency sources

`sglsp` SHALL provide go-to-definition for symbols defined in direct path or git
dependencies resolved by `sgpm` in the current workspace graph.

#### Scenario: Go-to-definition reaches a path dependency module

- **WHEN** a workspace imports a symbol from a path dependency and the editor requests
  go-to-definition
- **THEN** `sglsp` opens the dependency source location
- **AND** missing sources produce a stable diagnostic rather than a silent no-op
