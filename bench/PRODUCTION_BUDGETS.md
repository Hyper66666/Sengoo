# Production performance budgets

These gates are release engineering budgets, not cross-language marketing
claims. Raw samples and host metadata are uploaded by `perf-smoke` on every run,
and the frozen advanced-pipeline baseline report is retained in-repo.

## Compile scenarios

`bench/advanced_pipeline_bench.py` deterministically generates and measures
small (1k), medium (10k), large (100k), and release-scale (1M) programs plus
full incremental loop-body, signature-change, and new-function edits. These
names and line-count buckets are the frozen production scenarios. The blocking
hard gate uses Actions run `29327347740` as its frozen Windows reference,
enforces 30% regression limits
to account for shared-runner variance, and applies these absolute ceilings:

- real incremental edit: 200 ms;
- 100k full build: 2,000 ms;
- 1M full build: 7,000 ms;
- 100k frontend or reachability frontend: 750 ms;
- 1M frontend: 2,500 ms;
- 100k peak RSS: 300 MiB;
- 1M peak RSS: 1,800 MiB.

The C++ RSS ratios, frontend-share targets, and 2.5M ladder remain trend-only
comparisons and cannot make a release claim pass.

## Release resource scenario

`bench/scenarios/release-resource/runtime_loop.sg` is the fixed artifact,
startup, CLI, and runtime workload. `release_resource_gate.py` enforces:

- release `sgc` artifact: 20 MiB;
- generated program artifact: 5 MiB;
- average `sgc --version` startup: 250 ms;
- average `sgc check` on the fixture: 750 ms;
- median of repeated forced optimized builds: 5,000 ms;
- average one-million-iteration generated program run: 250 ms.

A threshold change must update this file and the gate defaults in the same
review, attach the previous and candidate raw JSON reports, and explain the
regression or reference-host change.
