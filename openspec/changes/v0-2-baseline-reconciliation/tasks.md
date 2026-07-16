## 1. Preserve current work

- [x] 1.1 Enumerate every worktree, branch, dirty file, untracked file, stash,
  patch, and remote-only commit relevant to Sengoo.
- [x] 1.2 Classify each item as merged, unique source, generated evidence,
  temporary artifact, or obsolete-with-proof.
- [x] 1.3 Checkpoint all unique source/doc/test changes before integration.
  - `origin/codex/sglsp-smart-completion-checkpoint` @ `444a154e0`
- [x] 1.4 Record branch/commit/patch locations in `INVENTORY.md`.

## 2. Reconcile capability ownership

- [x] 2.1 Recompute active-change status from `origin/main` and archives.
- [x] 2.2 Keep `native-debug-info` and `http-production-serving` as unique owners.
- [x] 2.3 Integrate or supersede `enhance-sglsp-smart-completion` without
  duplicating its protocol requirements.
  - Checkpointed on remote branch for M2 integration; not merged in M0.
- [x] 2.4 Archive, supersede, or narrow older umbrellas whose child work is done.
  - `mainstream-adoption-gap-closure` and `six-pillar-gap-closure` marked
    **Historical (v0.2 M0)** — no new ownership.
- [x] 2.5 Run strict validation after every ownership change.
  - `openspec validate --all --strict` → 52 passed.

## 3. Reconcile truth sources

- [x] 3.1 Update README install/version examples to the current published tag.
  - `v0.1.0-rc.1` in README.md and README.zh-CN.md.
- [x] 3.2 Reconcile `docs/language-reference.md` status rows with tests.
  - Subset/Experimental rows retained for open M1 surfaces; no false Supported.
- [x] 3.3 Reconcile `examples/realworld/SUPPORT_MATRIX.md` owners, status, and
  evidence links.
  - Portable row owners → archived experimental-scalar / NO-GO.
- [x] 3.4 Ensure no completed capability is described as active/reopened and no
  unproven capability is marked Supported.

## 4. Baseline verification

- [x] 4.1 `cargo fmt --check` (green).
- [x] 4.2 Workspace warnings-denied Clippy for production crates/tools (green;
  fixed `interest_count` cfg).
- [x] 4.3 Compiler, runtime, `sgc`, `sgpm`, `sgfmt`, and `sglsp` tests (green;
  restored LF for retained advanced-pipeline report + `bench/results/**/*.json -text`).
- [ ] 4.4 Installed realworld package loop on every supported release host
  (Actions on the baseline SHA after push).
- [ ] 4.5 Native safety, compatibility, and compile/resource performance gates
  (Actions on the baseline SHA after push; local pin hash restored).
- [x] 4.6 `openspec validate --all --strict` (52/52).
- [ ] 4.7 Record one common remote commit SHA and archive this change
  (after push + Actions green).
