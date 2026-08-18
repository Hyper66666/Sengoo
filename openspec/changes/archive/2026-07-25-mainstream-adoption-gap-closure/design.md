## Scope

`mainstream-adoption-gap-closure` is the umbrella lane that closes the four
adoption gaps identified after the `six-pillar-gap-closure` program: source
-level debugging, cancellation semantics, production HTTP serving, and
toolchain distribution. It consumes and extends:

- archived `codegen-ir-correctness-and-gate` (correctness prerequisite)
- archived `compile-scale-production-gate` / archived `frontend-1000k-perf-gate` supersession (perf gate re-run only, not owned)
- archived `async-default-followups`, `async-reactor-futures`,
  `concurrent-async-runtime` (cancellation builds on their pinned surfaces)
- archived `async-http-serving`, `stdlib-https-tls` deltas in
  `stdlib-http-server` and `stdlib-mainstream-usability`
- `tooling-mainstream-ecosystem` (internal release process becomes a
  distribution channel)

It must not silently redefine existing `STATUS_*` meanings, the pinned
`Poll<T>`/`Future<T>` contract, the non-canceling `select`/`timeout`
semantics, or the `Connection: close` default without explicit MODIFIED
requirements in a child change.

## Program shape

```text
Phase 0  Inventory + dependency map + matrix baseline (this umbrella)
Phase 1  Prerequisite: codegen-ir-correctness-and-gate implemented + archived
         (satisfied by archive 2026-06-11-codegen-ir-correctness-and-gate)
Phase 2  Pillar A (native-debug-info) + Pillar D (toolchain-distribution)
         in parallel — disjoint code surfaces
Phase 3  Pillar B (async-cancellation-semantics)
Phase 4  Pillar C (http-production-serving) — depends on Pillar B loser-
         cancellation primitives for connection teardown
Phase 5  Integration: matrix refresh, perf gate re-run, docs, archive gate
```

Parallel lanes are allowed inside phases, but Phase 5 is the only
integration gate that can mark the umbrella done.

## Upstream archive prerequisites

- `codegen-ir-correctness-and-gate` is archived and must stay green while
  `native-debug-info` merges IR-emission changes; both edit
  `compiler/src/codegen/` and the conformance gate drives the real CLI so
  debug-build regressions are caught.
- `compile-scale-production-gate` closed the 100k/1000k performance evidence
  and superseded `frontend-1000k-perf-gate`. This program only re-runs the
  perf gate as regression evidence in Phase 5 after `-g` / debug-info work;
  any regression blocks this umbrella before archive.
- Two active changes must not claim the same canonical requirement; child
  proposals list any unarchived upstream as an explicit blocker.

## Frozen decisions

### D1 — Debug info enablement policy (Pillar A)

- `sgc build`/`sgc run` gain `-g` (alias `--debug-info`). Explicit `-g`
  always emits debug metadata at any `-O` level.
- Without `-g`, no debug metadata is emitted in this wave (no implicit
  default change); a later change may revisit O0 defaults once cache and
  perf evidence exists.
- Debug builds use distinct artifact-cache fingerprints so `-g` and non-`-g`
  artifacts never alias.

### D2 — Debug info v1 surface (Pillar A)

| Area | v1 commitment |
| --- | --- |
| Compile units / files | DI compile unit per module with real file paths |
| Functions | DISubprogram with linkage + source name, file, line |
| Statements | `!dbg` locations on calls, branches, returns, and assignments |
| Breakpoints | Source file:line breakpoints bind and hit in lldb (DWARF) and WinDbg/cdb (CodeView via clang) |
| Stepping | `step`/`next` follow Sengoo source lines |
| Variables | Stretch subset: function parameters as DILocalVariable; full local inspection is out of v1 scope and recorded in the matrix |

### D3 — Cancellation contract (Pillar B)

- Cancellation remains cooperative: it is observed at await points, never
  preemptive mid-statement.
- `cancel_task(task_id)` keeps current behavior; the child specifies and
  tests its propagation: a canceled task stops at its next await point and
  reaches `task_status == 3` without running subsequent user code.
