# Local Python binding evidence

## 2026-11-04 reliability and real batch rerun

This section supersedes the earlier assertion-only repeated-call observation.
Exact native handle counters exposed leaked bridge-created String and Vec
inputs in the earlier implementation. The typed bridge now releases its inputs
after invocation, preserving an input handle only while an aliased owned output
is copied. Result failure placeholders and partial native conversion failures
also use explicit cleanup. The earlier ten-thousand-call pass below did not
measure native resources and must not be used as leak evidence.

Host: Windows 11 x64, CPython 3.12.14, Rust 1.94.0, clang/LLVM 19.1.7.
The tool snapshot was rebuilt from the current uncommitted working tree after
the bridge cleanup and NaN comparison fixes. Its base revision remains
`45c789f4fa58fc3cfa5291d73b131b0da07370a6`; the source-development reports have
`candidate_sha: null`. These local results are not exact-candidate or installed
release qualification. A final installed-toolchain run is tracked separately
in the candidate evidence.

| Executed check | Result |
| --- | --- |
| Python backend/wrapper/artifact unit tests | 15 passed, including late conversion failure, platform tags and corrupt wheel RECORD rejection |
| Typed bridge Rust/native tests | 6 passed, including rejected exports, String/Vec allocation failure, partial second input conversion, output failure and input/output aliases |
| Wheel construction | Two independent copied projects and their source archives built with locked renamed native C dependencies |
| Dependency repair | delvewheel 1.13.2 bundles `vcruntime140.dll`; final platform tags and every file's RECORD hash/size validated |
| Offline consumer | Both repaired wheels installed using `--no-index --no-deps --only-binary=:all:` in a fresh venv; install and execution PATH exclude Sengoo, Rust and C compilers |
| Functional batch checks | Numeric lengths 0, 1, 31, 1024, 65536; Unicode; exact i64 bounds; Result errors; output aliases; void-owned inputs; partial Python conversion failure |
| CSV application | 10,003 rows for batch sizes 127, 1024 and 4096; quoted/newline/Unicode fields, actual command invocation, empty input and late invalid-row atomic rollback |
| Native resource observation | 20,000 iterations per module, five measured calls per iteration: 200,000 total calls; String/opaque collection counts return to the baseline at all ten checkpoints |
| Module isolation | Distinct implementation identities, private runtime symbols and mutated descriptor import rejection passed |
| Workflow/static checks | actionlint 1.7.12 and `cargo fmt --all --check` passed |

The resource loop includes normal returns, explicit Result failures and
interrupted conversion paths. Exact counters cover String and opaque collection
handles, not every possible native allocation. Python tracemalloc excludes
native memory and is descriptive only; no RSS threshold or general no-leak
claim is inferred. Separate native failure probes are included in the remote
sanitizer gate.

Latest repaired source-development wheels:

| Artifact under `target/python-daily-final-wheels/` | SHA-256 |
| --- | --- |
| `sengoo_batch_a-0.1.0-py3-none-win_amd64.whl` | `6edcb73f32296ecc673a1719a6f66eccf653620a8055365f00ce4b40599bfbde` |
| `sengoo_batch_b-0.1.0-py3-none-win_amd64.whl` | `cef8ba5af5a9ac75a49e49ae5f1b28593ef0c7ada51c4f6184d74080b8af102e` |

Reviewable repair, consumer, resource and call-cost reports are stored in
`evidence/2026-11-04-windows/`. Timing samples include all Python/native
conversion and vector copying. On this host the copied affine operation is
approximately 9–10 times slower than a Python list at 1,024 and 65,536 elements;
the empty Sengoo call costs approximately 2 microseconds. These are descriptive
measurements, not a speedup claim or a portable timing threshold. The current
ctypes/vector path needs further optimization before simple arithmetic is a
reason to adopt it.

The wheel workflow builds native dependencies in four hosts, uses an immutable
manylinux container, and installs on CPython 3.11–3.14. Every checkout and artifact
is tied to the candidate SHA. Its final `python-wheel-gate` requires all build
and consumer jobs to succeed. Remote platform results, sanitizer success and
the independent two-week user pilot are not claimed by these local checks.

## Earlier 2026-11-03 checks (historical)

