# Daily development and Python ecosystem

Approved implementation baseline: `45c789f4fa58fc3cfa5291d73b131b0da07370a6` on main.
Stable compatibility input: v0.2.0, revision `92c8f399f61b73d63990581c637da68572b6e133`.

## Outcome

Native CLI developers can independently install, create, edit, test and ship projects.
Python users can install a platform wheel and call compiled Sengoo without build tools.
Preserve edition 2026, v0.2 sources and accepted manifest/lock/diagnostic/runtime ABIs.

## Ownership and order

| Stage | Active OpenSpec owner | Required output |
| --- | --- | --- |
| M0 | m0-development-baseline | Reconcile the native development baseline |
| M1 | m1-daily-developer-tools | Make daily editor and project operations reliable |
| M2 | m2-native-development-foundation | Verify ownership composition and native artifact builds |
| M3 | m3-official-cli-data-sqlite | Ship owning CLI, data and SQLite packages |
| M4 | m4-python-native-packages | Build typed Python bindings and installable wheels |
| M5 | m5-ecosystem-distribution | Distribute packages and prove external consumption |

M0 -> M1 -> M2 -> M3/M4 -> M5. Child tasks contain executable completion checks.
An implemented feature, a locally passing test, configured CI and remote release
proof are different facts and are recorded separately in the evidence ledger.

## Interfaces

- `sgc doctor [--format text|json] [--profile native|python]`: schema-v1 diagnostics,
  native compile/link probe in a temporary directory, actionable remediation, no auto-install.
- `sgc build --emit-shared` and `sgpm build --shared`: explicit library build,
  PIC on POSIX and selected exports as roots. Existing executable builds unchanged.
- `--python-export` selects existing checked synchronous nongeneric functions.
- `sgcli`, `sgdata`, `sgsqlite`: independent official packages at 0.1.0.
  SQLite is vendored 3.54.1; extension loading is disabled. Statements retain their
  connection, result bytes are copied, transactions roll back on Drop and reject nesting.
- `sengoo-python`: PEP 517 backend configured by `[tool.sengoo]` manifest/module/exports;
  produces ctypes PyDLL wrappers, stubs and self-contained platform wheels.

Python v1 supports i64/f64/bool, input &str/String, output String, copied Vec<i64/f64>,
unit and return Result<T,i64>. Reject NUL strings, borrowed returns, ADTs, callbacks,
async and generic exports. Validate and clean up all conversion failures. Result
errors become SengooError with a code; native process traps are not exceptions.
Each module has checked binding ABI metadata and isolated native symbols.

## Ecosystem defaults

Keep packages in this monorepo and distribute verified source archives and a relative
local-registry bundle. Do not introduce Git subdirectory dependencies or run a public
registry service. sgcli parses separately from printing/exiting. sgdata bounds input
sizes and preserves i64 precision; CSV supports quoted newlines, CRLF and escaped quotes.

Two external consumers: persistent CLI file/CSV indexer and a Python batch numeric
module. Four native hosts match current releases; Python validation covers 3.11-3.14.
Linux wheel tags require actual manylinux builds and inspection; macOS tags state the
real deployment target. GUI, game, portable backends and HTTP framework expansion are
outside this change.

## Acceptance and external work

Existing six main gates and the four-host release matrix remain required. Do not claim
remote jobs have passed without recorded runs. Human pilot requires 3-5 independent
users and two weeks of real observations. Public publication and pilot invitations
are separately triggered deliverables, not simulated acceptance evidence.

Planning estimate: 12-18 full-time engineer weeks, subject to discovered soundness
issues. Track implementation truth in `docs/development-evidence.md`.