- New consuming `select_cancel(f1..fN)` (2..8 homogeneous operands) returns
  the winner and cancels-then-drops losers deterministically; existing
  `select` semantics are unchanged.
- New `ProcessHandle.wait_cancellable` (name finalized in child design)
  resolves promptly on kill/cancel with `STATUS_CANCELED` or the
  child-pinned status; no new blocking states.
- No cancellation tokens/contexts in this wave; task ids and handles are the
  only cancellation capabilities.

### D4 — HTTP production serving order (Pillar C)

- Feature order inside the child: callback handlers → keep-alive →
  streaming bodies → TLS server. Each lands with its own tests before the
  next starts.
- Keep-alive is opt-in per server config with pinned bounds (max requests
  per connection, idle timeout); default stays `Connection: close`.
- TLS server reuses the existing client stacks (Schannel / rustls); no new
  TLS dependencies, no plaintext fallback marked as TLS success, and the
  client rows' negative-evidence rules apply unchanged.
- HTTP/1.1 only; HTTP/2, websockets, and proxying are out of scope.

### D5 — Distribution channel (Pillar D)

- Release artifacts: `sengoo-<version>-windows-x64.zip` and
  `sengoo-<version>-linux-x64.tar.gz`, each containing `sgc`, `sgpm`,
  `sgfmt`, `sglsp`, license, and a pinned-toolchain README; each archive has
  a `.sha256`.
- Built and uploaded by a tag-triggered GitHub Actions workflow that runs
  the `docs/internal-release.md` smoke matrix first; a failed smoke blocks
  publication.
- Install scripts (`install.ps1`, `install.sh`) download a pinned version,
  verify the checksum, and place binaries on PATH; they never auto-update.
- All four tools report the same version string sourced from
  `workspace.package.version` plus the git short hash.
- macOS remains out of scope; the workflow layout must not preclude adding a
  target later.

## Acceptance targets

| Pillar | Target | Measured by |
| --- | --- | --- |
| A | Breakpoint on a Sengoo source line binds and hits; stepping follows lines on Windows and Linux | Validated debugger transcripts + automated DI-presence IR tests |
| A | `-g` does not change program results and does not regress the non-`-g` perf gate | Conformance examples under `-g`; Phase 5 perf gate re-run |
| B | Canceled spawned task reaches status `3` without executing post-await user code | Compiler + native runtime tests |
| B | `select_cancel` winners/losers deterministic across 2..8 operands | Native tests incl. 3+ operands |
| B | Cancellable process wait resolves promptly after kill | Stdlib process tests on Windows + POSIX |
| C | Keep-alive serves N sequential requests on one connection within bounds | Runtime + realworld smoke |
| C | Handler-callback server routes per path; streaming body delivers bounded chunks | Runtime tests + realworld fixture |
| C | TLS server completes a real handshake with the test CA on at least one host per stack | `net::tls` server tests |
| D | Fresh host installs via script and runs `sgc run examples/01_hello.sg` | Release workflow smoke + documented transcript |
| D | All four tools report the same version | CLI version tests |

## Risks / Trade-offs

- Textual-IR debug metadata is verbose; emitting it only under `-g` keeps
  the default pipeline byte-identical and protects compile-perf targets.
- CodeView fidelity through clang on Windows may lag DWARF; v1 accepts
  line-level parity and records gaps in the matrix rather than blocking on
  full parity.
- Loser cancellation in `select_cancel` interacts with reactor-registered
  wakeups; the child must prove no dangling interest registrations
  (regression risk for `async-reactor-futures` invariants).
- Keep-alive widens DoS surface; bounds (request cap, idle timeout, pending
  cap reuse) are mandatory, not optional tuning.
- Distribution makes breaking CLI changes user-visible; version coherence
  and the smoke matrix become release blockers from the first published tag.

## Archive strategy

This umbrella is archiveable only after all four child changes are
implemented, strictly validated, and archived into their owned canonical
capabilities, the dependency prerequisite in Phase 1 is satisfied, the
SUPPORT_MATRIX rows for debugging, cancellation, HTTP serving, and
distribution cite proof, and the Phase 5 verification wave passes in one
recorded pass. Capability requirements belong to the child changes; this
umbrella carries cross-pillar integration requirements only.
