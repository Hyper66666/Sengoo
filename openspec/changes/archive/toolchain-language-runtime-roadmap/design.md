## Context

Sengoo currently has a strong compiler foundation but several adjacent tools and language/runtime capabilities are either partial or uncoordinated. The repository already includes `tools/sglsp`, `tools/sgfmt`, and `tools/sgpy`, plus compiler/runtime/docs surfaces that need a unified product direction.

This change defines a spec-first roadmap across four tracks:
1) high-priority developer tooling,
2) medium-priority language features,
3) compiler/runtime optimization,
4) documentation and ecosystem maturity.

A naming correction is also required: package manager branding and interfaces should move from `sgpy` to `sgpm`.

## Goals / Non-Goals

**Goals:**
- Establish clear, testable requirements for each roadmap capability.
- Sequence delivery so high-priority tooling (LSP/formatter/package manager) lands before advanced language/runtime features.
- Standardize configuration contracts (`Sengoo.toml`, formatter config) and CLI surfaces.
- Define a low-risk migration path from `sgpy` to `sgpm`.

**Non-Goals:**
- Deliver all capabilities in one implementation cycle.
- Fully redesign compiler internals beyond what each capability requires.
- Lock down every low-level implementation detail before prototyping.

## Decisions

1. Use capability-per-spec decomposition to keep scope manageable.
- Each capability gets its own `specs/<capability>/spec.md`.
- This allows independent implementation and review for tooling, language, runtime, and docs tracks.

Alternative considered: one monolithic roadmap spec. Rejected due poor traceability and high coordination overhead.

2. Prioritize tooling before language/runtime expansion.
- Phase 1: `sglsp`, `sgfmt`, `sgpm`.
- Phase 2: generics, async, macros.
- Phase 3: incremental compile, JIT/AOT, Python interop.
- Phase 4: docs and stdlib deepening.

Alternative considered: language features first. Rejected because weak tooling slows feedback loops and adoption.

3. Standardize package ecosystem on `sgpm` + `Sengoo.toml`.
- Introduce `sgpm` as canonical CLI and documentation term.
- Keep a temporary compatibility alias for `sgpy` during migration to avoid abrupt breakage.

Alternative considered: keep `sgpy` indefinitely. Rejected due naming mismatch with Sengoo branding.

4. Integrate diagnostics through existing compiler JSON channel.
- LSP diagnostics should consume `sgc --error-format json` output instead of duplicating analyzer logic.

Alternative considered: independent diagnostics pipeline inside LSP. Rejected due duplication and drift risk.

5. Keep architecture flexible for runtime backends.
- Specify behavior contracts for JIT/AOT and async runtime without over-constraining executor/backend internals.

Alternative considered: mandate one backend implementation now. Rejected because runtime/backend benchmarking is still evolving.

## Risks / Trade-offs

- [Scope breadth is large] Many capabilities can stall if tracked as one batch.
  -> Mitigation: implement in phased milestones and enforce capability-level acceptance criteria.
- [CLI migration friction] Existing scripts may depend on `sgpy` naming.
  -> Mitigation: add alias period, deprecation messaging, and migration docs.
- [Feature interaction complexity] Generics + async + macros can amplify compiler complexity.
  -> Mitigation: gate each feature with focused specs/tests and incremental rollout.
- [Performance regressions] New incremental and runtime paths can regress compile/run stability.
  -> Mitigation: benchmark gates and fallback paths for JIT/AOT and invalidation logic.

## Migration Plan

1. Publish roadmap specs and align naming (`sgpm`) in docs and command help text.
2. Land tooling capabilities first, including backward-compatible `sgpy` alias.
3. Roll out language features behind feature gates where practical.
4. Introduce optimization/runtime features with benchmarks and rollback switches.
5. Expand docs/examples/stdlib after core behavior stabilizes.

Rollback strategy:
- Keep old CLI entry points and runtime mode defaults until new paths are proven.
- Revert individual capability implementations independently if regressions appear.

## Open Questions

- How long should `sgpy` compatibility alias remain before removal?
- Should `Sengoo.toml` support workspace semantics in v1, or single-package only?
- Which async runtime API shape best fits Sengoo ergonomics and performance targets?
- Should procedural macros run in-process or in a sandboxed helper process?
