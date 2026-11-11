# Process-profile SG collection registry

## Why

The explicit-context SG RawVec core proves the algorithms but is not a process
runtime. Current source lowering cannot execute static/TLS variables, and existing
opaque, String, Buffer, Rc/Arc and async handles have distinct lifecycle domains.
A new opt-in process profile is needed before integrating any real runtime family.

## What changes

Add one process/DLL-local SG registry with 128 slots, primitive storage-address and
OS-lock leaves, and named wrappers for an owning word-block and the existing SG
RawVec core. Use an explicit negative-token range disjoint from the valid positive
context/reference opaque profile. Qualify metadata thread stress, callback reentry,
resources, mixed-profile rejection and two fresh SG-linked deployments.

## Ownership and scope

Owner: `/root/selfhost_core`. Capability: `selfhost-process-collection-registry`.
This child owns the new process-profile algorithms and qualification only. The
existing context core, namespace2/provider/source-lowering contracts, stdlib,
reference runtime globals and installed SDK are unchanged. Full runtime migration,
unbounded tables, final SDK packaging, self-build and M8 remain open.
