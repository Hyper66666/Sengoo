## 0. Program setup

- [x] 0.1 Freeze the product direction: Go-like default workflow, Rust-like
  resource safety, and Python interoperability on the production native path.
- [x] 0.2 Split M0-M4 into five independently archivable child changes.
- [x] 0.3 Add `INVENTORY.md` after M0 records the integrated mainline SHA,
  active owners, archived dependencies, and support-matrix baseline.
  - See `v0-2-baseline-reconciliation/INVENTORY.md` (sglsp checkpoint + owners).
- [x] 0.4 Validate this umbrella and every child with strict OpenSpec validation.
  - `openspec validate --all --strict` → 52 passed (after delta-spec body fixes).

## 1. M0 - Baseline reconciliation

- [~] 1.1 Complete and archive `v0-2-baseline-reconciliation`.
  - Local gates green; archive after remote multi-host Actions on baseline SHA.
- [x] 1.2 Reconcile unmerged valuable work, active changes, documentation, and
  support claims without destructive history rewriting.
- [~] 1.3 Record one green baseline SHA and remote location.
  - Filled when this branch is pushed and Actions attach to the SHA.

## 2. M1 - Language coherence

- [ ] 2.1 Complete and archive `v0-2-language-coherence`.
- [ ] 2.2 Prove ownership/borrow, match/trait, array, and control-flow contracts
  through compiler positive/negative tests and native runtime evidence.
- [ ] 2.3 Update `docs/language-reference.md` statuses and proof links.

## 3. M2 - Developer loop

- [ ] 3.1 Integrate or supersede `enhance-sglsp-smart-completion` with preserved
  protocol/performance evidence.
- [ ] 3.2 Complete and archive `native-debug-info`.
- [ ] 3.3 Complete and archive `v0-2-developer-loop`.
- [ ] 3.4 Prove one installed package edit/navigate/rename/format/test/debug/doc
  loop through real tool binaries.

## 4. M3 - Production standard library

- [ ] 4.1 Complete and archive `http-production-serving`.
- [ ] 4.2 Complete and archive `v0-2-production-stdlib`.
- [ ] 4.3 Prove bounded stream and Unicode baseline behavior through realworld
  fixtures without weakening existing Buffer/String compatibility.

## 5. M4 - Stability contract

- [ ] 5.1 Complete and archive `v0-2-stability-contract`.
- [ ] 5.2 Retain previous-release fixtures and pass two consecutive v0.2 release
  candidate compatibility matrices.
- [ ] 5.3 Publish migration guidance and the final v0.2 support boundary.

## 6. Final integration

- [ ] 6.1 Run formatting and warnings-denied lint for all workspace crates.
- [ ] 6.2 Run compiler, runtime, `sgc`, `sgpm`, `sgfmt`, and `sglsp` tests.
- [ ] 6.3 Run native sanitizer/leak, compatibility, performance, and fuzz gates.
- [ ] 6.4 Run installed release and every reviewed realworld package loop on all
  supported hosts.
- [ ] 6.5 Run `openspec validate --all --strict` on the same commit SHA.
- [ ] 6.6 Reconcile README, language reference, compatibility policy, and
  `SUPPORT_MATRIX.md`, then archive this umbrella.
