## 1. Shared entry contract

- [x] 1.1 Archive the native default-library, distribution, concurrency, and
  production-hardening prerequisites and retain their evidence links.
- [x] 1.2 Version the in-process MIR semantic contract and expose target-aware
  compiler plus `sgc` MIR bundle APIs.
  - `MIR_SEMANTIC_ABI_VERSION` / `MirBundle` live in `compiler/src/lib.rs`;
    `sgc` exposes `pipeline::compile_source_to_mir_bundle(source, opt, triple)`.
- [x] 1.3 Add and validate `runtime/abi/portable_runtime_abi_v1.json`, including
  required semantic IDs, stable ordinals, and forbidden native vocabulary.
  - Contract suite: `cargo test -p sgc --test portable_abi_contract`.
- [x] 1.4 Route WASM frontend lowering through explicit wasm32 semantics and
  reject unknown MIR/runtime ABI versions before backend lowering.
  - `build_wasm` lowers via `wasm32-unknown-unknown`; unknown MIR/runtime ABI
    versions fail before emission (`validate_portable_abi_versions`).
- [x] 1.5 Emit stable `unsupported-target-capability` diagnostics with target
  and capability fields and no native fallback.
- [x] 1.6 Add entry-contract tests for 32-bit pointer-sized lowering, out-of-
  range `usize`, ABI mismatch, portable ABI linting, and target diagnostics.
  - `cargo test -p sengoo-compiler --test mir_target_contract`
  - `cargo test -p sgc --test portable_abi_contract --test portable_targets`

## 2. Independent backend owners

- [x] 2.1 Create independently archivable `wasm-backend-v1` and
  `bytecode-vm-v1` owner changes.
- [x] 2.2 Classify the current scalar WASM emitter and `SGB1` interpreter as
  experimental prototypes with no artifact compatibility promise.
- [x] 2.3 Record the entry-review result and activate each child only after it
  consumes the shared MIR/runtime contract.
  - WASM activated and archived as scalar v1; bytecode NO-GO archived with
    `docs/bytecode-vm-value-review.md`.

## 3. Cross-target closure

- [x] 3.1 Keep native semantics as the differential oracle and one capability
  matrix as the support truth source.
  - `docs/portable-targets.md` is the portable matrix; native remains oracle.
- [x] 3.2 Record the bytecode value-review decision without treating prototype
  implementation as evidence that the VM must ship.
  - NO-GO recorded; production VM cancelled.
- [x] 3.3 Run `openspec validate wasm-and-bytecode-backends --strict` and
  `openspec validate --all --strict`.
