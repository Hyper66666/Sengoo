## Why

Build typed Python bindings and installable wheels for native CLI and Python developers while preserving v0.2 compatibility.

## What Changes

- Generate typed bridge metadata from checked nongeneric synchronous function types.
- Implement scalar/string/numeric-vector copies, owned cleanup and Result error conversion.
- Implement sengoo-python PEP 517 backend, ctypes PyDLL wrappers, stubs and platform wheels.
- Validate module ABI isolation, unsupported signatures, bounds and failure cleanup.
- Add Python batch consumer and four-host CPython 3.11 through 3.14 wheel jobs.

## Capabilities

### New Capabilities

- `python-native-packages`: Build typed Python bindings and installable wheels.

## Impact

The approved native daily-development program; existing experimental backend scope is unchanged.
