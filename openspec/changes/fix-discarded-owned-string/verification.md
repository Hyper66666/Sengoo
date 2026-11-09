# Verification

Host: Windows x64, `D:/Sengoo`, 2026-11-09. This is a development compiler
candidate, not installed-runtime release evidence.

Final compiler:

```text
D:/Sengoo/target/agent-parameter-compiler/debug/sgc.exe
SHA256 69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce
```

Historical compiler `target/agent-final-compiler/debug/sgc.exe` SHA256
`6b40efa12c374adbc48dd7f543ddc83820e00140fa87689b3cb3a35479039c7a`
is preserved. The initial four-part native reproduction measured String deltas
`0 / 5 / 1 / 1` for explicit discarded statements, ignored loop tails, named
eq arguments and fresh eq arguments. Its emitted IR is retained at
`target/discarded-owned-string-before.ll`, and the executable at
`target/discarded-owned-string-before.exe`.

Final machine-readable native evidence:
`target/agent-parameter-compiler/native-safe-evidence.json`.
The evidence stores compiler/clang hashes, 355 compiler/stdlib source-input
hashes, exact commands and outputs, and the unchanged-source guard. Check that
file's actual source-input inventory when comparing later candidates rather
than treating this document as a replacement for the recorded hashes.

Command:

```text
python -X utf8 tools/stdlib/tests/discarded_owned_string_verify.py --compiler D:/Sengoo/target/agent-parameter-compiler/debug/sgc.exe --clang C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe --output D:/Sengoo/target/agent-parameter-compiler/native-safe-evidence.json
```

All eleven build/run/IR steps passed. The O0/O2 programs each verified zero
resource deltas for discarded statements, ignored loop tails, named/fresh eq,
owner return/store and failed propagation, and cross-module hashing/UTF-8 copy.
The hash and copy cases repeat twenty times, check original Strings and Buffers
remain usable, and exercise Chinese/emoji copied through managed buffers.
A later failing argument checks that an earlier produced String is reclaimed
before the function returns. Independent named Result/Option JsonDoc,
discarded JsonDoc and borrowed-close fixtures passed with the same compiler.

Compiler suites passed:

- `cargo test -p sengoo-compiler --lib try_operator_tests --target-dir target/agent-parameter-compiler`: 26 tests.
- `cargo test -p sengoo-compiler --lib drop_flag_tests --target-dir target/agent-parameter-compiler`: 40 tests.
- `cargo test -p sengoo-compiler --lib borrow --target-dir target/agent-parameter-compiler -- --test-threads 2`: 38 tests, with the pinned clang directory on PATH.

These filters overlap; their counts are not a count of unique tests or a claim
that the entire compiler suite ran. Regression coverage includes a true transfer
after a borrow, same-argument aliasing, distinct receiver identities, duplicated
function identity, checked-versus-typed contract mismatch, recursive/unknown
calls, owner/raw-identity escapes, native-name shadowing and extern signature
mismatch. The corrected historical observation fixtures retain genuine consuming
counterparts with exact caller/callee Drop assertions.

All modified Rust files were individually rustfmt-formatted with children
disabled. `sgfmt --check` passed for the independent Sengoo regression using
`target/agent-formatter/debug/sgfmt.exe` SHA256
`d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
Strict OpenSpec validation passed. A whole-repository `cargo fmt --all --check`
also found pre-existing formatting differences outside these owned files; this
change does not claim that unrelated repository-wide gate passed.

The Agent kernel, SQLite ledger and schema integration owner must run their
separate full SDK resource gates against this pin. The ABI observation table is
deliberately limited; any additional native boundary needs an inspected
implementation and independent regression, not a wildcard rule or manual free.

## Fresh reference continuation, 2026-11-09

The full audit's six remaining String owners were traced to the source-level
`&string_new()` calls at the six stream-finalization boundaries. The retained
minimal pre-fix report is
`target/agent-audit-string-diagnostic/fresh-reference-before.json`: twenty
fresh reference calls leaked twenty String handles. The old reference-fresh
executable and source remain alongside it.

The initial HIRExpr::Ref correction did not cover the source Unary::Ref lowering
path. The latter now registers the initialized fresh referent before taking its
address too. The final development compiler for this correction is:

```text
D:/Sengoo/target/agent-parameter-compiler-v2/debug/sgc.exe
SHA256 7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7
```

All 1,176 compiler library tests pass in
`target/agent-parameter-compiler-v2/compiler-lib-final.log`. The new IR regression
checks direct and method borrowed calls, one Drop for a named owner, and cleanup
on both success and a failing later argument. The existing native fixture now
covers direct and method `&string_new()` plus String/Buffer/JsonDoc/Vec and later
argument failure in twenty rounds. Its O0 exact counter preflight passes, with
all four deltas zero, in
`target/agent-parameter-compiler-v2/fresh-reference-preflight.json`.

This identifies and verifies the language cause; it is not a frozen full-SDK
acceptance result. The complete ownership verifier includes this fixture and
hashes runtime headers in addition to C/SG sources. Final full-audit and SDK
acceptance must rerun after the parallel generic-output runtime work freezes.

The final frozen-runtime ownership gate now passes all seventeen steps in
`target/agent-completion/ownership/evidence.json`, including O0/O2 discarded
owners, named Result/Option, borrowed close, fresh reference, native copying,
HTTP/SSE boundaries and emitted IR. Its unchanged-source guard also passes.
The runtime C SHA256 is
`eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5`;
compiler SHA256 remains `7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`.
The same toolchain's `target/agent-completion/foundations/evidence.json` passes
the schema suite and complete thirteen-case audit with exact counters. The
earlier mixed-source ownership report is retained as
`target/agent-completion/ownership/pre-runtime-registry-lock.json`: all behavior
steps passed, but source freeze correctly rejected the concurrent runtime edit.
