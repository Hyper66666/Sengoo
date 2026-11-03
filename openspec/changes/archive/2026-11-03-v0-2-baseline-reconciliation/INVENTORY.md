# v0.2 Baseline Inventory

Snapshot date: **2026-07-16**.

| Field | Value |
| --- | --- |
| Inventory base | `origin/main` @ `7e9e4d910` (merge PR #45 wasm capacity harden) |
| v0.2 OpenSpec branch | `codex/sengoo-v0-2-openspec` @ `7dbf60813` (+ this M0 commit) |
| Published install tag | `v0.1.0-rc.1` |
| Product path | Production **native** LLVM-text; experimental WASM/bytecode out of M0–M4 scope |

## 1. Worktree / branch classification

| Location | Branch / SHA | Classification | Action |
| --- | --- | --- | --- |
| `D:\Sengoo` (primary) | was dirty on `codex/toolchain-transcript-evidence` | **Unique source** (sglsp/sgfmt) | Checkpointed to `codex/sglsp-smart-completion-checkpoint` @ `444a154e0` |
| `.worktrees/mainstream-adoption-next` | `codex/sengoo-v0-2-openspec` | **Active v0.2 lane** | Continue M0–M4 here |
| `.worktrees/production-hardening-v1` | `codex/wasm-malformed-capacity-fix` @ `791639db0` | **Merged** via PR #45 | No further action |
| `.worktrees/senline-service-dogfood` | `codex/senline-service-dogfood` | Unique product dogfood | Outside v0.2 core; leave on branch |
| `.worktrees/debugger-live-composites` | `codex/debugger-live-composites` | Unique debug evidence | Consumed by `native-debug-info` / M2 |
| Other `.worktrees/*` listed in M0 snapshot | various `codex/*` | **Merged or obsolete-with-proof** relative to maturity roadmap | Do not delete; not M0 blockers |
| `stash@{0}` / `stash@{1}` | local stashes | Unknown age | Leave; not used as evidence |

### Unique source checkpoint (required by M0)

| Item | Remote branch | SHA | Notes |
| --- | --- | --- | --- |
| `enhance-sglsp-smart-completion` + implementation | `origin/codex/sglsp-smart-completion-checkpoint` | `444a154e0` | OpenSpec change + `tools/sglsp` workspace index/completion/protocol goldens + `sgfmt` import fixtures + editor docs. **Not merged to main.** M2 integrates or supersedes. |

## 2. Active OpenSpec ownership map (post-M0)

| Change | Role | M0 disposition |
| --- | --- | --- |
| `sengoo-v0-2-mainstream-core` | v0.2 umbrella | Active coordinator |
| `v0-2-baseline-reconciliation` | M0 child | **This change** — archive when baseline SHA green |
| `v0-2-language-coherence` | M1 | Active after M0 |
| `v0-2-developer-loop` | M2 | Active; consumes debug + sglsp checkpoint |
| `v0-2-production-stdlib` | M3 | Active; consumes HTTP owner |
| `v0-2-stability-contract` | M4 | Active; fixtures from M0 |
| `native-debug-info` | Unique debug metadata owner | **Retain** — M2 consumer |
| `http-production-serving` | Unique production HTTP owner | **Retain** — M3 consumer |
| `enhance-sglsp-smart-completion` | On checkpoint branch only | **Not on main**; M2 must integrate or supersede with evidence |
| `mainstream-adoption-gap-closure` | Legacy umbrella (open tasks remain) | **Historical** — superseded by v0.2 program for new work; do not add new tasks |
| `six-pillar-gap-closure` | Legacy internal umbrella | **Historical** — evidence lineage only; new work under v0.2 children |

Archived maturity program (do not reopen for native v0.2 claims):

- `2026-07-15-language-maturity-roadmap`
- `2026-07-15-wasm-backend-v1` (experimental scalar only)
- `2026-07-15-bytecode-vm-v1` (NO-GO)
- `2026-07-15-wasm-and-bytecode-backends`
- Phases 0–4 native maturity children under `2026-07-1*` archives

## 3. Truth-source corrections applied in M0

| Source | Issue | Correction |
| --- | --- | --- |
| `examples/realworld/SUPPORT_MATRIX.md` | Portable row said `wasm-backend-v1 (reopened)` | Owner set to **archived** experimental scalar |
| `README.md` / `README.zh-CN.md` | Install examples used bare `0.1.0`; claimed no public tag | Use **`v0.1.0-rc.1`** / `--version 0.1.0-rc.1`; note published prerelease |
| `docs/language-reference.md` | Status rows left as-is where Subset/Experimental match open M1 work | No false Supported promotions in M0 |

## 4. Capability → owner (v0.2 starting set)

| Capability | Active owner | Evidence level |
| --- | --- | --- |
| Program order / archive gate | `sengoo-v0-2-mainstream-core` | OpenSpec + matrix |
| Baseline truth / single SHA | `v0-2-baseline-reconciliation` | This inventory + gate log |
| Borrow/Drop/match/traits/arrays | `v0-2-language-coherence` | Pending M1 |
| LSP/fmt/test/debug loop | `v0-2-developer-loop` (+ `native-debug-info`, sglsp checkpoint) | Pending M2 |
| HTTP production surface | `http-production-serving` | Open owner |
| Streams / Unicode baseline | `v0-2-production-stdlib` | Pending M3 |
| Edition/compat/panic/ABI RC | `v0-2-stability-contract` | Pending M4 |
| Experimental WASM | **none for production** | Archived experimental-scalar only |

## 5. Baseline verification record

Local Windows reference workspace (2026-07-16), to be re-run on the pushed SHA:

| Gate | Result | Notes |
| --- | --- | --- |
| `cargo fmt --check` | **green** | |
| Clippy `-D warnings` (`compiler`/`runtime`/`sgc`/`sgpm`/`sgfmt`/`sglsp`) | **green** | `interest_count` gated to `native-bridge` tests |
| `cargo test -p sengoo-compiler --lib` | **green** | 1101 passed |
| `cargo test -p sengoo-runtime --lib` | **green** | 69 passed |
| `cargo test -p sgc --test portable_targets` | **green** | 13 passed |
| `cargo test -p sgpm` | **green** | fixed retained report LF + `.gitattributes` |
| `cargo test -p sgfmt` | **green** | 3 passed |
| `cargo test -p sglsp` | **green** | 81 passed on mainline (checkpoint branch separate) |
| `openspec validate --all --strict` | **green** | 52/52 after delta-spec body SHALL fixes |
| Realworld / multi-host safety/perf | **Actions** | Attach to pushed remote SHA |
| **Baseline SHA** | `219a80b822afbf4c2eb24953aab6efbb11e5fb34` | `origin/codex/sengoo-v0-2-openspec` / PR #46 |

## 6. Non-destructive policy

- No force-push, hard reset, or deletion of foreign worktrees/branches in M0.
- Generated `target/`, `*.ll`, lock caches are never evidence.
- Stale checkboxes without tests remain open; implemented behavior without tests is not marked Supported.
