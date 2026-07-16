## 1. Preserve current work

- [ ] 1.1 Enumerate every worktree, branch, dirty file, untracked file, stash,
  patch, and remote-only commit relevant to Sengoo.
- [ ] 1.2 Classify each item as merged, unique source, generated evidence,
  temporary artifact, or obsolete-with-proof.
- [ ] 1.3 Checkpoint all unique source/doc/test changes before integration.
- [ ] 1.4 Record branch/commit/patch locations in `INVENTORY.md`.

## 2. Reconcile capability ownership

- [ ] 2.1 Recompute active-change status from `origin/main` and archives.
- [ ] 2.2 Keep `native-debug-info` and `http-production-serving` as unique owners.
- [ ] 2.3 Integrate or supersede `enhance-sglsp-smart-completion` without
  duplicating its protocol requirements.
- [ ] 2.4 Archive, supersede, or narrow older umbrellas whose child work is done.
- [ ] 2.5 Run strict validation after every ownership change.

## 3. Reconcile truth sources

- [ ] 3.1 Update README install/version examples to the current published tag.
- [ ] 3.2 Reconcile `docs/language-reference.md` status rows with tests.
- [ ] 3.3 Reconcile `examples/realworld/SUPPORT_MATRIX.md` owners, status, and
  evidence links.
- [ ] 3.4 Ensure no completed capability is described as active/reopened and no
  unproven capability is marked Supported.

## 4. Baseline verification

- [ ] 4.1 `cargo fmt --check`.
- [ ] 4.2 Workspace warnings-denied Clippy for production crates/tools.
- [ ] 4.3 Compiler, runtime, `sgc`, `sgpm`, `sgfmt`, and `sglsp` tests.
- [ ] 4.4 Installed realworld package loop on every supported release host.
- [ ] 4.5 Native safety, compatibility, and compile/resource performance gates.
- [ ] 4.6 `openspec validate --all --strict`.
- [ ] 4.7 Record one common remote commit SHA and archive this change.
