# Windows process verification, 2026-11-07

Baseline `86c6f87975646b91c9cf2d7d069ac06ee4243b9b`, local changes preserved.
Both forward-slash executable rejection and cmd nested-quote corruption were
reproduced independently. The native C process/terminal probe passed flood,
timeout, Unicode, stdin, descendant cleanup and exact Windows shell invocation
checks. Source and probe hashes are in
`target/response-retry-20261107/process-evidence.json`.

The adjacent CLI passed its executor suite including a real batch in a Unicode
path with spaces, quoted values and URL arguments. Its candidate then passed
13 native suites and 9 exact executable production suites, including Responses
tools/Bash/restart and actual read/patch/Bash-test/next-turn checks. Evidence:
`D:/Sencode-CLI/target/response-retry-20261107/native-evidence.json` and
`production-evidence.json`. The installed candidate's SHA256 is
`312a3e5219c1732d39a33273d9ac2f8fd8d07e212e683201771194abe04c2a83`.

Executable normalization is Windows-only; argument strings and POSIX execution
retain their behavior. This adds no runtime ABI. Validation is local Windows
x64 source-development evidence, not an unrun platform or published release.
Strict OpenSpec validation passed for all 73 Sengoo items.
