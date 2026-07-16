# Sengoo

[English](README.md) | [Chinese](README.zh-CN.md)

Sengoo is a self-developed compiled language focused on practical engineering outcomes:

- Python interoperability for gradual migration from existing ecosystems
- Fast full and incremental compile loops for day-to-day development
- Native execution from textual LLVM IR compiled and linked by `clang`, plus an opt-in primitive Cranelift fast-JIT
- Optional non-invasive reflection with sidecar metadata

Sengoo is still in active development, but the CLI workflow is already usable for real local projects.

## Installable Toolchain Archives

Sengoo can now be packaged as a self-contained toolchain archive containing
`sgc`, `sgpm`, `sgfmt`, `sglsp`, the standard library, and the C runtime bridge.
The archive layout lets installed `sgc` resolve `std::*` imports without a
source checkout.

Local dry-run packaging:

```powershell
cargo build -p sgc -p sgpm -p sgfmt -p sglsp --release
.\scripts\package-toolchain.ps1 -Version 0.1.0-smoke -NoBuild
$archive = Get-Content target/dist/latest-archive.txt
.\scripts\install.ps1 -Archive $archive -InstallDir target/install-smoke
```

Pinned release install or upgrade (current published prerelease):

```powershell
.\scripts\install.ps1 -Version 0.1.0-rc.1
```

```sh
sh scripts/install.sh --version 0.1.0-rc.1
```

