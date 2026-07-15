# Retained compatibility projects

Each subdirectory is a package frozen at the oldest currently supported source
surface. CI copies it outside the checkout and runs the locked package loop
with both the named released toolchain and the current toolchain.

- `v0.1.0-rc.1`: edition 2026, manifest schema 1, lockfile v2, library import,
  file-level test, formatting, documentation, and native build.

Compatibility fixtures avoid newly added language or stdlib APIs. Expanding a
fixture is a compatibility decision and must preserve execution under its
named release.
