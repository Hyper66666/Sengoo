## Implementation and qualification

- [x] Reproduce native01 with independent called/uncalled and std::option cases.
- [x] Instrument the exact invalid-local branch without bypassing validation.
- [x] Read current language formatting contract and frozen reference/runtime paths.
- [ ] Implement source-bound builtin identity validation in an independent snapshot.
- [ ] Implement i64/shared-str typed runtime calls with separate C ABI signatures.
- [ ] Execute exact stdout/stderr, Unicode and embedded-NUL differential tests.
- [ ] Execute HIR identity, AST, source and callable-signature mutation rejections.
- [ ] Verify same-name ordinary functions retain their own behavior.
- [ ] Report the next real std::option/std::file frontier without suppressing bodies.
- [x] Register the reviewed add-selfhost-print-intrinsics child before production integration.
- [ ] Run existing HIR/MIR, ownership and native/source regressions against the exact integrated producer.
- [ ] Verify first-call resource recovery, exact limits and preserved outputs on failure.
- [ ] Freeze source/tool/runtime/header/library and command receipts, record full revision/host, and pass child/all strict validation before committing.
