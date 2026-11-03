# Environment diagnosis

Requires the development toolchain `0.2.1-dev.0` or a later release containing
this command. Run `sgc doctor` before creating a native project, or
`sgc doctor --profile python` before building Python wheels from source.
`--format text` is the default; `--format json` writes a single JSON report to
stdout. Exit status is zero only when every required check succeeds. Failure
diagnostics may also be printed to stderr.

Doctor checks coherent versions/build identities of `sgc`, `sgpm`, `sgfmt`
and `sglsp`, required standard-library modules, runtime ABI and installed
runtime checksums, clang 15+, and Windows MSVC/SDK availability. It then builds,
links and runs a minimal Sengoo program in a unique temporary directory using
the selected runtime mode. The probe is removed afterwards. It does not install
dependencies or change system configuration.

The Python profile also checks CPython 3.11–3.14 and importability of the
`sengoo_python` backend. Set `SENGOO_PYTHON` to choose an interpreter. Installing
a precompiled application wheel does not require this developer environment.

JSON schema v1 is independent of language diagnostics and reflection metadata:

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | integer | Always `1` for this report format |
| `profile` | string | `native` or `python` |
| `target` | string | Host target triple |
| `toolchain` | string | Compiler version with build identity |
| `runtime_abi` | integer | Required runtime ABI, currently `1` |
| `ok` | bool | Every required check passed |
| `checks` | array | Objects containing `id`, `ok`, `detail`, and optional `fix` |

Sibling tools beside `sgc` take precedence over PATH. A mixed version is a
failure even when each tool starts independently. The native probe tests the
actual runtime/linker combination; finding a clang executable alone is
insufficient. Source contributors use
`sgc --runtime-mode source-development doctor`; installed users use the default
runtime mode and the complete archive layout.

Proof: doctor JSON tests in `tools/sgc/src/doctor.rs`, end-to-end probe and fresh
projects in `scripts/verify_daily_development.py`, and current outcomes in
[development-evidence.md](development-evidence.md).
