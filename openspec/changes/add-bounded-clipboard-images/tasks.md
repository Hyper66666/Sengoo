## Implementation

- [x] Add public owning String wrappers and Windows read-only clipboard functions (owner: Sengoo foundation).
- [x] Add bounded image conversion, exclusive PNG writes, and image-source ownership cleanup.
- [x] Preserve original file paths in a single CF_HDROP JSON snapshot; leave path-copy policy to Sengoo callers.

## Verification

- [x] Run isolated clipboard fixture with real GDI+/COM/files, Unicode text and files, malformed/oversize input, no overwrite, and stable handle counts (`target/clipboard-image/native.log`).
- [x] Retain a private-window-station probe; record actual host denial (exit 77, CreateWindowStation error 5) without touching the interactive clipboard (`target/clipboard-image/station.log`).
- [x] Run public Sengoo API/NUL rejection and previous clipboard-copy regression; the independent Rust test harness passed 2/2 using the pinned compiler (`target/clipboard-image/rust-tests.log`).
- [x] Validate this OpenSpec change and all active changes strictly (65/65 on 2026-11-05).
- [x] Record full baseline revision, host, commands and source/binary hashes (`target/clipboard-image/evidence.json`); freeze the source for CLI integration.

## External acceptance

- [ ] Run real clipboard integration on a host allowing an isolated private station; host denial is not a pass.