The release workflow builds native Windows x64, Linux x64, macOS arm64, and
macOS x64 archives, verifies SHA-256 during installation, and publishes signed
GitHub build provenance after platform smoke. The published prerelease tag is
[`v0.1.0-rc.1`](https://github.com/Hyper66666/Sengoo/releases/tag/v0.1.0-rc.1).
A later v0.2.0 channel is the target of the `sengoo-v0-2-mainstream-core`
program and is not claimed installed here.

Native debug-info builds use `-g` / `--debug-info`:

```powershell
sgc build path/to/main.sg -O 0 --debug-info
```

## AI Handoff Pack (for other LLMs)

To let another model take over quickly, use:

- `docs/AI_FEWSHOT_PLAYBOOK.md` for few-shot examples with code and rationale
- `sgc --error-format json ...` for machine-readable diagnostics
- function contracts (`requires` / `ensures`) to encode intent before implementation
- `--contract-checks auto|on|off` to decide whether runtime contract guards are inserted

Contract example:

```sg

def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

## Practical Demos (Developer-Oriented)

If you want business-style proof points instead of only synthetic microbenchmarks, run:

```bash
# Sengoo vs Python hot-path runtime demo
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo auto reflection vs C++ manual registry demo
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

Latest demo snapshots (measured on **February 16, 2026**):

- Hot-path demo report:
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- Reflection ergonomics demo report:
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| Hot-path runtime avg (ms) | 25.23 | Python: 1285.13 |
| Hot-path speed ratio | 50.93x faster than Python | baseline |
| Reflection rule file LOC | 28 | C++: 55 |
| Manual registry entries | 0 | C++: 2 |
| Missing dynamic rules | 0 | C++: 1 |

## Why Sengoo

### 1) Hybrid Python Migration, Not Rewrite-Only Migration

Sengoo runtime exposes a Python interop layer (see `runtime/src/python.rs`) so teams can keep Python orchestration while moving hot paths to compiled native modules.

Interop benchmark snapshot (measured on **February 16, 2026**):
`bench/results/1771234431756-python-interop.json`

| Runner | Loop avg (ms) | Calls/s | vs Python native |
|---|---:|---:|---:|
| Python native | 0.965 | 5.18M | baseline |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

### 2) Fast Feedback Through Incremental Pipeline Reuse

Compiler pipeline focus:

- Build and run cache plus module fingerprint invalidation
- AST-aware edit classification (`noop` / `impl_only` / `interface_change`)
- Workset-aware backend orchestration
- Optional daemon mode for persistent process workflows

Cross-language scenario matrix snapshot (measured on **February 16, 2026**):
`bench/results/1771185238357-scenario-matrix.json`

| Metric (avg) | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| Full compile (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| Incremental after edit (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| Incremental reduction (%) | 95.99% | -2.28% | -4.95% | 2.61% |

Advanced pipeline snapshot (real edits plus 100k/1000k scale, averaged on **February 18, 2026** from two runs):
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

Real incremental scenarios (`after_avg_ms`, Sengoo):

| Scenario | After avg (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC full pipeline (Sengoo):

| Stage | Avg (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 153.87 |
| Codegen object (`codegen_obj_avg_ms`) | 90.61 |
| Link (`link_avg_ms`) | 173.05 |
| End-to-end (`e2e_avg_ms`) | 417.53 |

10k-1000k four-language e2e compile comparison (`Sengoo / C++ / Rust / Python`):

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

Low-memory mode e2e snapshot (same 1000k workload, measured on **February 18, 2026**):

| Mode | 1000k e2e avg (ms) | Peak RSS (MB) |
|---|---:|---:|
| Default (`sgc build`) | 2331.39 | 1418.61 |
| Low-memory (`sgc build --low-memory`) | 1737.71 | 672.10 |

Low-memory mode benefits:
- Reduces peak memory by about `52.62%` in this 1000k case.
- Improved e2e compile time by about `25.46%` in this same case due to aggressive unreachable-function pruning.

Low-memory mode trade-offs:
- Disables or bypasses part of incremental cache and session reuse.
- Uses a single-thread frontend and a lower MIR optimization cap.
- On smaller projects or warm incremental loops, it can be slower than the default mode.

How to enable:

```bash
sgc build your_file.sg --low-memory
sgc run your_file.sg --low-memory
```

10k-1000k compile peak-memory comparison (RSS MB, compile-stage only, lower is better):

| LOC | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 10k | 18.88 | 75.68 | 70.84 | 41.40 |
| 100k | 140.18 | 118.50 | 337.86 | 288.46 |
| 1000k | 1367.99 | 435.22 | 2681.55 | 2610.90 |

Sengoo vs C++ RSS ratio: 10k `x0.25`, 100k `x1.18`, 1000k `x3.14`.

Sengoo 1000k stage split:
- Frontend: `1589.02ms` (`86.93%`)
- Codegen object: `76.77ms` (`4.20%`)
- Link: `162.04ms` (`8.86%`)

### 3) Runtime-Class Performance Track

Scenario runtime p50 average (same matrix file `1771185238357`):

| Language | Runtime p50 avg (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.59 |
| Python | 45.14 |

Interpretation:

- Sengoo runtime behavior is currently in the same class as C++ and Rust in this loop-heavy matrix profile.
- In these samples, Sengoo is significantly faster than Python runtime execution.

### 4) Non-Invasive Reflection (Auto by Default)

Reflection in Sengoo is designed for low baseline overhead with an auto mode:

- Default mode is `--reflect=auto`
- Auto mode enables reflection only when reflect imports are detected (`import reflect;` / `import std::reflect;`)
- Force enable with `--reflect` or `--reflect=on`
- Force disable with `--reflect=off`
- Metadata emitted to sidecar JSON (`*.sgreflect.json`)
- Typed runtime invocation (`call_i64` / `call_f64` / `call_bool`) with signature checks
- Native reflection binding path is used when available (fallback handler path is retained)

Reflection build example:

```bash
sgc build examples/09_method_call.sg -O 2
```

Fine-grained reflection selection:

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

Runtime reflection usage example (Rust):

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

Reflection overhead benchmark:

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

Reflection benchmark cases:

- `disabled`: compile with reflection fully off (baseline path)
- `enabled-unused`: compile with `--reflect=on`, runtime reflection API not called
- `enabled-used`: compile with `--reflect=on`, perform runtime symbol listing and typed reflection invoke

Current gate defaults:

- `soft`: enabled-unused overhead <= `25%`, enabled-used overhead <= `45%`
- `hard`: enabled-unused overhead <= `15%`, enabled-used overhead <= `30%`
- Disabled regression check compares against `bench/baseline.json` key `reflection/<suite>/disabled` when available

### 5) Runtime Integration Stack

The latest runtime track adds a reusable FFI wrapper layer and integration utilities:

- Sengoo source wrappers in `tools/stdlib/`
  - `db.sg`, `ffi.sg`, `lua54.sg`, `proto.sg`, and `net.sg`
  - Common C-string inputs now have `&str` helpers; explicit buffer/output pointers remain represented as `i64`
  - Example programs live under `examples/reflection/`
- Database runtime MVP (`runtime/src/reflect/runtime_db.rs`)
  - Lifecycle: `open` / `close` / `ping`
  - Query path: `exec` / `query` with result handles
  - Structured status plus error-message channel instead of plain `-1/0`
- Full C and C++ wrapper path (`runtime/src/reflect/runtime_ffi.rs`)
  - C library open / call / close
  - C++ object lifecycle wrappers: create / call / destroy
  - Fixed-arity `i64` value calls without caller-managed pointer slots
  - Callback bridge: bind / dispatch / unbind
  - Binary payload bridge: managed buffer handles (`new/from/len/ptr/copy_in/copy_out/free`)
- Lua bridge
  - Lightweight runtime subset (`sengoo_lua_*`)
  - Native Lua 5.4 dynamic-library bridge proof of concept (`sengoo_lua54_*`)
  - Fixed-arity Lua54 `i64` value calls without caller-managed pointer slots
- Integration validation lanes
  - Protobuf wire encode/decode FFI path (`runtime_proto`)
  - Network bench runtime path with p50/p95/p99 metrics (`runtime_net_bench`)

Related docs:
- `docs/runtime-ffi-lua.md`
- `docs/database-runtime.md`
- `docs/runtime-protobuf-ffi.md`
- `docs/runtime-network-bench.md`

## Build and Run

```bash
cargo build --release
```

```bash
target/release/sgc run examples/01_hello.sg
```

```bash
target/release/sgc build examples/05_loop.sg -O 2
```

Useful commands:

```bash
# type check
sgc check <file.sg>

# type check (JSON diagnostics for automation and agents)
sgc --error-format json check <file.sg>

# compile and run
sgc run <file.sg> -O 1

# compile and run with runtime contract guards (auto: on for O0/O1, off for O2/O3)
sgc run <file.sg> -O 1 --contract-checks auto

# build native binary
sgc build <file.sg> -O 2

# generate rustdoc-like API docs
sgc doc <file.sg> --output target/doc

# force full rebuild
sgc build <file.sg> -O 2 --force-rebuild

# optional daemon mode
sgc daemon --addr 127.0.0.1:48765
```

## Examples

Curated runnable examples live in [`examples/`](examples/README.md). They cover basic syntax plus async, generics, traits, FFI, and reflection wrappers.

Useful entry points:

- [`examples/realworld/`](examples/realworld/README.md) for package-shaped
  fixtures that run the locked `sgpm update`, `check`, `test`, `fmt --check`,
  `doc`, and `build` loop. Use
  [`examples/realworld/SUPPORT_MATRIX.md`](examples/realworld/SUPPORT_MATRIX.md)
  as the support and gaps reference.
- [`examples/async/`](examples/async/) for `sleep`, `spawn`, `select`, and task lifecycle APIs
- [`examples/generics/`](examples/generics/) for `Vec<i64>`, `Option<T>`, `Result<T, E>`, and `std::collections` import patterns; stdlib smoke tests also cover runtime-backed `Vec<bool>` and `HashMap` mutators
- [`examples/traits/`](examples/traits/) for trait dispatch and generic trait method instantiation
- [`examples/ffi/`](examples/ffi/) for Sengoo <-> C calls

Smoke coverage: `cargo test -p sgc examples_smoke_`.

## Package Manager (sgpm)

`sgpm` is the package-manager MVP for project-level Sengoo workflows. It
supports `Sengoo.toml`, local path dependencies, git dependencies resolved
through a root-package cache, local file registry dependencies with semver
constraints, `new`/`init` project scaffolding, topological `build`/`check`, `run`, profile-aware `test`, `fmt`,
`doc`, `tree`, `update` lockfile generation, `update --check` lockfile freshness
checks, `update --refresh` git cache refreshes, `cache list`,
`cache clean --git`, `--locked` command execution, `clean`, publish dry-runs,
local registry publishing, remote package upload to configured registry URLs,
workspace member selection with `--package`, and all-member workspace execution
with `--workspace` for supported package graph commands.

```bash
sgpm new hello
cd hello
sgpm check
sgpm build
sgpm run
```

For a project-shaped loop with tests, docs, formatting, lockfiles, and
mainstream stdlib usage, run one of the committed realworld fixtures:

```bash
cd examples/realworld/cli-json-audit
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

The same locked sequence is supported from
`examples/realworld/http-client-status` and
`examples/realworld/workspace-doc-loop`; `examples/realworld/workspace-audit`
is the flagship maturity fixture for the same loop; see
[`examples/realworld/README.md`](examples/realworld/README.md).

Use `sgpm init` inside an existing directory to preserve unrelated files while
creating the missing Sengoo package scaffold.
Use `sgpm new math_utils --lib` or `sgpm init --lib` for a reusable library
package with `src/lib.sg`.

Local `sgpm update` writes `Sengoo.lock`, including resolved git commits and
selected local or remote registry versions. `sgpm update --workspace` writes
one root workspace lockfile for all members. Local publish dry-runs can create
package archives and checksums, and
`sgpm publish --registry <name>` can publish a package into a configured local
file registry. `sgpm publish` uploads to `[registries.default].url` and
`sgpm publish --registry <name>` uploads when the named registry has `url`.
`sgpm doc` generates package API docs through `sgc doc`, preferring `[lib]`
entries when a package exposes one.
Workspace roots can declare `[workspace].members`, inherit workspace-level
registries, and run package graph commands with
`--package <name>` or supported all-member package graph commands with
`--workspace`. `sgpm update --refresh` reclones cached git dependencies before
writing the lockfile. `sgpm cache list`, `sgpm cache clean --git`, and
`sgpm cache clean --registry` expose the root package dependency caches for
inspection and pruning. `sgpm update --check` verifies the lockfile without
rewriting it for CI-style workflows, and `--locked` makes package graph commands
fail before invoking external tools when the lockfile is stale. See
`docs/sgpm-quickstart.md` for the current manifest and command surface.
Path, git, and registry packages that declare `[lib]` can now be imported by
dependency name from application source; pure library dependencies are
type-checked before dependent binaries are built. Packages that declare both
`[bin]` and `[lib]` can also import their own library by package name from
their binary entry. Dependency keys currently must match the target package's
`[package].name`; renamed aliases are not supported yet.

## Standard Library

The source-side standard library lives under `tools/stdlib/` and is split by
surface area: `option.sg`, `result.sg`, `collections.sg`, `string.sg`,
`strconv.sg`, `json.sg`, `status.sg`, `io.sg`, `file.sg`, `dir.sg`,
`path.sg`, `process.sg`, `args.sg`, `env.sg`, `math.sg`, `error.sg`, and
reflection wrappers such as `ffi.sg`, `db.sg`, `lua54.sg`, `net.sg`, and
`proto.sg`. Runtime bridges are split across `runtime.c`, `runtime_collections.c`,
`runtime_json.c`, `runtime_process.c`, and `runtime_string.c`. See
`tools/stdlib/README.md` for module summaries and current deferrals.

Use source modules directly from Sengoo code:

```sg
import std::collections;

def main() -> i64 {
    let values = vec_new_i64();
    values.push(41);
    values.get(0).unwrap_or(0) + 1
}
```

`sgc check`, `sgc build`, and `sgc run` preload the requested stdlib source
module plus its current transitive source dependencies before compiling.
For example, reflection wrappers such as `std::db`, `std::lua54`, `std::net`,
and `std::proto` also preload the managed FFI `Buffer` helper used by their
diagnostic and output-copy wrappers.
Curated stdlib examples live under `examples/stdlib/`, including a runnable
`std::string` import smoke path.

## Async Execution

`sgc run` now supports a native async path when the entrypoint is `async def main()`.

Currently supported:

- `async def`
- awaiting futures produced by async functions, async blocks, and supported async builtins
- native execution through the runtime bridge used by `sgc run`
- frame-backed async lowering for sequential control flow, `if`, `loop`, and `match`-shaped code paths covered by the current tests
- `sleep(ms)` as an awaitable timer future
- `timeout(future, ms)` as an awaitable `Future<bool>`
- `spawn(future)`
- `spawn_task(future) -> i64`
- `cancel_task(task_id) -> bool`
- `task_status(task_id) -> i64` (`0=unknown`, `1=pending`, `2=completed`, `3=canceled`)
- `join(f1, f2)`
- `select(f1, f2)` for two futures with the same result type, including scalar, tuple, and struct results covered by the current tests

Current limitations:

- `select` is currently limited to two operands
- loser futures in `select` are not canceled yet
- `spawn(future)` still returns an awaitable `Future<T>`; task lifecycle management is exposed separately through `spawn_task/cancel_task/task_status`
- timer support currently covers `sleep` and `timeout`, but not a general timer queue / wheel
- no IO wakeups yet
- no user-defined awaitables or full trait-based Future abstraction

Minimal example:

```sg
async def add1(x: i64) -> i64 {
    x + 1
}

async def main() -> i64 {
    let task = spawn(add1(41));
    let value = select(task, add1(1));
    print(value);
    value
}
```

Task lifecycle example:

```sg
async def child() -> i64 {
    await sleep(5);
    7
}

async def main() -> i64 {
    let task = spawn_task(child());
    let before = task_status(task);
    await sleep(10);
    let after = task_status(task);
    if before == 1 && after == 2 { 42 } else { 0 }
}
```

Timer example:

```sg
async def work() -> i64 {
    42
}

async def main() -> i64 {
    let task = work();
    let ready = await timeout(task, 10);
    if ready {
        await task
    } else {
        0
    }
}
```

Run it the same way as sync programs:

```bash
sgc run <file.sg> -O 1
```

## VS Code Extension

- Extension package location: `vscode-sengoo/`
- Current package version: `1.0.0`

## Benchmark Reproducibility

Benchmark suites are maintained in a separate repository:

- `https://github.com/Hyper66666/bench`

Common commands:

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

Fairness profile used in advanced pipeline comparison:

- C++: precompiled header enabled
- Rust: cargo incremental enabled (`CARGO_INCREMENTAL=1`)

## Documentation

- API docs generator: `sgc doc <file.sg> --output target/doc`
- Tutorial: `docs/sengoo-tutorial.html`
- Language features: `docs/language-features.md`
- Development guide: `docs/DEVELOPMENT_GUIDE.md`
- Compatibility and supported hosts: `docs/compatibility-policy.md`
- Internal release process: `docs/internal-release.md`

## Repository Layout

```text
Sengoo/
|-- compiler/        # Frontend, type checker, HIR/MIR pipeline
|-- runtime/         # Runtime support, Python interop, reflection runtime API
|-- tools/
|   |-- sgc/         # Compiler CLI
|   |-- sgfmt/       # Formatter
|   `-- sglsp/       # Language server
|-- examples/        # Language examples
|-- docs/            # Tutorial and developer docs
`-- vscode-sengoo/   # VS Code extension
```

## Language Ergonomics (Try / Match)

The compiler now ships a pinned `try-and-match-ergonomics` surface (see
`openspec/specs/try-and-match-ergonomics/spec.md`):

- Postfix `?` for `Result<T, E>` and `Option<T>` with stable mismatch diagnostics
- `try { ... }` blocks that contain propagation and return the inferred container
- `match` with enum variant patterns, guards, `| or` patterns, and exhaustiveness checks
- MIR lowering for guarded arms and enum discriminant dispatch; async `match` + `await` arms are covered by `cargo test -p sgc match`

Runnable samples live under `examples/ergonomics/`.

## Project Status

Current stage: early but fast-iterating — usable for local projects, with weekly
compiler/stdlib/tooling landings.

Current focus:

- Runtime hardening for FFI/async edges and stdlib breadth
- `sgc test` manifest workflows and package-level smoke gates
- Frontend compile profiling and MIR lowering hot-path work
- Tooling polish (`sglsp` diagnostics, JSON error output, stdlib indexing)

Development progress (May–June 2026):

- **Control flow**: `?`, `try {}`, enum `match`, guards, and or-patterns are implemented end-to-end (parser → typeck → MIR → native codegen). Regression tests live under `compiler/src/tests/*try*` and `*match*`.
- **Async**: resume lowering no longer reuses the future local for frame loads (fixes LLVM SSA duplicates in `match` + `await` paths). `cargo test -p sgc match` covers native async match smoke.
- **Standard library**: mainstream scripting surface expanded — owned `String`, `std::json`, `std::status` taxonomy, synchronous `std::io`, directory listing and copy/move (`std::dir`, `std::file`), shell-free `std::process` helpers plus capture/timeouts, and decimal `std::strconv`. Examples `examples/stdlib/15`–`20` exercise the new modules.
- **Compiler pipeline**: native artifact caches now fingerprint `runtime.c` bytes (not just path) so in-place runtime edits cannot reuse stale executables. Frontend phase timings and MIR lowering hot-path fixes (including `Cow`-based block-state updates) landed under `openspec/specs/frontend-build-performance/`.
- **Tooling**: `sgc` gains a `test` command path for manifest-driven smoke; `sglsp` stdlib go-to-definition and diagnostics align with imported `std::*` modules. Large `sgc` sources were split for maintainability (interface commands, fingerprints, incremental bench hooks).
- **Earlier May refactors** (still relevant): `NetRuntime` owns all socket/server state; Lua54 reflection tests are mutex-serialized in test builds; `env.declare_fn` removed duplicate function-type clones; HIR class inheritance resolution borrows AST names instead of cloning strings.

Verification snapshot on the integration branch (local, June 2026):

- `cargo test -p sengoo-compiler --lib` — compiler unit suite
- `cargo test -p sgc match` — async/match native smoke
- `cargo test -p sglsp` — LSP surface
- `openspec validate --all --strict` — canonical specs

Notes:

- All benchmark numbers above are local-machine measurements and should be treated as trend indicators.
- Use the benchmark repository and CI gates to verify performance on your own hardware.
