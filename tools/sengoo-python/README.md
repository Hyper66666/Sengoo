# sengoo-python 0.1.0

PEP 517 source-build backend for Sengoo. A generated wheel contains its native
library, typed binding descriptor, Python wrapper, `py.typed` and `.pyi` stubs.
Wheel installation runs no build hooks and needs no Sengoo, Rust or clang.

```toml
[build-system]
requires = ["sengoo-python==0.1.0"]
build-backend = "sengoo_python.backend"

[project]
name = "my-numeric-module"
version = "0.1.0"

[tool.sengoo]
manifest = "Sengoo.toml"
module = "my_numeric_module"
exports = ["calculate"]
```

The manifest must define `[lib]`; run `sgpm update` to generate its lockfile.
Install this backend, then run `python -m sengoo_python check` and
`python -m build --wheel`. Development builds can use `--no-isolation` after
installing the backend from this directory. `SENGOO_SGPM` and `SGPM_SGC` select
installed tool executables. For toolchain development only,
`SENGOO_PYTHON_RUNTIME_MODE=source-development` selects the source runtime.

Source archives preserve the lockfile and local dependency sources. Path
dependencies and local registry bundles must live inside the Python project;
`build_sdist` rejects external paths that an archive would omit. CI rebuilds
wheels from the resulting source archives as well as from project directories.

Binding support requires toolchain **0.2.1-dev.0 or newer** containing
`--emit-shared` and `--python-export`; published v0.2.0 binaries do not implement these flags.
The supported interpreter range for this first backend is CPython 3.11–3.14.

Supported arguments are checked `i64`, `f64`, `bool`, UTF-8 `&str`/`String`,
copied `Vec<i64>`/`Vec<f64>` and unit (`None`). Supported returns are these owned/scalar types,
unit (`None`) and `Result<T, i64>` (`SengooError.code` on failure). Borrowed
returns, user objects, callbacks, async and generic exports fail the build.
Strings containing NUL fail before calling native code. Invalid Unicode and
out-of-range integers also fail before native code. Functions retain the GIL.
Native traps/process termination are not represented as recoverable exceptions.

Each module checks its private binding ABI identity on import. Bridge cleanup
owns native allocations, including Result error placeholders. Existing runtime
ABI and reflection metadata are unchanged. Libraries hide runtime symbols to
isolate independently loaded modules.

The bridge also releases the owned String/numeric-vector inputs it creates.
When a function returns one of those input handles, cleanup retains that handle
until Python has copied the output. Normal, unit and explicit error returns all
use this cleanup. Custom objects and nested vectors remain rejected, so this
ownership rule does not extend to arbitrary aggregates.

Native wheels must be produced on their target platform. Linux output starts
with `linux_*`: build in the pinned manylinux image in the wheel CI and run
`auditwheel repair` to obtain verified manylinux tags. macOS deployment tags
come from the native binary's Mach-O deployment version and host architecture. No cross-wheel
retagging is performed by this backend.

The workflow separates raw and delivered wheels and records dependency repair,
platform tags, RECORD hashes and final checksums. Windows wheels vendor their
non-system DLL dependencies with delvewheel. Only repaired outputs are passed
to the four-host CPython matrix. The consumer installs offline and runs copied
batch application scripts with build tools absent from PATH; resource and call
cost reports carry the candidate SHA. Native handle counters are exact for the
tracked String/opaque resources. Python memory samples and host-local timings
are descriptive and carry no portable threshold.

See [the external batch consumer](../../examples/realworld/python-batch) and
run tests with `python -m unittest discover -s tests` from this directory.
