## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for runtime-source cache fingerprinting.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [ ] 2.1 Add run-cache metadata coverage for runtime fingerprint drift.
- [ ] 2.2 Add build-cache metadata coverage for runtime fingerprint drift.
- [ ] 2.3 Add cache-miss diagnostic coverage for runtime-source drift.
- [ ] 2.4 Add runtime object-cache identity coverage for equal-length byte changes.

## 3. Implementation

- [ ] 3.1 Add serde-compatible runtime fingerprint fields to run/build metadata and keys.
- [ ] 3.2 Compute runtime fingerprints in `sgc run` and `sgc build`.
- [ ] 3.3 Require runtime fingerprint matches before artifact reuse and report drift clearly.
- [ ] 3.4 Key cached runtime objects by streamed source-byte fingerprint.

## 4. Verification

- [ ] 4.1 Run focused red/green cache fingerprint tests.
- [ ] 4.2 Run `cargo fmt --check`.
- [ ] 4.3 Run `cargo test -p sgc`.
- [ ] 4.4 Run `cargo clippy -p sgc --all-targets -- -D warnings`.
- [ ] 4.5 Run `cmd /c openspec validate sgc-runtime-cache-fingerprint --strict`.
- [ ] 4.6 Run `git diff --check`.
