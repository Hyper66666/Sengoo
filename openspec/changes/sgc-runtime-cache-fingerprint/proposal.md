## Why

`sgc run` and `sgc build` currently treat the runtime C source path as the
runtime linkage identity. Editing `tools/stdlib/runtime.c` without changing a
Sengoo source file can therefore produce a cache hit that executes or ships an
old native runtime. The runtime object cache has a second, narrower gap: it
keys on path, byte length, and second-resolution modification time, so a
same-length edit within one second can reuse stale object code.

## What Changes

- Fingerprint runtime C source bytes whenever `sgc` prepares run/build cache
  identity.
- Store that fingerprint in run/build metadata and require an exact match
  before returning cached artifacts.
- Include the byte fingerprint in runtime object-cache identity instead of
  relying on file length and second-resolution modification time.
- Report runtime-source drift as an explicit cache-miss reason.
- Treat older metadata without a runtime fingerprint as stale when a runtime C
  source is present.
- Add focused regression coverage for run keys, build keys, diagnostics, and
  runtime-object identity.

## Impact

- Affected spec: `frontend-build-performance`
- Affected code: `tools/sgc/src/{model_types,workset,native_toolchain,commands/run,commands/build}.rs`
- Dependencies: none
- Syntax or stdlib API changes: none
