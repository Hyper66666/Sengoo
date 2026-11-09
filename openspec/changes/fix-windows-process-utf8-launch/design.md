# Wide synchronous process boundary

## Implementation

Keep the current UTF-8 CRT argument encoder and explicit `cmd /c` command-tail
handling. Convert the resulting command line and optional working directory
using `sengoo_process_utf16`, and construct the environment with the existing
`sengoo_process_wide_environment` helper. Declare these shared helpers before
the synchronous implementation instead of maintaining an ANSI-only environment
builder.

Complete conversion before creating pipes or a child. Reject malformed UTF-8,
failed environment construction and Windows-overlong command lines through the
existing failed-launch result. Use `STARTUPINFOW`, `CreateProcessW` and
`CREATE_UNICODE_ENVIRONMENT`; free all temporary wide allocations on each
return. Leave capture/drain, timeout and pipeline ownership logic unchanged.

Asynchronous owned/piped/detached launches already use wide APIs and retain
their behavior. The older separate `process_run` convenience path remains
outside this change's claim.

## Verification

The standalone C regression uses the existing isolated opaque-token fixture
with real Windows child processes. A child copied to a Unicode executable path
checks its wide argv, working directory and Unicode environment names/values.
Tests cover spaces, quotes, `&`, empty arguments, trailing backslashes,
inherited/cleared environments, 120 malformed UTF-8 launch attempts, capture,
pipeline stdin, timeout and exact native/OS handle recovery across 30 rounds.

A separate SG program built by the unchanged verified seed uses the actual
production source runtime, performs 20 synchronous launches and checks String,
Buffer, opaque and process resource baselines. The Python runner records
source/tool hashes, native identities, commands, raw output and limitations in
`target/selfhost/process-utf8/`. It does not print environment values.

The complete M1 bootstrap gate must subsequently rebuild with this source
runtime and retain its original Unicode-path fixture. Updating the frozen
source identity does not change or certify another seed executable.
