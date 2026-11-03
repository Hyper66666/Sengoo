# Implementation and verification

- [x] 1. Generate typed bridge metadata from checked nongeneric synchronous function types.
- [x] 2. Implement scalar/string/numeric-vector copies, owned cleanup and Result error conversion.
- [x] 3. Implement sengoo-python PEP 517 backend, ctypes PyDLL wrappers, stubs and platform wheels.
- [x] 4. Validate module ABI isolation, unsupported signatures, bounds and failure cleanup (Windows CPython 3.12).
- [x] 5. Add Python batch consumer and four-host CPython 3.11 through 3.14 wheel jobs.
- [x] Validate this change and repository OpenSpec strictly (51/51).

Local implementation proof is `tools/sengoo-python/EVIDENCE.md`. Configuring the
four-host / four-interpreter jobs does not complete their execution. Their same-SHA
remote success, repaired manylinux/macOS wheels and clean-host runtime evidence
remain required external gates under M5.
