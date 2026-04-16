# Non-scalar select design

Date: 2026-04-16

Goal: support `select(first, second)` for matching `Future<T>` where `T` is not limited to scalar bool/int/float types.

Chosen ABI:
- Add runtime function `sengoo_async_select_winner(i64, i64, i64, i64) -> i64`.
- Return `0` when the first future wins and `1` when the second future wins.
- Runtime does not extract the typed result.
- Compiler lowers `select(a, b)` into:
  - emit winner call
  - branch on winner
  - call `first_origin__result(first_handle)` in one branch
  - call `second_origin__result(second_handle)` in the other branch
  - merge branch results with `phi`

Why this shape:
- avoids per-type runtime ABI explosion
- works for scalar and aggregate results uniformly
- keeps ownership clear: runtime chooses the winner; compiler owns typed extraction
- does not require changing existing async result dispatch helpers used elsewhere

Compatibility:
- Existing scalar `sengoo_async_select_*` entrypoints can remain temporarily.
- New lowering path should prefer the generic winner ABI for all select calls.

Test plan:
- compiler tests for non-scalar `select` on tuple/struct futures
- runtime test for winner function behavior
- sgc native end-to-end test for non-scalar select
