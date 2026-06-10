## Scope

This child change owns the remaining P4 language-surface polish that is not
already delivered by existing language changes. It is additive by default:
accepted forms become supported with tests, and rejected forms keep stable
diagnostics in `sgc` and `sglsp`.

## Existing Ownership To Avoid

- `language-surface-expansion` owns the phase 4a attribute matrix, class header
  trait lists, and dynamic native i64 FFI arity `0..=8`.
- Archived `try-and-match-ergonomics` owns `?`, `try {}`, match pattern
  semantics, exhaustiveness, unreachable-arm diagnostics, and the simple
  wildcard quick fix.
- Archived `owned-string-text` owns canonical stdlib `String`, move semantics,
  explicit conversions, and the no-`as_str()` v1 boundary.
- `async-reactor-futures` and `concurrent-async-runtime` own runtime semantics,
  reactor/future contracts, thread-pool behavior, and cross-thread `Send`
  checks.
- `runtime-hardening-ffi-async` owns runtime FFI safety/status behavior, not
  source-level FFI signature relaxation.

## Inventory Of Remaining Restrictions

| Area | Observed restriction | Candidate ownership |
| --- | --- | --- |
| Attributes | `cfg` currently accepts only `target_os = "..."`; unsupported attributes are rejected generically; direct parser paths still preserve extern-only attribute handling. | Add compatible `cfg` predicate forms only when parser, diagnostics, and `sglsp` source ranges are pinned. |
| FFI source signatures | Typeck rejects generic extern functions and non-scalar/non-`&str` signatures; aggregate, owned `String`, callback, and unsupported ABI shapes remain blocked. | Add a small accepted source signature set only with negative tests for every still-blocked shape. |
| Async frames | MIR frame helpers still reject payload-carrying enum values crossing `await`. | Accept payload enums across awaits only after frame layout/load/store tests prove no unsound move or drop behavior. |
| Match/try diagnostics | Core semantics are owned, but future relaxations need stable diagnostic codes and LSP quick-fix parity rather than generic phase errors. | Require diagnostic-code inventory before adding new pattern or propagation forms. |
| Parser/typeck/lowering parity | Some restrictions surface from different phases with different messages. | New language polish tasks must prove the accepted/rejected form is consistent across parser, typeck, lowering, JSON diagnostics, and LSP. |

## Approach

1. Add inventory tests before relaxations.
2. Relax one surface group at a time.
3. For every accepted form, add parser, typeck/lowering, `sgc` JSON diagnostic
   where applicable, and `sglsp` coverage.
4. For every still-rejected adjacent form, add negative tests with stable codes
   or documented stable message prefixes.
5. Keep incompatible cleanup out of implementation until migration docs exist
and the parent umbrella accepts the cleanup gate.

## Pinned Decisions For This Change

This change is intentionally narrow even though it touches several language
areas.

### Attributes

The next accepted `cfg` predicate set is:

- `target_os = "windows" | "linux" | "macos"`
- `target_family = "windows" | "unix"`
- `feature = "<manifest-feature-name>"`
- `all(<predicate>, ...)`
- `any(<predicate>, ...)`
- `not(<predicate>)`

`feature` values resolve from the selected package manifest when a manifest is
available and default to false in standalone mode unless a future command-line
feature flag spec accepts explicit feature selection. Unsupported attributes and
malformed predicates remain errors. `deprecated` remains warning-only and keeps
the existing accepted declaration-site matrix.

### FFI Source Signatures

This phase does not widen the accepted FFI type set. Generic extern functions,
aggregate parameters/returns, owned `String` parameters/returns, callbacks,
mutable references, and unsupported ABI names remain rejected. The deliverable
is stable parser/typeck/JSON/LSP diagnostics and negative tests for those
neighbors, not a new ABI.

### Async Frames

This phase accepts payload-carrying enum locals, parameters, and return values
crossing `await` only if async frame layout/store/load/drop tests prove the
payload is preserved across suspend/resume and cleanup. If implementation cannot
prove this without widening ownership semantics, the feature remains Deferred
with stable diagnostics; no half-lowered payload enum may reach LLVM.

### Match/Try

This phase does not add match guards, new pattern syntax, or implicit error
conversion. It owns diagnostic parity only: compiler JSON diagnostics and LSP
quick fixes must match the archived `try-and-match-ergonomics` behavior for
accepted and rejected forms.

## Migration Policy

This change is additive. If implementation discovers a cleanup that would reject
previously accepted source or rename public syntax, the cleanup must move behind
a migration document before implementation continues. The document must name the
old behavior, new behavior, user-visible diagnostic, replacement code shape, and
compatibility window.

## Verification

Minimum future verification:

- `openspec validate language-default-polish --strict`
- Targeted `cargo test -p sengoo-compiler` filters for each accepted/rejected
  language surface.
- Targeted `cargo test -p sglsp` filters proving diagnostic range, severity,
  code, and quick-fix parity where a quick fix is safe.
- `sgc check` or JSON diagnostic snapshots for CLI-facing rejection paths.
