## Program setup

- [x] Record published pre-self-hosting checkpoint `cc70247ec5bd9531bea751a08288980da40ecc68` and separate preview/release claims.
- [x] Assign the first implementation child and reserve later ownership without duplicating behavioral requirements.
- [x] Define M0-M8, final S2/S3 equality and 10% time/15% RSS cutover limits.
- [x] Validate this umbrella using `openspec validate self-hosting-program --strict` (passed on Windows x64, 2026-11-09).

## Integration milestones

- [ ] M0: complete the seed/source/tool identity lock, supported corpus and measured baseline freeze with named evidence.
- [x] M1: complete and verify `add-selfhost-foundations` (Windows x64 44-step source-hashed gate, `target/selfhost/m1-run2/evidence.json`; M0 measured-baseline/seed-distribution qualification remains open).
- [ ] M2: register and verify the SG syntax/formatter child.
- [ ] M3: register and verify the semantic/HIR/MIR child.
- [ ] M4: register and verify the Windows x64 native-backend child.
- [ ] M5: register and verify the SG linker and native-debug child.
- [ ] M6: prove first compiler-only native fixed point while preserving the compatibility gap ledger.
- [ ] M7: finish all four SG tools, SG runtime and Windows supported compatibility.
- [ ] M8: verify full-toolchain fixed point, performance, two candidates, installation and rollback before default cutover.

## Continuing evidence

- [ ] Publish a full revision/host/command/result ledger for every claimed milestone.
- [ ] Repeat final self-build in different directories and empty caches with subprocess evidence.
- [ ] Run the existing compatibility/resource/performance gates alongside self-hosting gates.
- [ ] Preserve four-host stable release qualification and separate future Linux/macOS self-hosting work.
- [ ] Run `openspec validate --all --strict` and reconcile canonical specs before archiving completed children.
