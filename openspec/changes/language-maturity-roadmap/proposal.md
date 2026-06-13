## Why

Sengoo has a working compiler/toolchain baseline: AOT through textual LLVM IR +
clang (with a Cranelift fast path), a static type checker plus borrow checker,
structs/enums/generic structs/traits/closures, `match` + `?`/`try`, a
cooperative async runtime, C FFI, Python interop, reflection, and a broad
tooling surface (`sgc`, `sgpm`, `sgfmt`, `sglsp`, VS Code extension). The
`mainstream-usable-loop` and `six-pillar-gap-closure` programs proved the
package workflow is real.

The remaining gap is no longer "does the workflow exist" — it is "is the
*language itself* mainstream-usable end to end". Concretely, three structural
language gaps still force unsafe and verbose code, and a set of ecosystem gaps
still block confident adoption:

- **Memory model is not self-consistent.** The spec promises RC + cycle GC, but
  the implementation has a borrow checker for references while runtime heap
  resources (`Vec`, `String`, `Buffer`, `JsonDoc`, handles) require manual
  `.free()` / `.drop()` / `.close()` (see `examples/stdlib/20_owned_string.sg`
  and `examples/realworld/cli-json-audit/src/main.sg`). The default is not
  memory-safe.
- **Generics + traits are too weak.** The stdlib is forced to hand-specialize
  per scalar (`Vec<i64>`, `Vec<bool>`, `StringMapI64`...) and user generic types
  only work through concrete impls (`impl Result<i64, i64>`). There are no
  general trait bounds, no trait objects, no core traits (Clone/Eq/Ord/Hash/
  Display/Iterator).
- **Strings are not first-class.** `print` only prints integers, there is no
  formatting/interpolation, no `+` concat ergonomics, and Unicode is byte-order
  only. Spec literal forms (f-string, byte string, multiline, numeric suffixes,
  `0o`/`0b`) are unimplemented.
- **Ecosystem/distribution gaps**: i64-centric numeric story, no general
  containers, single-thread cooperative concurrency with an unproven
  cross-platform IO reactor, no real debugger stepping, no public package
  registry or prebuilt binary distribution, only LLVM/Cranelift backends (no
  WASM/bytecode), and a language spec that is a stale design draft.

## Proposal

Deliver a phased, three-tier (P0/P1/P2) language-maturity program. This is the
umbrella OpenSpec lane; like `six-pillar-gap-closure`, it owns the cross-pillar
contract and sequencing while each pillar lands as an independently reviewable,
revertible, and archivable **child change** with its own capability delta,
tasks, and verification.

The umbrella is not done until every required child change is validated and
archived. An accepted-risk matrix row cannot replace an unimplemented pillar.

| Tier | Child change | Capability | Primary scope |
| --- | --- | --- | --- |
| P0 | `automatic-memory-management` | `memory-management` | One coherent model: ownership + automatic `Drop` insertion (RAII), eliminating manual `.free()/.drop()/.close()` |
| P0 | `generics-and-trait-system` | `generics-and-traits` | Real monomorphization, trait bounds `T: Trait`, trait objects `dyn`, associated types, core traits |
| P0 | `first-class-strings-and-formatting` | `strings-and-formatting` | First-class `String`/`&str`, formatting + interpolation, `print`/`println` of any `Display`, UTF-8 correctness |
| P1 | `numeric-type-system` | `numeric-types` | Integer widths + signedness, defined overflow semantics, float math/parse/format |
| P1 | `generic-collections` | `generic-collections` | Generic `Vec<T>`/`HashMap<K,V>`/`HashSet`/`BTreeMap` + iterator adapters on top of P0 generics |
| P1 | `concurrency-safety-and-async-io` | `concurrency-and-async-io` | `Send`/`Sync` data-race model, multi-threaded executor, cross-platform IO reactor |
| P1 | `debugger-and-test-framework` | `debug-and-test-tooling` | Real source-level debugging (step/inspect) + richer test framework (fixtures, parametrization, coverage) |
| P2 | `package-registry-and-distribution` | `package-registry-distribution` | Public registry protocol, prebuilt binary distribution, macOS channel |
| P2 | `wasm-and-bytecode-backends` | `backend-targets` | WASM backend and a portable bytecode VM as promised by the spec |
| P2 | `authoritative-language-reference` | `language-reference` | Accurate, versioned, implementation-synced language reference replacing the stale draft |
| P2 | `flagship-reference-application` | `flagship-application` | A non-trivial real application written in Sengoo as proof of usability |

### Sequencing

P0 is foundational: `generic-collections`, `numeric-type-system`, and the
stdlib rewrite all depend on `generics-and-trait-system` and
`automatic-memory-management`; `strings-and-formatting` depends on the memory
model for owned `String`. P1 builds the usable surface on top of P0. P2 is
adoption/ecosystem and can proceed in parallel once P0 stabilizes the language.

See `design.md` for the cross-pillar dependency graph and the decision record
(notably choosing ownership + `Drop` over RC + cycle GC).

## Non-goals

- Self-hosting the compiler in Sengoo.
- Removing or breaking existing Buffer/handle stdlib APIs during the transition;
  new safe APIs are added alongside and old names stay source-compatible until a
  later, separately-proposed deprecation.
- Launching a hosted commercial package registry service (the P2 child specifies
  the protocol and a reference server, not an operated service).
