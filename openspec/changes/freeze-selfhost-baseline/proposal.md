# Freeze Windows self-hosting compatibility and performance inputs

## Why

SG self-hosting requires a retained native reference and a broader compatibility
corpus than the initial source/span/arena tests. Default cutover also needs
same-host measurements against qualified reference tools rather than comparing
an early SG build to an unrelated debug compiler or old report.

## Changes

Add an explicit authoritative corpus/workload manifest, a Windows reference and
candidate measurement runner, and documented release-qualification boundaries.
Freeze file identities and raw one-warmup/ten-sample workload observations with
OS root peak working sets and bounded descendant sampling. Candidate runs use
interleaved reference/candidate order and reject time or memory regression.

## Ownership and scope

This child owns M0 corpus inventory, measurement protocol and qualification
binding. It does not implement seed rebuilding, the compiler, the native SDK or
the correctness/installation/other-host gates named by the inventory. The
`self-hosting-program` umbrella owns milestone integration and final cutover.
