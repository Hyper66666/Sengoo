## Stability classes

| Class | v0.2 meaning |
| --- | --- |
| Stable | Source/tool behavior preserved for all v0.2.x patch releases |
| Supported subset | Preserved within its documented bounds; unsupported cases fail explicitly |
| Experimental | No compatibility promise; opt-in and excluded from default-path claims |
| Deprecated | Still works in the current line, warns with replacement and removal horizon |

## Decisions

### D1: v0.2 uses edition 2026

Edition `2026` remains the accepted/default edition for v0.2. Patch releases do
not change grammar or Stable semantics within that edition. A future breaking
syntax change requires an accepted OpenSpec and either a later edition or an
explicit pre-1.0 migration gate; silent reinterpretation is forbidden.

### D2: Stable v0.2.x means patch compatibility

A source package, manifest, lockfile, diagnostic consumer, or runtime artifact
classified Stable and accepted by v0.2.0 must remain accepted by later v0.2.x
releases unless it depends on a security defect or unsound behavior. Such an
exception requires a security/correctness notice, stable diagnostic, and
migration path.

### D3: Deprecation is additive before removal

A Stable public surface is first marked Deprecated with a stable code,
replacement, and earliest removal version. It remains functional throughout the
current v0.2.x line. Removal requires a later minor/edition change and migration
tests. Warnings are suppressible only through a documented mechanism, not by
removing the diagnostic code.

### D4: Public input never produces an unclassified panic

Compiler source, manifests, lockfiles, packages/archives, protocol JSON, runtime
handles, and portable artifacts are untrusted public inputs. They return bounded
diagnostics/statuses. Unexpected internal failures are caught at CLI boundaries
where safe and reported as `internal-compiler-error` (or tool-specific stable
equivalent) with tool version and optional backtrace instructions; FFI never
unwinds across its boundary.

### D5: Versioned boundaries reject before dependent interpretation

Edition, schema, lockfile, diagnostic/test report, MIR semantic ABI, runtime ABI,
and portable runtime ABI versions are parsed and checked before consuming
version-dependent fields or executing code. Unknown explicit versions never
fall back to the newest known layout.

### D6: Two consecutive release candidates prove v0.2.0

Two candidate commits must pass the complete installed release, compatibility,
safety, performance, realworld, and OpenSpec matrices on every supported host.
A P0/P1 fix affecting Stable behavior after a successful candidate resets the
two-candidate count. Each candidate is retained as a compatibility fixture and
has checksums/provenance.

## Rollback contract

The previous published toolchain remains installable. Rollback reinstalls it,
verifies checksums, runs retained package fixtures without rewriting lockfiles,
and reports incompatible new artifacts rather than corrupting them.
