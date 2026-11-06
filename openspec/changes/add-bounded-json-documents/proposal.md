## Why

Real media requests exceed the legacy 1 MiB JSON parse/copy bound before the
explicitly bounded transport can start. Raising the legacy constant would
silently expand every existing caller's admission policy.

## What Changes

Add explicit 1..16 MiB document constructors and parsers. Retain the selected
limit through copy and serialization, bound payload/container ownership and
aggregate extended-document admission, and provide length-aware string input.
Keep legacy parse/copy limits and runtime ABI 1 unchanged.

## Ownership

Sengoo foundation owns this requirement; Sencode owns complete provider request
admission and transport verification. No media UI or PDF engine is added here.
