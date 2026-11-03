# Daily-development evidence ledger

## Baseline

- Host: Windows x64, 2026-11-03 (Asia/Shanghai).
- Implementation base: `45c789f4fa58fc3cfa5291d73b131b0da07370a6`.
- Stable release: v0.2.0 at `92c8f399f61b73d63990581c637da68572b6e133`.
- Published evidence: https://github.com/Hyper66666/Sengoo/releases/download/v0.2.0/release-evidence.json
- Stable six required gates and four package/transition hosts are recorded in that
  immutable release manifest. That is historical release evidence, not evidence
  for this new working tree.

## Current implementation

Development version: `0.2.1-dev.0`; edition 2026 and runtime ABI 1 remain unchanged.
The first-round table below is historical local evidence for the implementation
on the base revision. It does not certify a later candidate. A committed
candidate's full identity is retained in its toolchain build manifest,
`DELIVERY.json` and the individual evidence reports. The
[candidate validation procedure](candidate-validation.md) keeps local execution,
remote execution and stable release qualification separate.

| Executed check | Outcome / scope |
| --- | --- |
| Rust 1.94.0 repair; clang 19.1.7; MSVC 14.44.35207 + SDK | Toolchain works on Windows x64 |
| `cargo test --locked -p sengoo-compiler --lib` | Initial local baseline: 1123 passed |
| Final compiler library and native ownership suites | Compiler 1132/1132, ownership composition 15/15, ordered collections 12/12 passed after the cleanup fixes |
| `cargo test --locked -p sgc --bin sgc -- --test-threads=2` | Final 506/506 passed; includes the repaired collections example, native runtime composition and typed bridge tests. Legacy example helpers may internally return early for unavailable optional environments; the required new native acceptance and sanitizer gates fail closed |
| `collections_iteration` and the original stdlib collections smoke | Seven legacy enumeration families support repeated next/exhaustion with cleanup; original example restored output 60; 110 stdlib compiler surface tests passed |
| `cargo +1.94.0 test -p sglsp --quiet` | 190 main LSP tests + 1 evidence verifier passed; semantic identity/version/readonly rename, queued incremental composition, reversed workers and close/reopen regressions included |
| `cargo test --locked -p sgfmt` | 13 passed, including negative impl and generic trait preservation |
| `cargo test --locked -p sgc --bin sgc native_toolchain::tests::` | 20 passed; includes Windows MSVC discovery and verbatim/UNC path regressions |
| `cargo test --locked -p sgc --test native_cache` | Passed: cache reuse, same-length transitive header change, build and run relink |
| `cargo test --locked -p sgc --test shared_library` | 2 passed after unifying native link implementation |
| `sgc --runtime-mode source-development doctor --format json` | All native checks passed; temporary program compiled/linked/ran |
| Outside-checkout bin/lib `new → update → check/fmt/test/doc/build --locked` | Passed on Windows; binary `run --locked` passed |
| `openspec validate --all --strict` (1.15.0) | 51 passed: 44 canonical specs + 7 active changes |
| Python bindings/backend/wheel consumer | See [Python evidence](../tools/sengoo-python/EVIDENCE.md): typed bridge tests, clean offline install, two modules, 10,000 repeated calls |
| `python packages/test_native_packages.py --sanitize` | Three C package harnesses passed Windows AddressSanitizer; stderr retained in `target/package-native-tests/evidence.json` |
| Final sgpm unit and integration suites | 64 unit + 79 integration passed; partial-write/disconnect and immutable dependency regressions included |
| Real registry `fmt → test → doc/check --locked` | Passed with real sgc/sgfmt, unchanged lockfile and byte-identical registry. Test/format traversal stops at registry/git boundaries; checksum verification remains strict |
| Installed doctor text/native and JSON/python profiles | Successful backend profile and deliberately missing interpreter failure both verified in `target/doctor-profile-evidence.json`; failing JSON remains parseable and includes fixes |
| `scripts/package_ecosystem.py build` twice | Three package archives, registry bundle and manifest reproduced identical SHA-256 values in `target/ecosystem-dist` and `target/ecosystem-dist-repeat` |
| `scripts/ownership-sanitizer-gate.py` | Windows generated IR + seven split C runtimes passed ASan; real heap-overflow canary rejected, 19/19 resource releases and String baseline restored. Evidence: `target/ownership-sanitizer-gate/evidence.json` |
| `scripts/verify_daily_development.py` with the checksum-installed toolchain | Passed outside checkout: bin/lib new **and** init, full locked loops, library consumption, retained v0.1/v0.2 lockfiles without update, all three extracted official packages, persistent index/query and corruption rejection. Evidence: `target/daily-installed-evidence.json` |
| Delivered native application | Consumer source removed; PATH contained only delivered directory. Persistent Alice and Unicode queries passed, unknown option returned 2 |
| Final installed-compiler Python builds | Two external PEP 517 projects and both source archives rebuilt; fresh offline installation, isolated dual-module calls and full conversion checks passed. Final wheel hashes: `tools/sengoo-python/EVIDENCE.md` |

