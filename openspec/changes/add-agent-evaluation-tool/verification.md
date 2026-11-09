# Verification

See `tools/sgeval/EVIDENCE.md`, `target/sgeval-final-evidence.json` and
`target/sgeval-final-offline.json`. On Windows x64, baseline
`ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus working changes, compiler
`f7ea03a89c27ccedf8e6f2f8c81fe7ab6008887dd028fd8848102774e2f5dde5` and
formatter `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`
pass the final native 20-iteration resource-baseline check, seven public offline
groups, formatting and dedicated strict OpenSpec validation.

The package is copied outside the checkout and built by `sgpm build --locked`;
its lockfile remains unchanged. Public CLI checks run with compiler and
source-runtime overrides removed and System32-only PATH. The final binary
SHA256 is `308d12387d99915561afb3457ce84fd14ff19c71a26debc69aaeeb3b786bec8f`.

No replay execution capability exists. A fixture records a `run_command` event
that would write a sentinel; the sentinel remains absent after repeated replay.
The exact same recording produces byte-identical JSON reports. Failed assertions
are nonzero with failed reports; malformed data and capacity failures cannot
create successful reports or overwrite preexisting output. Raw and escaped NUL
is rejected recursively before any C-compatible string view, including keys,
redactions and assertion payloads. NUL-bearing strings are unsupported.

`tools/sgeval` owns the complete Sengoo record/redact/replay/assert logic;
Python is only the external test driver. This closes the standalone experimental
evaluation slice, including its external locked consumer check. It does not
establish live-model, installer VM, release-host or full Agent SDK gates.
