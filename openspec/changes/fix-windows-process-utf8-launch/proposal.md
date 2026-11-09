# Preserve UTF-8 in Windows synchronous process launches

## Why

The M1 bootstrap driver passed a source path containing `世界`, spaces and `&`
to the reference compiler. The child received mojibake and could not read the
source. `ProcessCommand.run` used `CreateProcessA` although its arguments are
UTF-8; owned asynchronous launches already use strict UTF-16 conversion and
`CreateProcessW`.

## Changes

Use the existing Windows wide command-line, working-directory and environment
boundary for synchronous `ProcessCommand.run`, including both stages of its
pipeline. Preserve the existing argument quoting, capture, timeout and ABI1
behavior. Add real native and SG regression evidence without weakening the
bootstrap Unicode-path test.

## Scope and ownership

This change owns the synchronous `ProcessCommand` Windows launch boundary.
It does not claim the older fixed-three-argument `process_run` helper, a new
compiler seed, a minimal SDK, POSIX changes or SG self-hosting completion.

The implementation owner is this change; the self-hosting foundation gate is
an independent consumer. A source checkpoint and lock update follow passing
verification because the actual source runtime has changed.
