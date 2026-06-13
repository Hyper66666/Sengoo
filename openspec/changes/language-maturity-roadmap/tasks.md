## 0. Program setup

- [ ] 0.1 Create the eleven child changes named in `proposal.md`, each with its
  own capability delta, tasks, and archive gate.
- [ ] 0.2 Freeze the cross-pillar contract in `design.md` (memory model,
  monomorphization + `dyn`, core trait set, transition strategy); any later
  change to a frozen decision must update `design.md` before code edits.
- [ ] 0.3 Link every child proposal back to this umbrella and record owner/status.
- [ ] 0.4 Snapshot the `examples/realworld/SUPPORT_MATRIX.md` rows this program
  intends to move (memory safety, generics, strings, numerics, concurrency).
- [ ] 0.5 Run `openspec validate language-maturity-roadmap --strict`.

## 1. P0 gate — language foundations

- [ ] 1.1 `automatic-memory-management` validated and archived.
- [ ] 1.2 `generics-and-trait-system` validated and archived.
- [ ] 1.3 `first-class-strings-and-formatting` validated and archived.
- [ ] 1.4 P0 conformance: a realworld fixture rewritten with zero manual
  `.free()/.drop()/.close()`, generic `Result<T, E>` used across at least two
  concrete types, and `println` formatting an owned `String` and a struct via
  `Display`.

## 2. P1 gate — usable surface

- [ ] 2.1 `numeric-type-system` validated and archived.
- [ ] 2.2 `generic-collections` validated and archived.
- [ ] 2.3 `concurrency-safety-and-async-io` validated and archived.
- [ ] 2.4 `debugger-and-test-framework` validated and archived.
- [ ] 2.5 P1 conformance: stdlib collections expose generic `Vec<T>`/`HashMap`
  without scalar hand-specialization; a multi-threaded example is data-race
  checked; a debug session steps and inspects a local.

## 3. P2 gate — ecosystem and adoption

- [ ] 3.1 `package-registry-and-distribution` validated and archived.
- [ ] 3.2 `wasm-and-bytecode-backends` validated and archived.
- [ ] 3.3 `authoritative-language-reference` validated and archived.
- [ ] 3.4 `flagship-reference-application` validated and archived.

## 4. Umbrella closure

- [ ] 4.1 Confirm all eleven child changes have passed `--strict` validation and
  been archived in dependency order.
- [ ] 4.2 Update `examples/realworld/SUPPORT_MATRIX.md` and `README.md` so the
  P0–P2 capabilities are listed as Supported (not "subset"/"deferred") with proof.
- [ ] 4.3 Run `openspec validate --all --strict`.

## Verification

- `openspec validate --all --strict`
- P0/P1/P2 conformance tasks above (1.4, 2.5, and each child's own gate)
- Existing gates remain green: `cargo test -p sengoo-compiler --lib`,
  `cargo test -p sgc core_conformance_examples_compile_link_and_run`,
  `cargo test -p sglsp`
