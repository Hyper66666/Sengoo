## 1. Toolchain Foundations (High Priority)

- [x] 1.1 Implement `sglsp` baseline server lifecycle on `tower_lsp` and wire initialize/shutdown/capability negotiation.
- [x] 1.2 Add incremental document sync handling (`didOpen`, incremental `didChange`, `didClose`) in `sglsp`.
- [x] 1.3 Implement `sglsp` completion, definition, and hover handlers backed by compiler symbol/type data.
- [x] 1.4 Integrate `sgc --error-format json` output into `sglsp` diagnostics publishing pipeline.
- [x] 1.5 Repair and stabilize `sgfmt` parsing/formatting API compatibility with current compiler AST/frontend surfaces.
- [x] 1.6 Add formatter config file support (rustfmt-like options) and deterministic/idempotent formatting tests.
- [x] 1.7 Expose formatter through unified CLI entry (`sengoo fmt` / `sgc fmt`) with project-root discovery.
- [x] 1.8 Ship `sgpm` MVP command/documentation surface (`new`, `build`, `check`, `run`, `test`, `fmt`, `tree`, `clean`); `sgpy` compatibility alias is deferred.
- [x] 1.9 Implement `Sengoo.toml` manifest parsing, semver validation, and path-only dependency resolution in `sgpm`.
- [x] 1.10 Add dependency shape diagnostics for unsupported registry/git/version dependencies; private registry auth/source configuration is deferred to a follow-up change.

## 2. Language Features (Medium Priority)

- [x] 2.1 Implement generic function declarations/calls and type-argument inference in type checking.
- [x] 2.2 Implement generic struct declarations/instantiation and field typing through instantiated types.
- [x] 2.3 Add async function and await syntax parsing/lowering/type-check support.
- [x] 2.4 Introduce coroutine-compatible runtime scheduling interface for async task execution.
- [x] 2.5 Implement declarative macro definition/invocation parsing and expansion pipeline.
- [x] 2.6 Implement procedural derive macro loading/execution and post-expansion validation.

## 3. Compiler and Runtime Optimization

- [x] 3.1 Implement AST-aware edit classifier (`noop`, `impl_only`, `interface_change`) in incremental build path.
- [x] 3.2 Improve module fingerprint invalidation to minimize unaffected recompilation.
- [x] 3.3 Add Cranelift-backed fast JIT execution mode for development iteration.
- [x] 3.4 Add AOT compilation mode and artifact packaging path for production deployment.
- [x] 3.5 Expand `runtime/src/python.rs` embedding API coverage and error propagation guarantees.
- [x] 3.6 Add build/export pipeline for Sengoo modules as Python extension modules.

## 4. Docs and Ecosystem

- [x] 4.1 Update `docs/sengoo-tutorial.html` to cover current syntax/toolchain workflows end to end.
- [x] 4.2 Implement API documentation generation command and output layout (rustdoc-like browsing).
- [x] 4.3 Add and validate runnable examples for core language/tooling features in CI.
- [x] 4.4 Expand `tools/stdlib` with `Vec<T>` and `HashMap<K,V>` core operations.
- [x] 4.5 Implement `Iterator` trait with commonly used adapters and coverage tests.
- [x] 4.6 Complete `Option<T>` and `Result<T,E>` ergonomic APIs and conformance tests.

## 5. Verification and Rollout

- [x] 5.1 Add per-capability acceptance test suites mapped to each OpenSpec scenario.
- [x] 5.2 Add migration docs for `sgpy -> sgpm`, including compatibility window and deprecation timeline.
- [x] 5.3 Run full regression (`cargo test`) and toolchain integration checks before enabling defaults.
