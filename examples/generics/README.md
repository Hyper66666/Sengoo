# Generics Examples

Run each file with `sgc run <path>`.

| File | Demonstrates | Expected output |
|---|---|---:|
| [`01_vec_i64.sg`](01_vec_i64.sg) | A `Vec<i64>`-shaped generic container and specialized i64 method | `60` |
| [`02_option_unwrap.sg`](02_option_unwrap.sg) | `Option<T>` and `unwrap_or` | `9` |
| [`03_result_chain.sg`](03_result_chain.sg) | `Result<i64, i64>` method chaining | `18` |
| [`04_stdlib_collections.sg`](04_stdlib_collections.sg) | Importing runtime-backed `std::collections` | `60` |
| [`05_bound_and_dyn.sg`](05_bound_and_dyn.sg) | A generic function with a trait bound plus a `dyn Trait` call | `29` |
