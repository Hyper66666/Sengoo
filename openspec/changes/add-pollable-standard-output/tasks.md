## Generic I/O owner

- [x] Inspect current synchronous I/O and owned child-stdin write boundaries;
  record cancellation, platform, capacity and reclamation design constraints.
- [x] Implement additive StdWriter ownership, generation checks and status API
  without changing existing synchronous entrypoints.
- [x] Implement bounded Windows local byte-pipe workers with independent
  duplicated handles, cancellation race handling and actual join on cleanup.
- [x] Implement capacity reservation and live writer/thread/handle/byte accounting,
  including constructor failure rollback and unsupported endpoint diagnostics.
- [x] Add real blocked-reader, cancellation race, partial write, binary fidelity,
  stale handle and repeated resource baseline native regressions.
- [x] Run formatter, language/native tests and Windows host gates; document exact
  candidate SHA, timings and remaining platform limits before marking supported.
- [x] Keep the initial POSIX backend explicitly unsupported and record its host
  gates as unverified; implementing that backend is a separate future change.

## Ecosystem integration owner

- [x] Switch the optional MCP stdio adapter to the verified generic writer while
  retaining its bounded outbox, application permissions and protocol state.
- [x] Verify blocked-output concurrent input/cancel against an independent MCP
  client, then run locked external consumers and distribution compatibility gates.

## Design validation

- [x] Pass strict OpenSpec validation for this change. This validates the proposal
  format only and does not constitute implementation or platform evidence.
