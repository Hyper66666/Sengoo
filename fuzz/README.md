# Bounded fuzz corpus

These inputs seed the deterministic per-commit hardening smoke. Inputs are
kept below 64 KiB so malformed data cannot turn the smoke gate into an
unbounded allocation test.

When a compiler, package-manager, archive, or runtime parser crash is fixed,
add the minimized input here or add a deterministic regression test that names
the original failure. Scheduled hardening jobs may run more generated cases,
but they must always replay this retained corpus first.
