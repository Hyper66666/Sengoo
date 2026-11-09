# Seed lock and SG self-hosting foundations

## Why

The self-hosting program needs a reproducible first entry that uses known
compiler/runtime bytes and proves useful compiler data structures in SG before
the frontend and native backend are ported. An unpinned shell invocation or a
hello-world executable would not establish that foundation.

## What changes

Add a bootstrap identity lock, an explicit stage0 entry/build procedure and SG
source storage, byte spans, symbol interning and node arenas under `selfhost/`.
Supply native positive, invalid-input, boundary and resource-lifecycle checks
plus an evidence-producing verifier. The lock and test protocol are bootstrap
internals and do not replace published package/runtime schema versions.

This child owns M1 foundation behavior and the seed identity portion of M0.
The umbrella `self-hosting-program` owns milestone integration and final gates.

## Impact and limits

Stage0 still uses the frozen Rust seed, clang and current runtime. Source runtime
overrides require `sgc --runtime-mode source-development` before the subcommand.
The first entry is not an SG compiler, a native backend, a final minimal SDK,
or a proof that Rust/LLVM have been removed. Existing installed tools and
source/edition/runtime ABI behavior remain unchanged.
