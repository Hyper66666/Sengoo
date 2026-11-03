## Context

Probes against the checked-in `sgc` establish the current baseline. Working
today: `match` with payload variants, `?` propagation, `return`, statement-style
`if`, range `for`, static methods (`Type::new`), `+` string concatenation, and
`format()`. Rejected today:

| Probe | Diagnostic |
| --- | --- |
| `for x in vec` | `for loop expects an array, slice, or range iterable` |
| `Some(n)` / `None` | `type Option is not generic` |
| `match r { Ok(v) => .., Err(e) => .. }` | `type Result is not generic` |
| `if let Some(v) = x` | `parse error: invalid pattern` |
| `vec![1, 2, 3]` | `未定义的变量: vec` |
| `map.keys()` | `类型 HashMap<String,i64> 没有方法 keys` |
| `println("{}", x)` | `参数数量错误: 期望 1 个, 找到 2 个` |
| `f"{p:?}"` | `parse error: unexpected token in f-string interpolation` |

Two of those diagnostics are Chinese and the rest English, in one toolchain.

`sgfmt` **does** collapse multi-line blocks, so the single-line nested style in
the flagship fixtures is canonical formatter output rather than hand-written
legacy. `format_block` (one statement per line) is reached only from
`lib.rs:120` for function bodies; every other block — `if`, `while`, `for`,
`loop`, `match` arms, `async`, `parallel`, `try` — goes through
`format_block_inline` (`expressions.rs:17`), which joins statements with a
single space unconditionally. `FormatOptions.max_width` is parsed from config
and CLI and validated, but no formatting code ever reads it, so `--max-width`
has no effect.

Consequence: any idiomatic multi-line rewrite of an `if`/`while` body fails
`sgpm fmt --check`. P4 therefore depends on making block formatting
width-aware (D7).

An earlier draft of this document asserted the opposite. That verification was
faulty: `sgfmt <file>` prints to stdout and requires `-w`/`--write` to modify
the file, so an unchanged file was misread as preserved formatting.

## Decisions

### D1 Enum-shaped `Option` and `Result` with a deprecated field surface

`Option<T>` becomes `enum Option<T> { None, Some(T) }` and `Result<T, E>`
becomes `enum Result<T, E> { Ok(T), Err(E) }`.

Direct field access (`.is_ok`, `.is_some`, `.value`, `.error`) is preserved for
one release as compiler-known accessor methods over the enum, each emitting a
deprecation diagnostic with a migration hint. Rationale: 236 stdlib and 195
example/fixture sites read these fields today; a flag-day rewrite would make the
change unreviewable and would strand out-of-tree code.

`option_none_with(placeholder)` and `result_*_with(placeholder, ..)` become
deprecated wrappers that ignore their placeholder argument, then are removed in
the following release. `None` and `Err(e)` need no placeholder.

Struct-literal construction of these types (`Result { is_ok: true, value: v,
error: 0 }`) is rejected once the enum form lands; the diagnostic names the
`Ok`/`Err`/`Some`/`None` replacement.

### D2 `?` type checking is unchanged

`peel_result_ty_static` and `peel_option_ty_static`
(`compiler/src/typeck/check/try_helpers.rs`) recognise these types by name and
type-argument arity, not by field layout, so the propagation contract, the
`PropagationContext` stack, and `try {}` blocks carry over unchanged. The work
is confined to MIR construction/destructuring and Drop glue for the new payload
layout.

### D3 `for` desugars onto the existing `Iterator` protocol

`for pat in expr { body }` lowers to the established iterator loop when `expr`
is a supported collection or an iterator value, and keeps the current direct
lowering for arrays, slices, and ranges (no regression, no extra indirection on
the fast path). Supported receivers in this wave: `Vec<T>`, `VecDeque<T>`,
`HashMap<K,V>`, `HashSet<T>`, `BTreeMap<K,V>`, `BTreeSet<T>`, and any value
whose type provides the `Iterator` protocol.

Iteration borrows by default; `for x in vec` on an owning collection follows the
existing owning-iterator rules already proven for chained terminals, so
mutation-while-iterating stays rejected by the current borrow rules rather than
by a new mechanism.

Map iteration yields entries; `keys()` and `values()` are separate entry points
(D5) rather than overloads of the default iteration.

### D4 Output layering

Default `sgc run` / `sgc build` output on success is limited to program output
plus a single result line. Cache statistics, workset manifests, frontend
scheduler and session lines, generic-instance cache counters, and pass-through
toolchain warnings move behind `--verbose`. Errors and warnings that a user must
act on are never suppressed. `--error-format json` output is unaffected.