Host: Windows x64, 2026-11-03, CPython 3.12.14, Rust 1.94.0,
clang/LLVM 19.1.7 and MSVC BuildTools 14.44.35207.

Base revision: `45c789f4fa58fc3cfa5291d73b131b0da07370a6`.
These tests cover the uncommitted implementation on that base, not a published
candidate. `sgc` and `sgpm` report `0.2.1-dev.0 (45c789f4fa58)`.

| Executed check | Result |
| --- | --- |
| `cargo +1.94.0 test -p sgc --bin sgc python_bindings::tests` | 4 passed, including deterministic native allocation/push failure cleanup |
| `cargo +1.94.0 test -p sgc --test shared_library` | 2 passed: C export survives without main; private/runtime exports and incompatible flags rejected |
| `python -m unittest discover -s tools/sengoo-python/tests` | 12 passed: bounds, Unicode, conversion failures, ownership cleanup, build identity, Linux/macOS tags, source archive local dependency validation |
| `python tools/sengoo-python/tests/wheel_smoke.py --build-dir target/python-dependency-wheels` | Two independent projects copied outside the checkout, locked, and built through pip's PEP 517 frontend; both source archives also rebuilt into wheels |
| `python tools/sengoo-python/tests/wheel_smoke.py --verify-dir target/python-dependency-wheels` | Offline installation into fresh venv; tools/source overrides removed, PATH reduced, `-I` consumer passed |
| Unified native linker C export probe | Passed using a copied compiler built after the common linker refactor |
| `openspec validate m4-python-native-packages --strict` | Passed |

The native consumer checks numeric scalars, checked integers, booleans, UTF-8
borrowed/owned strings, copied integer/float vectors, empty vectors, unit input
and output, `Result<i64, i64>`, `Result<(), i64>`, explicit error codes and
keyword arguments. Invalid NUL, Unicode, types and overflowing integers fail
before native invocation. Ten thousand repeated string/vector calls passed.

Two modules use distinct function implementations (`module_identity()` returns
1 and 2), both remain callable together, and their runtime symbols are hidden.
A modified binding descriptor is rejected on import. Generated `.pyi`, library,
descriptor, wrapper and `py.typed` are included in each wheel.

Each source build also includes a renamed path dependency with native C source;
the source archives preserve its manifest, source, native file and locked graph.
Building both wheel archives after extraction passed without rewriting the lock.

Local output checksums (these are source-development artifacts):

| Artifact under `target/python-dependency-wheels/` | SHA-256 |
| --- | --- |
| `sengoo_batch_a-0.1.0-py3-none-win_amd64.whl` | `7eb4f94371e312e344f36051aa69fc1e5c7ba62997bc1be462803f70f5d335c2` |
| `sengoo_batch_b-0.1.0-py3-none-win_amd64.whl` | `3bfdea75055d0a9bc6052d78af4c62aef766d9fd6c2db4580c4552e821d41eb9` |

The backend wheel is `target/python-backend-wheel/sengoo_python-0.1.0-py3-none-any.whl`,
SHA-256 `97ae2fba1b445224d62b043f7e11c4650109813805cea602b9febbe584d62cf4`.

After the final compiler ownership fixes, both external projects and their
source archives were rebuilt using the checksum-installed toolchain outside
the checkout, with the default **installed** runtime mode. Fresh offline wheel
installation and the complete two-module consumer passed again. The log is
`target/installed-python-wheel-evidence.log`; delivery uses these newer wheels:

| Artifact under `target/installed-compiler-wheels/` | SHA-256 |
| --- | --- |
| `sengoo_batch_a-0.1.0-py3-none-win_amd64.whl` | `3943bf945b11f45c8bfd9fcc6c0b8043dea891e2c19901dea092426aadf0ce91` |
| `sengoo_batch_b-0.1.0-py3-none-win_amd64.whl` | `55420e14384bc00cf5adeb60e769bb42b3cdebc718f167a16a25148f006e3952` |

`.github/workflows/python-wheels.yml` configures four native hosts and installed
consumers on CPython 3.11–3.14, with a pinned manylinux container, auditwheel
repair, macOS delocate and native bridge sanitizer probes. Those remote jobs
have **not** run for this working tree. Neither the configured jobs nor these
Windows checks prove the remaining hosts, sanitizer gate, release qualification
or independent two-week pilot acceptance.
