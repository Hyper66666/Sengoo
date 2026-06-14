## 1. Vec<T>

- [ ] 1.1 Generic, owning `Vec<T>` over the runtime growable buffer; `push`,
  `pop`, `get` (`&T`), `set`, `len`, `is_empty`, `insert`, `remove`, `clear`.
  - Partial: existing scalar i64/bool wrappers remain, and this slice adds a
    runnable `Vec<String>` example over the existing owned-string runtime
    helpers (`push`, clone-on-`get`, transfer-on-`remove`). Full arbitrary
    `Vec<T>`, `insert`, type-driven `len/free` override resolution, and struct
    element support remain open.
- [ ] 1.2 `Drop` for `Vec<T>` drops each live element then frees the buffer.
- [ ] 1.3 `iter() -> impl Iterator<Item = &T>` and `into_iter() -> impl Iterator<Item = T>`.
- [ ] 1.4 Tests: `Vec<String>`, `Vec<struct>`, drop-of-elements leak check.
  - Partial: `examples/stdlib/25_generic_collections.sg` and sgc smoke tests
    now cover `Vec<String>` push/get/remove. Struct element and leak/drop checks
    remain open.

## 2. Hash and ordered maps/sets

- [ ] 2.1 `HashMap<K, V>` / `HashSet<T>` using `Hash` + `Eq`.
  - Partial: `HashMap<String, i64>` is now exposed as a thin string-key wrapper
    over the existing runtime string map so users can write the mainstream
    surface in examples. Fully trait-driven `HashMap<K, V>` and `HashSet<T>`
    remain open.
- [ ] 2.2 `BTreeMap<K, V>` / `BTreeSet<T>` using `Ord` (deterministic iteration).
- [ ] 2.3 `VecDeque<T>` double-ended queue.
- [ ] 2.4 Tests: string-keyed map with struct values; ordered iteration; drop of
  keys and values.

## 3. Iterator adapters

- [ ] 3.1 Implement `map`, `filter`, `fold`, `enumerate`, `take`, `skip`,
  `count`, `sum` over `Iterator`.
- [ ] 3.2 `collect` into `Vec<T>` and into maps/sets.
- [ ] 3.3 Tests for adapter chains and `collect`.

## 4. Migration and docs

- [ ] 4.1 Re-express the scalar helpers (`vec_new_i64`, `StringMapI64`, ...) as
  thin wrappers over the generic types, keeping their names source-compatible.
- [x] 4.2 Update `tools/stdlib/README.md` collections section.
- [x] 4.3 Add `examples/stdlib/` programs using `Vec<String>` and a
  `HashMap<String, i64>`.
- [x] 4.4 Run `openspec validate generic-collections --strict`.
  - Passed for this slice.

## Verification

- `cargo test -p sengoo-compiler --lib` and `cargo test -p sgc`
- New generic-collection examples compile, link, run, and leak-check clean