Suppressing `clang` `-isystem` unused-argument warnings on a successful link is
part of this: they are an artifact of how the driver is invoked, not a signal
about user code.

### D5 Everyday syntax scope

- `vec![a, b, c]` and `vec![value; count]` are pinned built-in forms lowering to
  `vec_new` plus pushes. Not a general macro facility.
- `println`/`print`/`eprintln` accept a format string plus arguments, routed
  through the existing `format` pipeline, so `{}`, `{:?}`, positional, and
  precision specifiers behave identically to `format`.
- `{:?}` renders `#[derive(Debug)]` shapes in both `format`/`println` arguments
  and f-string interpolation.
- `HashMap`/`BTreeMap` gain `keys()` and `values()` iterators; `HashSet`/
  `BTreeSet` iterate elements directly.

### D6 Diagnostic language

All compiler, `sgc`, and `sglsp` diagnostic text is English, matching the
majority of existing messages, the stable diagnostic codes, and the reference
documentation. Stable codes and JSON shapes do not change. Chinese strings in
compiler diagnostics are translated, not deleted, and the existing stable code
for each message is preserved.

### D7 Block formatting becomes width-aware (P5)

`format_block_inline` falls back to the multi-line `format_block` rendering when
the inline form would exceed `max_width`, activating the already-parsed,
already-validated but currently unread option. Default stays 100.

P5 is an independent priority rather than a P4 sub-task: a formatter that forces
every conditional body onto one line is a defect every user meets daily,
independent of how any particular example is written. P4 does not block on it —
flattening to guard clauses with early `return` is a real readability win under
the current formatter, and was verified to pass `sgfmt --check` unchanged. P5
then removes the ceiling that flattening alone cannot lift.

Rollout is deliberately narrow. The formatter change lands with its own tests,
and the resulting formatting is applied only to the fixtures P4 rewrites. A
repo-wide reformat is a separate follow-up, because 53 of 198 in-tree `.sg`
files contain 251 lines over 100 characters and sweeping them now would collide
with the concurrent `Option`/`Result` migration.

Because previously conformant files can become non-conformant under the new
rule, any `fmt --check` gate that newly fails must be identified before the
sweep is scheduled.

### D8 Pre-existing v1 lockfile defect is out of scope

`sgpm <cmd> --locked` fails for every fixture holding a **v1** lockfile.
`read_locked_registry_graph` (`tools/sgpm/src/lockfile.rs:49-55`) deserializes
the whole document into the v2 shape *before* checking `version`, so a v1
`source = "path+."` string fails against the v2 `source.kind` table and is
reported as `Sengoo.lock is out of date; run sgpm update` — while
`sgpm update --check` on the same file reports it current. The two commands
contradict each other and the message names the wrong cause.

Affected: `cli-json-audit`, `compressed-json-artifact`,
`default-library-conformance`, `http-client-status`, `p0-foundations`,
`workspace-doc-loop`, `package-release-loop`. Unaffected (v2):
`workspace-audit`, `async-channel-smoke`, `http-echo-service`,
`python-hot-path`.

This predates and is independent of this change. Fixtures under it are verified
through the non-locked loop, and their locked-gate task stays open with the
blocker recorded rather than being ticked or silently worked around. Fixing it
belongs to the `sgpm` owner: either reorder the version check before
deserialization, or regenerate the v1 locks — both mutate gate inputs for
in-flight work, so neither is done here.

## Risks / Trade-offs

- **Migration volume (D1).** ~430 in-tree sites plus fixtures. Mitigated by the
  deprecated accessor surface, so the type change and the call-site rewrite can
  land in separate reviewable steps.
- **Drop correctness (D1).** Enum payloads change what must be dropped on each
  arm. The existing exact-once Drop tests must be extended to cover `Option`/
  `Result` payload moves before the struct form is removed.
- **Iteration performance (D3).** Array/slice/range loops keep their direct
  lowering, so the hot path is unchanged; only collection iteration takes the
  protocol path.
- **Output suppression (D4).** Hiding detail can mask real problems. Mitigated
  by keeping every actionable diagnostic visible and by making `--verbose`
  restore the current output exactly.

## Migration Plan

1. Land the enum types with compatibility accessors; both old field reads and
   new constructors compile.
2. Migrate stdlib, then examples and fixtures, to constructors and `match`.
3. Remove struct-literal construction and placeholder constructors; keep the
   deprecated accessors one release.
4. Remove the deprecated accessors in the following release.

## Open Questions

- Whether `Option`/`Result` should additionally gain `map`/`and_then`/`ok_or`
  combinator families in this wave or a follow-up. Current stdlib exposes only
  scalar-specialised helpers; the enum form makes generic combinators
  expressible for the first time.
