## 1. Preparation

- [x] 1.1 Explicitly supersede `sgpm-alias-multiversion` and copy forward its canonical package-graph deltas before implementing 3.1.
- [x] 1.2 Run `openspec validate ecosystem-toolchain-maturity --strict`.

## 2. Cross-compile and timings

- [x] 2.1 Implement `sgc build --target` for reference triples.
- [x] 2.2 Add `docs/cross-compilation.md`.
- [x] 2.3 Implement `--timings-json` export.

## 3. Registry and LSP

- [x] 3.1 Extend metadata JSON with `yanked` and `features`.
- [x] 3.2 LSP go-to-definition across dependency sources.
- [x] 3.3 Update `docs/sgpm-quickstart.md` publish checklist.

## 4. Verification

- [x] 4.1 `cargo test -p sgc cross_compile -- --nocapture --test-threads=1`
- [x] 4.2 `cargo test -p sgpm metadata`
- [x] 4.3 `cargo test -p sglsp dependency -- --nocapture` and `cargo test -p sglsp stdlib -- --nocapture`

## Archive Gate

- [x] `openspec validate ecosystem-toolchain-maturity --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] `sgpm-package-graph` ownership is canonical or explicitly superseded before archive.
- [x] Registry `yanked`/`features` metadata is implemented and tested in `tools/sgpm`.
- [x] Cross-compile works on one reference triple or documents evidenced skip (host `--target` + `docs/cross-compilation.md`; cross-host requires documented sysroot env).
