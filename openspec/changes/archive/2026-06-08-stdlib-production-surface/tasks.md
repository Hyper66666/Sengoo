## 1. Preparation

- [x] 1.1 Run `openspec validate stdlib-production-surface --strict`.
- [x] 1.2 Run `openspec validate --all --strict`.

## 2. Implementation

- [x] 2.1 Add `_string` path/directory helpers per `stdlib-mainstream-usability` delta.
- [x] 2.2 Add owned JSON string reads and 1 MiB cap.
- [x] 2.3 Implement `Vec<String>` and `StringMapString`.
- [x] 2.4 Add recursive IO and process pipe/background APIs.
- [x] 2.5 Add sync fd IO helpers.

## 3. Verification

- [x] 3.1 `cargo test -p sengoo-compiler --lib stdlib_` (62/62).
- [x] 3.2 `cargo test -p sgc stdlib_ -- --nocapture --test-threads=1` (104/104), including real `pipe_stdout_to` stdin transport.
- [x] 3.3 `cargo test -p sglsp stdlib_` (21/21).
- [x] 3.4 Update `SUPPORT_MATRIX.md` rows owned by Pillar 1.

## Archive Gate

- [x] `openspec validate stdlib-production-surface --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Canonical deltas `stdlib-mainstream-usability` and `owned-string-text` are complete and ready to promote on archive.
- [x] Verification commands in section 3 pass.
