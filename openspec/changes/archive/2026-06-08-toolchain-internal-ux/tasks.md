## 1. Prerequisites

- [x] 1.1 Archive `sgc-test-manifest-tooling` or record it as an explicit blocker.
- [x] 1.2 Run `openspec validate toolchain-internal-ux --strict`.
- [x] 1.3 Run `openspec validate --all --strict`.

## 2. Assertions

- [x] 2.1 Have `sgc test` create a unique assertion result path and pass it through `SENGOO_ASSERT_REPORT` in capture and `--nocapture` modes.
- [x] 2.2 Emit the bounded schema-version-1 JSON envelope from typed `std::assert` failures and remove any fd-3 assumption.
- [x] 2.3 Add compiler/runtime callsite plumbing for optional assertion `file` and numeric `line` fields.
- [x] 2.4 Parse, validate, size-limit, remove, and map the result file into `sgc test` text and JSON `assertion` output.
- [x] 2.5 Add Windows and POSIX tests for valid, missing, malformed, oversized, and unsupported-version envelopes (`assertion_transport` e2e).
- [x] 2.6 Preserve ordinary non-zero assertion termination when `SENGOO_ASSERT_REPORT` is absent.
- [x] 2.7 Migrate one realworld smoke test to assert helpers.

## 3. Real e2e and docs

- [x] 3.1 Add real `realworld_e2e` integration tests.
- [x] 3.2 Add CI job `realworld-e2e` without fake `sgc`/`sgfmt`.
- [x] 3.3 Add `docs/debugging-native.md`, `docs/editor-setup.md`, `docs/internal-release.md`.

## 4. Verification

- [x] 4.1 `cargo test -p sgc test` (9/9 commands::test unit tests; assertion_transport 2/2)
- [x] 4.2 `cargo test -p sgpm realworld` (1/1 realworld_e2e)
- [x] 4.3 `cargo test -p sglsp realworld` (6/6)

## Archive Gate

- [x] `sgc-test-manifest-tooling` archived before this child archives.
- [x] `openspec validate toolchain-internal-ux --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Canonical delta `tooling-mainstream-ecosystem` is complete.
- [x] `realworld-e2e` passes with real binaries or documents an evidenced toolchain skip.