The composition tests found and fixed real defects beyond the original baseline:
Option fallback transfers, successive `?`, inactive Result cleanup, remaining
fields after user Drop, cross-function owning Vec fields and enumeration wrappers.
Unsupported owning cancellation/capture/partial-move shapes have documented stable
diagnostics. Final installed native integration passed on Windows x64. Local
first-round review artifacts were assembled in `target/daily-development-delivery-final`, including
SHA256SUMS, a source-file hash inventory, toolchain, registry bundle, backend,
two wheels, examples, documentation and actual local evidence.

The ownership sanitizer gate adds `sanitize_address` only to a test copy of
generated IR, runs Clang's LLVM instrumentation pass, verifies actual checked
memory operations and requires an overflowing canary to fail. On POSIX, the
seven C sources and probe also use UBSan with an independent canary. Raw LLVM
IR is not claimed to have C-frontend UBSan semantics. Windows LeakSanitizer is
not available; explicit native/String live counts check the exercised ownership
paths. Windows interceptor warnings are preserved for inspection.

## Candidate hardening, 2026-11-04

These Windows x64 checks were executed during development before candidate
freeze. They are not four-host or published-release results.

| Executed check | Result |
| --- | --- |
| Compiler library after collection ownership repair | 1132/1132; owning native compositions 16/16; ordered collections 12/12 |
| IEEE floating-point regression | Compiler 1133/1133 after NaN inequality fix; native numeric 3/3, including f32/f64 NaN, signed zero and infinity at O0/O2 |
| Generated ownership oracle self-tests | 9/9, including deterministic seed identities, boundary validation and retained failure artifacts |
| Actual generated-code ASan | Default seed 20261104 with four positive cases and three stable diagnostic rejections passed; alternate seed 0x5eed with eight cases also passed. Original probe created/dropped 19/19, String 0→0 |
| Incremental index outside the checkout, source and installed tools | Both passed 12 behavior checks and 54 commands using verified local-registry archives. Unchanged imports produced zero SQL audit writes and identical database bytes; malformed/oversize replacement and partial-insert SQL failures preserved prior rows and metadata |
| Official package native ASan | Three harnesses passed after UTF-8 text NUL rejection; the real Sengoo data test restored String/CSV/Buffer baselines |
| Candidate evidence validator | 10/10 tests. Missing targets, skipped/failed mandatory steps, foreign/expired/old-attempt artifacts and workflow identity errors reject. Independent read-only review replayed these failure cases and collection-time attempt changes |
| Distribution contracts | 21/21 passed using the same PowerShell 7 host as release jobs; the stable collector still rejects mixed-SHA and non-main-push gate evidence |
| Python real workload and bridge cleanup | 15 backend tests and six typed/native bridge tests passed. Two repaired wheels installed offline; Unicode CSV task processed 10,003 rows at three batch sizes with atomic late-error recovery. 20,000 iterations per module exercised 200,000 measured calls; String/opaque handles stayed at zero across ten checkpoints. Host-local batch cost reports retained; these are development-run results before candidate freeze |
| Workflow syntax | actionlint 1.7.12 passed; wheel repair/build reports and four-host/16-interpreter consumers remain required |
| Rust formatting and sgc Clippy | `cargo fmt --all --check` and `cargo clippy --locked -p sgc --bin sgc -- -D warnings` passed |
| OpenSpec | 52/52 strict checks passed after adding the candidate-hardening change |

The new cases exposed a returned HashMap/Rc destructor omission, NUL-containing
file text being silently truncated at C boundaries, and leaked Python input
String/Vec handles despite correct numeric results. Native counter observations
are required for the updated bridge. IEEE floating `!=` with NaN also received a
runtime regression after the real batch error checks exposed its ordered
predicate. See the Python evidence for the final bridge/resource run; old
10,000-call value assertions do not prove resource cleanup.

After freezing, rebuild and rerun the installed acceptance against the clean
committed candidate. `prepare_daily_delivery.py` refuses mismatched candidate
identities, changed wheel bytes or missing native-resource/cost reports.
Retained pre-fix failures are development diagnostics, not passing evidence.

## External gates

- [ ] Six required main workflows on the new candidate revision.
- [ ] Four-host installed new-project/package/wheel acceptance.
- [ ] Published candidate upgrade and checksum-verified rollback.
- [ ] 3-5 independent users, two weeks of actual pilot observations (**deferred by the owner**, not started).

No external gate is satisfied by writing a workflow or running an internal smoke.
