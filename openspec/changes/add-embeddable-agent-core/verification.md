# sgagent core evidence

Host: DESKTOP-FUAQMFB, Windows x64, 2026-11-09. Experimental package 0.1.0,
edition 2026. Sengoo baseline `ea803f1913f9d08f6393d412b0eb4b1de3055b3e`;
the current tree contains SDK and parallel integration changes.

## Frozen inputs

- Core `packages/sgagent/src/lib.sg` SHA256
  `396c91ca2d94b4679d40cebda39aa3728b300c6c363b220162bdc85949870cee`.
- Compiler `target/agent-parameter-compiler/debug/sgc.exe`, SHA256
  `69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`.
- Formatter `target/agent-formatter/debug/sgfmt.exe`, SHA256
  `d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814`.
- Explicit Clang 19.1.7 Windows MSVC target and source-development runtime mode.
  The full report captures tool paths/hashes, all copied package inputs and
  native-runtime source hashes; it verifies the sets remained unchanged.

## Complete core and external-host run

`packages/sgagent/tests/run.py` passed all 26 recorded steps at
`packages/sgagent/target/acceptance.json`, SHA256
`6bcaed4a7f529d4bdb3517ad9f9affeb54ce01786cccb9cf894ac9b2776e1c7a`.

- Ten SG formatter checks, including the real SQLite source and its tests.
- Scheduler, tool schedule, request retry, kernel, budgets and guards each
  independently compiled and executed as native programs.
- Twenty repeated guard rounds check String/Buffer/opaque handle baselines;
  malformed control identities, policy shapes, widened restored grants,
  inconsistent cleanup/claim phases and invalid retry state are rejected.
- The kernel exercises committed admission, manual approvals, Plan, tool ID
  deduplication, explicit exact-usage budgets, unlimited default request count,
  five pollable network retries and provider-directed continuation.
- Unknown claims pause and preserve cleanup ownership. Stale or unverified host
  assertions fail; quiescence confirmation is committed, survives another
  snapshot/restart and does not resolve an unknown outcome. Separate verified
  result resolution commits without executing that tool again.
- A copied-package consumer outside the source checkout resolves a relative
  dependency graph, checks it locked and builds the actual maintenance host.
- The copied SDK sources and consumer are removed before the final programs
  run with System32-only PATH and no source-runtime overrides.

The maintenance executable SHA256 is
`0afd69bd3e26d22495698137b07717efd18aaf74993dbfc426389b0a929c284c`.
Its actual file read, exact patch, verification process and SQLite checkpoint
pass. One process stops after the committed patch; another resumes and verifies
without repeating it. Four cumulative requests and read/patch/test tool order
are preserved with a new fence. A second case kills the host after the patch
effect and before result commit: ordinary restart refuses before any provider
request, durable claim remains unknown and the patch count stays one.

The model peer is a synthetic loopback Responses server. File/process/SQLite
operations and process restarts are real; this is not live-provider evidence,
an arbitrary command sandbox or clean-machine distribution qualification.

After the complete run, sample stdout changed from per-event builtin `print`
to `io_stdout_write`/`io_stdout_flush` on nonempty deltas. The core hash did not
change. A separate 14-step targeted run at
`packages/sgagent/target/exact-maintenance-69a-acceptance.json`, SHA256
`bb39e1d17c850bb9fccf514ae201ee97729ee075c4af83d4939d020257296c16`,
passes exact first-process/continuation/crash stdout and repeats both real
maintenance lifecycle cases. Its executable is
`e1768201df79bfc0179ed78db946ee3d5d8289673bcef3eaf053800c8fddbe5c`.
The original 26-step report remains separately preserved as
`target/core-69a-full-acceptance.json`; the targeted test is not presented as a
new full native-suite run.

## Real ledger gate

The independent SQLite owner passed
`target/agent-sqlite-ledger/final/evidence.json` using the same compiler/core.
Its native executable SHA256 is
`cb6d6f659cd070fd719c2b7f358b48fd5cc9d66c949a6ee55ce1ddbde216061a`.
Nine records include transaction/fencing/lock ownership, corruption rejection,
strong crash recovery, host-only resolution and twenty-round handle/resource
baselines. Full storage contract and owned test driver belong to
`openspec/changes/add-agent-sqlite-ledger/` and
`packages/sgagent/tests/verify_sqlite.py`.

## Preserved failures and remaining gates

The first full run reached all five earlier native suites but guard testing
returned capacity status13. Its report remains at
`target/capacity-guard-failed-acceptance.json`. The fixture retained too many
4MiB checkpoint documents across recovery stages. The diagnostic is preserved
at `target/guards-capacity-diagnostic.sg`; splitting the fixture stages into
short helper scopes retained all assertions and passed without increasing a
runtime bound or changing the SDK core.

The earlier String +5 loop-result leak was kept visible until a generic compiler
repair; the final twenty-round guard above uses that repaired compiler and no
manual-free workaround. Later compiler candidates must receive their own
relevant resource/output-path checks; these results do not certify another
binary by inference. Root integration still owns the CLI candidate, its existing
Plan/Goal/permissions/cancel/recovery gates, packaging and publication. This core
change does not mark the complete ecosystem roadmap done or claim non-Windows
platform, sanitizer or real-provider testing.
