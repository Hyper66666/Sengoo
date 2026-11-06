# Borrow-checking limitation found by the native CLI

The Windows development candidate still conservatively rejects this owned
`String` return with `borrow-escapes-owner`:

```sg
import std::string;
import std::path;
import std::file;

def find(value: &str) -> Result<String, i64> {
    loop {
        let candidate = path_normalize_string(value)?;
        let lower = str_to_ascii_lower(candidate.as_str())?;
        if lower.len() == 0 { break; };
        if file_exists(candidate.as_str()) {
            return Result { is_ok: true, value: candidate, error: 0 };
        };
        break;
    };
    string_result_err(5)
}

def main() -> i64 { 0 }
```

This is an incorrect rejection of an owned value. The outer borrow-checking pass
revisits a nested binding after its type scope has been popped, and the condition
retains a temporary `as_str()` loan. A correct repair needs binding identities
and scoped type information; weakening escape checks by name would risk accepting
real escaping references and shadowed bindings.

The matching candidate was checked with `sgc --runtime-mode source-development
check`. Separating the condition into a local passes:

```sg
let exists = file_exists(candidate.as_str());
if exists {
    return Result { is_ok: true, value: candidate, error: 0 };
};
```

Returning `candidate.clone()` also passes and is used by the CLI candidate. It
creates an independent owned string and incurs a copy. This remaining limitation
does not bypass borrow checks or change the runtime ABI. It is not counted as a
fixed compiler regression in the candidate's passing test totals.
