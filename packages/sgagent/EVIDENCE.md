# sgagent 0.1.0 executable evidence

Windows x64, DESKTOP-FUAQMFB, 2026-11-09. Experimental native package. All
model traffic below uses loopback fixtures; file/process/SQLite operations are
real. Public-provider, clean-machine, sanitizer and other-platform gates are
separate.

The core source SHA256 is
`396c91ca2d94b4679d40cebda39aa3728b300c6c363b220162bdc85949870cee`.
The compiler used here is `target/agent-parameter-compiler/debug/sgc.exe`, SHA256
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`;
the formatter is `target/agent-formatter/debug/sgfmt.exe`, SHA256
`d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
Reports capture Clang/sgpm identity and frozen package/runtime inputs.

| Report | Actual result |
| --- | --- |
| `target/core-69a-full-acceptance.json` | 26 steps: ten format checks; six native suites; locked outside-checkout host; removed copied SDK sources before runtime; normal restart and strong crash cases |
| `target/exact-maintenance-69a-acceptance.json` | 14 targeted steps after replacing per-event builtin printing with public raw stdout; exact stdout, read/patch/process/SQLite/resume and crash assertions all pass |
| `../../target/agent-sqlite-ledger/final/evidence.json` | Independent ledger owner: nine real native/DB/multiprocess records, including twenty-round resource counters, unknown recovery and fencing |

The 26-step report SHA256 is
`6bcaed4a7f529d4bdb3517ad9f9affeb54ce01786cccb9cf894ac9b2776e1c7a`.
Its maintenance binary is
`0afd69bd3e26d22495698137b07717efd18aaf74993dbfc426389b0a929c284c`.
The later 14-step report SHA256 is
`bb39e1d17c850bb9fccf514ae201ee97729ee075c4af83d4939d020257296c16`;
its precisely streamed maintenance binary is
`e1768201df79bfc0179ed78db946ee3d5d8289673bcef3eaf053800c8fddbe5c`.
The core library is identical in both; only sample stdout and its stricter
fixture/runner selection changed.

Six native suites cover compatible scheduler/tool-schedule/retry policy, full
kernel dispatch, explicit budgets, unlimited default requests, Plan and dynamic
schema/permission checks. Guards repeat twenty times and return String, Buffer
and opaque handles to baseline. Malformed/NUL identities, widened restored
permissions, contradictory cleanup phases and unverified host outcome claims
are rejected. A trusted host's cleanup and verified outcome resolution are
distinct committed facts and never re-execute the unknown tool.

The maintenance application performs one exact patch, exits and restarts to
run a real verification command. Four model requests, cumulative token counts,
new fencing and all three known tool results survive. Killing after the patch
effect but before result commit leaves a claimed unknown result; ordinary
restart refuses before a provider request and the patch count remains one.
Runtime uses System32-only PATH with source-runtime variables removed.

The initial capacity-failed report is preserved separately. Excess long-lived
4MiB documents in one test scope were shortened with helper scopes; no bound
was raised. The prior ignored-owning String leak was fixed in the compiler;
the package does not manually release borrowed handles to suppress it.

Run `tests/run.py` with explicit compiler/clang/formatter/sgpm paths. Its default
builds all six native suites; `--tests guards` restricts the repeated native
set while retaining the external maintenance flow, and an empty `--tests` runs
only the ten format and maintenance-host gates. The ledger's separate
`tests/verify_sqlite.py` owns its adversarial/multiprocess proof. Any later
compiler candidate needs its own relevant resource/output evidence; these
reports cannot certify a different binary or the complete ecosystem roadmap.
