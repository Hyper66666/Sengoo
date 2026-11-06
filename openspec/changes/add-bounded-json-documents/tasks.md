- [x] Reproduce large legacy input rejection before runtime changes (`json_large_legacy_limit.sg`; `target/file-media/json-legacy-run.log`).
- [x] Add document-scoped parse/build/copy/serialization limits with aggregate admission.
- [x] Add public SG and native rejected-pointer/cleanup regressions (`json_bounded` integration suite: 2 passed).
- [x] Verify legacy Unicode/copy behavior (`json_unicode`: 2 passed) and document evidence in `docs/bounded-json-documents.md`.
- [x] Integrate provider request through its separate parent-owned loopback fixture (three protocols, >1 MiB PNG and preflight refusal evidence in Sencode `target/provider-media/native2.log`).
- [ ] Obtain clean native sanitizer and other required platform qualification; Windows ASan exits zero but reports an interception warning.

`new_string_owned(&String)` preserves an owned string's stored length, including
NUL. Borrowed `&str` views retain the existing C-terminated representation and
cannot promise suffix preservation after NUL. No legacy ABI constant was raised.
