## 1. Classify and document surfaces

- [ ] 1.1 Inventory language, stdlib, CLI, manifest, lockfile, diagnostics,
  runtime ABI, portable ABI, and editor protocol surfaces.
- [ ] 1.2 Mark each Stable, Supported subset, Experimental, or Deprecated in the
  authoritative reference/support policies.
- [ ] 1.3 Pin edition 2026 and v0.2.x patch compatibility rules.

## 2. Deprecation and migration

- [ ] 2.1 Define deprecation metadata, stable warning code, replacement text,
  earliest removal version, and documented suppression policy.
- [ ] 2.2 Add compiler/JSON/LSP tests for deprecated language/stdlib surfaces.
- [ ] 2.3 Add a migration guide for every v0.1 -> v0.2 source/tool behavior change,
  including `chars()` item typing.
- [ ] 2.4 Prove deprecated v0.2 surfaces remain functional throughout v0.2.x.

## 3. Version boundary enforcement

- [ ] 3.1 Test supported/unknown edition diagnostics.
- [ ] 3.2 Test manifest, lockfile, registry, diagnostic JSON, test JSON, and LSP
  protocol schema rejection before version-dependent parsing.
- [ ] 3.3 Test MIR semantic, native runtime, and portable ABI mismatch rejection
  before linking/execution.
- [ ] 3.4 Retain v0.1.0-rc.1 and every v0.2 candidate fixture outside the source
  checkout path.

## 4. Panic and safety policy

- [ ] 4.1 Inventory public input boundaries and existing panic/unwrap sites.
- [ ] 4.2 Add bounded fuzz/regression cases for source, manifest, lockfile,
  archive, JSON protocol, runtime handle, and portable artifact inputs.
- [ ] 4.3 Convert public-input panics to stable diagnostics/statuses and ensure
  FFI never unwinds.
- [ ] 4.4 Add bounded internal-error envelopes with tool version and opt-in
  backtrace guidance; no raw panic is counted as a valid user diagnostic.

## 5. Release-candidate and rollback gates

- [ ] 5.1 Produce candidate 1 installed artifacts, checksums, provenance, and
  four-host evidence; retain its fixture.
- [ ] 5.2 Produce candidate 2 from a later commit with the same complete gate; if
  a P0/P1 Stable-behavior fix lands, restart at candidate 1.
- [ ] 5.3 Test upgrade from v0.1.0-rc.1 and candidate 1, plus rollback to the
  previous published archive without lockfile rewriting.
- [ ] 5.4 Publish migration, compatibility, support, security/correctness
  exception, and rollback documentation.

## 6. Verification and archive

- [ ] 6.1 Run compatibility, fuzz, sanitizer/leak, performance/resource, and
  installed realworld matrices on the final candidate SHA.
- [ ] 6.2 Run full workspace formatting, warnings-denied Clippy, and tests.
- [ ] 6.3 Run strict OpenSpec validation.
- [ ] 6.4 Update the language reference and support matrix with final stability
  classes and evidence, then archive this change.
