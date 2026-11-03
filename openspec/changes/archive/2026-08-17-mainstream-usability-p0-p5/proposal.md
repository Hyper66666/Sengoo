## Why

Sengoo's language core is stronger than its day-to-day feel. Direct probes with
the checked-in `sgc` confirm that `match`, `?`, `return`, statement-style `if`,
range `for`, static methods, and `format()` all work today. What a newcomer
actually hits first, however, is a wall of small refusals on the most common
operations in any mainstream language.

The dominant cause is one modelling decision. `Option<T>` and `Result<T, E>` are
declared as **structs**, not enums:

```sg
struct Option<T> { is_some: bool, value: T }
struct Result<T, E> { is_ok: bool, value: T, error: E }
```

An "absent" `Option` must therefore still carry a value, which is why the
standard library needs `option_none_with<T>(placeholder: T)`, and why the
flagship fixture reads:

```sg
root.object_get("max_depth").unwrap_or(JsonValue { doc_handle: 0, node_id: 0 })
```

Hand-building a sentinel aggregate to read an optional field is not something a
user of a mainstream language is ever asked to do. The same root cause makes
`Some`/`None`/`Ok`/`Err` unavailable as constructors, makes `match` on fallible
results impossible, and pushes `if result.is_ok == false` chains through user
code.

This also leaves an implementation gap against Sengoo's own archived
`try-and-match-ergonomics` capability, which already pins `Ok(value)`,
`Err(error)`, `Some(value)`, and `None` semantics that no program can currently
express.

Two further gaps compound the impression. `for x in collection` is rejected for
every generic collection (`for` accepts only arrays, slices, and ranges), so the
single most common loop in mainstream code cannot be written. And `sgc run`
prints compiler internals — cache misses, workset manifests, frontend scheduler
statistics, and `clang -isystem` warnings — on a successful run.

Closing these makes existing strengths reachable rather than adding breadth.

## What Changes

Six ordered priorities. P0 is the root-cause fix; each later item is
independently shippable.

1. **P0 — `Option`/`Result` become real enums.** `Some`/`None`/`Ok`/`Err`
   constructors, `match` on both types, and no sentinel construction. Field
   accessors (`.is_ok`, `.value`, `.error`, `.is_some`) remain available for one
   release as a deprecated compatibility surface so the ~430 existing call sites
   migrate incrementally.
2. **P1 — `for` over collections.** `for x in vec` and `for x in vec.iter()`
   lower through the existing `Iterator` protocol for `Vec`, `HashMap`,
   `HashSet`, `BTreeMap`, `BTreeSet`, and `VecDeque`.
3. **P2 — Quiet, coherent tooling output.** Successful `sgc run`/`build` print
   only program-relevant output; diagnostics and progress detail move behind
   `--verbose`. All compiler diagnostics use one language.
4. **P3 — Everyday syntax.** `vec![]`, multi-argument `println("{}", x)`,
   `{:?}` debug formatting for derived shapes, and `keys()`/`values()` map
   iteration.
5. **P4 — Idiomatic flagship examples.** Rewrite the collapsed single-line,
   deeply nested example sources into flat guard clauses with early `return`
   and `?`. Requires no language or formatter change.
6. **P5 — Width-aware block formatting.** `sgfmt` currently collapses every
   non-function block onto one line: `format_block` (one statement per line) is
   reached only for function bodies, while `if`, `while`, `for`, `loop`, `match`
   arms, `async`, `parallel`, and `try` all go through `format_block_inline`,
   which joins statements with a single space unconditionally.
   `FormatOptions.max_width` is parsed, validated, and stored but never read by
   any formatting code, so `--max-width` has no effect. Make the inline form
   fall back to the multi-line rendering when it would exceed `max_width`.

## Capabilities

### Modified Capabilities

- `language-reference`: `Option`/`Result` enum form with pattern constructors,
  `for`-over-`Iterator` iteration, `if let`, and debug formatting requirements.
- `stdlib-mainstream-usability`: enum-shaped `Option`/`Result` surface, the
  deprecated compatibility accessors, collection iteration entry points, and
  map `keys()`/`values()`.
- `tooling-mainstream-ecosystem`: default output verbosity contract,
  single-language diagnostics, and width-aware block formatting.

## Impact

- Compiler: MIR lowering and Drop glue for enum-shaped `Option`/`Result`,
  `for`-loop desugaring onto `Iterator`, `if let` patterns, `vec![]` and
  format-argument parsing, diagnostic message unification.
- Standard library: `option.sg`, `result.sg`, `collections.sg`, and every
  wrapper returning `Option`/`Result` (236 field-access sites).
- Examples and fixtures: 195 field-access sites plus the flagship rewrites.
- Documentation: language reference status rows and the support matrix.
- `sgc` CLI output layering; `sglsp` diagnostic parity.

Type checking of `?` is expected to need no change: `peel_option_ty_static` and
`peel_result_ty_static` in `compiler/src/typeck/check/try_helpers.rs` already
match on type name and arity rather than on struct fields.

## Non-Goals

- No trait-based `IntoIterator` generalisation beyond the collections listed in
  P1; user types keep the existing `Iterator` protocol.
- No implicit error conversion on `?` (no `From`-based widening in this wave).
- No `Box<dyn Error>`, error trait objects, or backtrace capture.
- No full `Formatter` customisation grammar beyond `{:?}` for derived shapes.
- No macro system; `vec![]` is a pinned built-in form, not user-definable.
- No change to the native ABI, async semantics, or backend target set.
