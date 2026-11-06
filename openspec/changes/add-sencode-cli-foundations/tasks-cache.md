# Compiler cache identity implementation owner

- [x] Guard build/run/frontend/generic persisted cache boundaries with the running executable SHA-256, rejecting missing identities.
- [x] Verify same-content moved binaries reuse and changed-content same-version binaries rebuild through actual compiler invocations on Windows x64.
- [x] Run 7/7 cache unit tests, compiler replacement integration and native transitive-header build/run cache reuse integration on Windows x64.

Owner: process/system/package implementation. This regression was exposed by
receiver ABI repairs reusing a same-version compiler's previous native output.
