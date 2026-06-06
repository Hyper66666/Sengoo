# Mainstream Usable Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Sengoo's realworld package workflow demonstrable and verifiable through committed examples, locked sgpm commands, CLI/LSP diagnostics, support matrix docs, and OpenSpec-backed acceptance tests.

**Architecture:** Treat `mainstream-usable-loop` as an integration lane over existing stdlib/runtime/tooling capabilities. First land committed `examples/realworld` fixtures, then wire sgpm/sgc/sglsp tests around those fixtures, and finally update user-facing docs to point at the support matrix as the single source of truth.

**Tech Stack:** Rust workspace (`sgc`, `sgpm`, `sgfmt`, `sglsp`, `sengoo-compiler`, `sengoo-runtime`), Sengoo source packages, OpenSpec, Markdown docs.

---

### Task 1: Realworld Fixture Baseline

**Files:**
- Create: `examples/realworld/README.md`
- Create: `examples/realworld/SUPPORT_MATRIX.md`
- Create: `examples/realworld/cli-json-audit/**`
- Create: `examples/realworld/http-client-status/**`
- Create: `examples/realworld/workspace-doc-loop/**`
- Modify: `examples/README.md`
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: Create committed package fixtures**

Add exactly these packages:

- `cli-json-audit`: single package with `Sengoo.toml`, `src/main.sg`, `tests/audit_smoke.sg`, sample data, and docs. It covers `std::args`, `std::file`, `std::dir`, `std::json`, `std::log`, `std::status`, and `std::collections`.
- `http-client-status`: single package with `Sengoo.toml`, `src/main.sg`, `tests/http_status_smoke.sg`, and docs. It imports public `std::http` and `std::log`, covers JSON/status handling, and uses supported or explicit unsupported HTTP behavior.
- `workspace-doc-loop`: workspace or dual-target fixture with root `Sengoo.toml`, at least one `[lib]`, tests, docs, and `std::process` invocation.

**Step 2: Add the support matrix**

Create `examples/realworld/SUPPORT_MATRIX.md` with this required table shape:

```markdown
| Capability | Status | Host scope | Proof example/test | Stable diagnostic/status | Upstream spec/change |
```

Cover async IO, task cancellation, select limitations, process cancellation/background execution, compression, TLS/HTTP, dynamic FFI, package/test/doc diagnostics, and LSP coverage.

**Step 3: Smoke the first fixture locally**

Run from each fixture directory once it exists:

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

Expected: commands pass, or a documented platform-specific unsupported path is asserted by the example and matrix.

### Task 2: sgpm Locked Loop Coverage

**Files:**
- Modify: `tools/sgpm/tests/integration.rs`
- Modify only if needed: `tools/sgpm/src/runner.rs`
- Modify only if needed: `tools/sgpm/src/lockfile.rs`
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: Add realworld fixture integration helpers**

Add helpers that locate committed fixtures under `examples/realworld`, copy them to temporary test directories when mutation is required, and run `sgpm update` plus locked commands.

**Step 2: Assert lockfiles are not rewritten by locked commands**

After `sgpm update`, capture `Sengoo.lock` contents. Run `sgpm check --locked`, `sgpm test --locked`, `sgpm fmt --check --locked`, `sgpm doc --locked`, and `sgpm build --locked`. Re-read `Sengoo.lock` and assert content equality.

**Step 3: Add failure-path tests**

Cover at least stale lockfile, manifest/package context preservation, and unsupported feature/status reporting where practical.

**Step 4: Run focused tests**

```powershell
cargo test -p sgpm realworld -- --nocapture
```

Expected: new realworld locked-loop tests pass.

### Task 3: sgc, stdlib, and Diagnostics Coverage

**Files:**
- Modify: `tools/sgc/src/tests.rs`
- Modify: `compiler/src/tests/stdlib_surface_tests.rs` only if a realworld-used stdlib surface lacks compiler coverage
- Modify: `tools/sgc/src/runtime_hardening_tests.rs` only if unsupported runtime behavior lacks coverage
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: Add sgc coverage for realworld entries**

Add checks/builds/runs or reduced fixtures proving `sgc` handles the realworld import sets and structured diagnostics.

**Step 2: Add missing stdlib coverage only when evidence shows a gap**

Prefer existing stdlib examples/tests. Add new tests only for behavior uniquely exercised by realworld packages.

**Step 3: Run focused tests**

```powershell
cargo test -p sgc realworld -- --nocapture
cargo test -p sgc stdlib_ -- --nocapture
cargo test -p sengoo-compiler --lib stdlib_
```

Expected: realworld/import/stdlib coverage passes.

### Task 4: LSP Realworld Import Coverage

**Files:**
- Modify: `tools/sglsp/src/stdlib.rs`
- Modify: `tools/sglsp/src/main.rs` only if existing reduced-fixture tests cannot cover the requirement
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: Add package-derived reduced fixtures**

Use fixture names that trace back to the examples, such as `realworld_cli_json_audit_imports`, `realworld_http_client_status_imports`, and `realworld_workspace_doc_loop_imports`.

**Step 2: Cover completion, hover, signature help, definition, diagnostics, and formatting**

Reuse existing `sglsp` test helpers where possible. Avoid introducing a full package harness unless reduced fixtures cannot represent the import behavior.

**Step 3: Run focused tests**

```powershell
cargo test -p sglsp realworld
cargo test -p sglsp stdlib_
```

Expected: new and existing `sglsp` tests pass.

### Task 5: User-Facing Docs

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/sgpm-quickstart.md`
- Modify: `examples/README.md`
- Modify: `examples/realworld/README.md`
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: Link the realworld workflow**

Add concise entry points from the README, Chinese README, quickstart, and examples index to `examples/realworld`.

**Step 2: Keep support semantics in one place**

Link `examples/realworld/SUPPORT_MATRIX.md` instead of duplicating the matrix in top-level docs.

**Step 3: Static verification**

```powershell
rg -n "realworld|SUPPORT_MATRIX|sgpm update|--locked" README.md README.zh-CN.md docs/sgpm-quickstart.md examples/README.md examples/realworld
```

Expected: docs expose the realworld workflow and support matrix.

### Task 6: Final Verification

**Files:**
- Modify: `openspec/changes/mainstream-usable-loop/tasks.md`

**Step 1: OpenSpec validation**

```powershell
cmd /c openspec validate mainstream-usable-loop --strict
cmd /c openspec validate --all --strict
```

Expected: all pass.

**Step 2: Repository verification baseline**

```powershell
cargo fmt --check
cargo test -p sengoo-compiler --lib
cargo test -p sgc
cargo test -p sengoo-runtime --lib
cargo test -p sgpm
cargo test -p sgfmt
cargo test -p sglsp
```

Expected: all pass, or any platform-specific skip is documented in `SUPPORT_MATRIX.md` and relevant test output.

**Step 3: Mark OpenSpec tasks complete**

Update `openspec/changes/mainstream-usable-loop/tasks.md` only for items proven by files and command output.
