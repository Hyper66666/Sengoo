# Testing Sengoo packages

`sgc test` supports both the existing file-entry convention and function-level
test discovery.

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
must return an `i64` process status. Generated entry wrappers are temporary and
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

## Current limits

`#[test]` attributes, setup/teardown fixtures, parametrized cases, and
`--coverage` are not implemented yet. Files that define `main` keep the legacy
one-file/one-test behavior even if they also contain `test_*` functions.
