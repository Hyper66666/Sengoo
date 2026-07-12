## 1. Entry gate and child ownership

- [x] 1.1 Create independently archivable `wasm-backend-v1` and
  `bytecode-vm-v1` changes with separate designs, specs, tasks, and owners.
- [ ] 1.2 Confirm the native MIR/runtime ABI is versioned and the roadmap's
  default-library, distribution, concurrency, and production-hardening gates
  are archived.
- [ ] 1.3 Run a documented go/no-go review for each backend's user value,
  maintenance budget, owner, support tier, and alternatives.

## 2. Shared conformance and capability matrix

- [ ] 2.1 Freeze the native conformance corpus and differential result format
  consumed by both children.
- [ ] 2.2 Update the per-target capability matrix with explicit production,
  experimental, unsupported, and host-limited states.
- [ ] 2.3 Require stable target diagnostics for unsupported stdlib or ABI
  capabilities; no child may silently run native code as fallback.

## 3. Child closure

- [ ] 3.1 Complete and archive `wasm-backend-v1`.
- [ ] 3.2 Complete and archive `bytecode-vm-v1`, or archive an approved
  replacement decision cancelling it after the go/no-go review.
- [ ] 3.3 Run `openspec validate wasm-and-bytecode-backends --strict` and
  `openspec validate --all --strict`.
