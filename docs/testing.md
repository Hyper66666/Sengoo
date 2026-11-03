# Testing Sengoo packages

In `0.2.1-dev.0`, `sgpm test` and `sgpm fmt` select the current project and its
development path dependencies. They stop at registry and Git dependency
boundaries, preserving immutable package contents and locked checksums. To run
an official package's own tests, extract its source archive and select that
package directly. Dependency source is still typechecked and built for consumers.

`sgc test` supports both the existing file-entry convention and function-level
test discovery. Language support is defined in [language-reference.md](language-reference.md)
and [the support matrix](../examples/realworld/SUPPORT_MATRIX.md). The runner's
executable contract is tested in `tools/sgc/src/commands/test.rs` and the
`sgc` native integration tests; current branch evidence is in
[development-evidence.md](development-evidence.md).

Repository distribution tests require PowerShell 7 (`pwsh`) on PATH on every
host, matching the release workflows. They exercise archive manifests,
checksums and strict release evidence; Windows PowerShell 5 is not the test host.

## File tests

Every `tests/**/*.sg` file with a `main` function is one test case. Packages can
also declare extra files with `[[test]]` entries in `Sengoo.toml`.

```sg
def main() -> i64 {
    if 2 + 2 == 4 { 0 } else { 1 }
}
```

Exit status `0` passes; a nonzero status fails.

## Function tests

A test file without `main` can define one or more zero-argument synchronous
functions whose names start with `test_`:

```sg
def helper(value: i64) -> i64 {
    value + 1
}

def test_increment() -> i64 {
    if helper(41) == 42 { 0 } else { 1 }
}

def test_zero() -> i64 {
    if helper(-1) == 0 { 0 } else { 1 }
}
```

`sgc test` runs these as separate cases named
`tests/file.sg::test_increment` and `tests/file.sg::test_zero`. Each function
can return an `i64` process status, a `bool` (true passes), or unit. Generated entry wrappers are temporary and
are removed after the case runs.

Use `--filter test_increment` or
`--exact tests/file.sg::test_increment` to select a function case.

## Output

Text is the default:

```text
test ok tests/file.sg::test_increment
test result: 1 passed
```

`sgc test --format json` keeps the existing schema version and fields. Function
cases add an optional `function` field:

```json
{
  "schema_version": 1,
  "passed": 1,
  "failed": 0,
  "total": 1,
  "tests": [
    {
      "name": "tests/file.sg::test_increment",
      "path": "tests/file.sg",
      "function": "test_increment",
      "ok": true
    }
  ]
}
```

`std::assert` failures continue to include the helper, message,
expected/actual values, and source file/line when available.

## Attributes, fixtures and coverage

A function can use `#[test]` instead of a `test_` prefix. A supported parameter
case has one argument and uses `#[case("label", expression)]`; each case becomes
a separate invocation. Put attributes and function declarations on their own
lines because discovery currently uses the runner's bounded line parser.

```sg
#[case("zero", 0)]
#[case("positive", 42)]
def nonnegative(value: i64) -> bool {
    value >= 0;
}
```

Optional synchronous `setup()` and `teardown()` functions run around each
discovered function case. Teardown is called after a normal return; process
termination or a native trap does not guarantee teardown. Use owned values and
Drop for resource cleanup.

`sgc test --coverage --format json` adds executable-line coverage to schema v1;
it is not branch coverage. `--nocapture` displays program output directly.
Files that define `main` keep the legacy one-file/one-test behavior even if they
also contain attributed functions. Fixtures and parameter cases are not a
general procedural-macro or arbitrary fixture-dependency system.
