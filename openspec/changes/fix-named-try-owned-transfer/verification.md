# Verification, Windows x64, 2026-11-07

Baseline: `86c6f87975646b91c9cf2d7d069ac06ee4243b9b` with preserved local changes.
The adjacent CLI baseline is `b2b85ef2c8f369f2fd69ef8729ab420cc420959a`.

Before the fix, `tools/stdlib/tests/try_owned_result.sg` exited 2 with
`named propagation closed its returned document`. The independent compiler
test also failed: both question-mark branches called the old named wrapper's
Drop helper before returning the transferred payload or error.

After the fix, 14 try-operator compiler tests, 68 Drop-related compiler tests
and native JSON Result success/error and Option liveness checks pass.
Exact compiler/source hashes and logs are in the local artifact
`target/response-retry-20261107/try-ownership-evidence.json`. The compiler SHA256
is `6507e0e2eff15fd375054334f4e4ff6bccf540431f5f81bd56625f7f0d616252`.

An additional native composition sweep found a pre-existing failure in
`explicit_field_drop_free_and_close_suppress_automatic_field_cleanup`.
The sweep completed with 15 passes and this one failure.
Both the old pinned compiler and the fixed compiler print each of 831, 832 and
833 twice for explicit field releases inside a destructor. Its source contains
no question-mark expression. The baseline reproduction is retained in
`target/response-retry-20261107/preexisting-explicit-destructor-baseline.log`.
This separate destructor issue is not counted as a passing regression and the
candidate does not claim a fully passing language-wide ownership gate.

This is a source-development toolchain verification, not a release or a
four-platform qualification. CLI integration and local installation are tracked
in the adjacent change `fix-all-request-retries-and-response-diagnostics`.
