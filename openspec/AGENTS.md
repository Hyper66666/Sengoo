# Sengoo OpenSpec workflow

This repository uses the `spec-driven` schema in `config.yaml`.

1. Read the current language reference, support matrix and affected canonical
   specs before changing a supported behavior. Historical proposals are not a
   substitute for executable evidence.
2. Put a change in `changes/<name>/` with `proposal.md`, `design.md`, `tasks.md`
   and capability deltas under `specs/<capability>/spec.md`. Requirements use
   `### Requirement:` and executable scenarios use `#### Scenario:` with
   WHEN/THEN assertions. Mark additions, modifications and removals explicitly.
3. Give each requirement one implementation owner. Umbrellas track child
   changes and integration gates rather than duplicating their requirements.
4. User approval of an implementation plan authorizes its corresponding
   changes. Preserve the edition 2026 and published v0.2 compatibility policy;
   document experimental boundaries and any soundness exception.
5. Check off a task only after its implementation and named verification pass.
   Record the full revision and host for release evidence. A configured CI job
   is not a successful run; required host skips do not count as passes.
6. Run `openspec validate <change> --strict` and `openspec validate --all
   --strict` where the installed CLI supports them. Reconcile specs before
   archiving. Preserve historical archives; a reconciliation note may explain
   which later evidence supersedes an old residual.

The current native production reference is text LLVM IR compiled with clang.
WASM, bytecode, Cranelift and graphics keep their documented independent scope.
