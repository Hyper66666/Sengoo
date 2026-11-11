# General conditional partial-field cleanup — review draft

Status: registered implementation child, 2026-11-11, within the continuing
self-hosting program. Independent representation prototypes and verification
come first. No production capability or completed milestone is claimed.

## Problem and evidence

The SG frontend can produce a valid ownership action for a conditionally live
field, but MIR lowering deliberately rejects that action with 5008. The native
consumer has one initialization flag per local, not an independently addressable
flag for each owning field. Removing the guard would therefore be unsound.

The independent cause report is
`target/selfhost/string-lowering-audit01/cause.json`, SHA-256
`9586ad271e7a25c50d24fc1a2817e9f057b41c3e9b5ce480dd627b068fbd36ec`.
Its instrumented copy observed action 13: cleanup kind 3, conditional 1,
path 5 with a non-root parent. This hits `sh_l_actions_phase`'s conditional
field guard, not its missing-local guard. The unchanged development compiler
reports the same issue in `tools/stdlib/string.sg` at bytes 17287..17536.

## Proposed change

Introduce explicit, bounded MIR ownership paths and private per-field
initialization state. Preserve the checked source ownership decisions. A move
clears precisely the moved path's cleanup obligation; initialization restores
precisely the initialized subtree; cleanup releases each still-owned terminal
subobject once in reverse field order. Whole aggregate cleanup and replacement
must use the same state as partial field cleanup.

Prefer a versioned MIR schema extension over reinterpreting opcode 21's local
ID as a field flag. Keep target identity, source aggregate layout, Windows C
ABI and runtime ABI unchanged. See `design.md` for the candidate table and
operation contracts; numeric opcode assignments require an inventory before
implementation.

## Preserved boundaries

- Reserved legacy owner handle projection remains a move. This proposal does
  not turn `String.handle` into a copy or infer observer authority by spelling.
- All bodies required by the current driver are still checked and lowered.
  An uncalled failing body cannot be skipped to pass an import-only program.
- Partial moves through a custom-`Drop` aggregate, tagged payloads, references,
  dynamic indices and unresolved layout/glue retain their existing rejections
  unless a separately reviewed stage explicitly establishes the capability.
- Pending Drop glue kind four remains rejected. Structural cleanup cannot
  bypass a destructor that explicitly destroys a field.
- Final SDK/self-build, async cancellation and exception unwinding cleanup are
  not established by this work.

## Why this does not alone qualify `String.unwrap_or`

The canonical method selects a raw handle through a legacy move, then calls
`sengoo_string_clone_status`. The actual C provider resolves the source and
allocates another string; it does not consume the source handle. Under retained
move semantics, the selected original is excluded from cleanup. Consequently,
fixing 5008 may expose a resource leak rather than finish the standard-library
contract. This is a source-based prediction, not an executed leak result.

The resource oracle must cover both branches with real nonzero allocations,
track original and cloned identities, and fail if the selected original is
unreleased. A source-bound observer contract or a standard-library source
change, if needed, is a separate reviewed decision. This proposal grants
neither exception. Import success and `main` returning 42 are insufficient.

## Staging and ownership

1. Freeze representation, compatibility and the exact supported path domain.
2. Implement and qualify named-struct paths, branch cleanup and structured exits.
3. Qualify replacement/reinitialization and loop backedges without stale flags.
4. Qualify real C ABI, frame/unwind metadata and resource behavior, including the
   independent `unwrap_or` resource decision.
5. Run the current ordinary/source integration gates and record a new bounded
   checkpoint; do not close full M3/M4 or final SDK work.

Implementation owner: `add-selfhost-conditional-field-cleanup`. Its dedicated
`selfhost-conditional-field-cleanup` capability owns path interpretation, typed
MIR representation, native flag execution and their tests. The semantics and
native-backend parent changes track integration dependencies without duplicating
these requirements. Canonical memory-management semantics remain unchanged.

## Reading basis

Design read against HEAD `0e112a64d393f219a8f9669f24f8728de8f3aff8` plus the
ongoing joint factory/print working tree. This is not a claim that the working
tree equals that commit. A later gate must capture its own complete inputs.

Canonical contract: `openspec/specs/memory-management/spec.md`,
`docs/language-reference.md` ownership section, and
`examples/realworld/SUPPORT_MATRIX.md`. Selfhost deltas:
`add-selfhost-semantics`, `add-selfhost-native-backend`, and
`add-selfhost-handle-disarm`. Current representation/consumer:
`selfhost/src/{borrow,hir,mir,mir_lower,mir_validate,native}.sg` and
`selfhost/README.md`. Existing named-field behavior is a subset; the canonical
language specification is not proof that every SG native case already works.

## Executed reference baseline

The canonical Some/None method was executed through the frozen reference and
the captured actual C runtime, ten scopes per branch. It performed 20 independent
clones and 50 successful releases; every scope restored zero live Strings. The
selected original handle was already invalid when the method returned. Report
`target/selfhost/string-unwrap-reference01/run01/evidence.json`, SHA-256
`0312d6f4078fe5adaedc8701a428bc396f19095ec9a91cd0f1fe1e497d5f581c`, is
a reference-only baseline, not evidence that SG lowering or cleanup passes.
