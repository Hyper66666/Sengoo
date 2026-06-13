## Why

`std::collections` only ships scalar hand-specializations — `Vec<i64>`,
`Vec<bool>`, `StringMapI64`, `StringMapBool`, and copy-based `TextList` — because
the language lacked real generics. With `generics-and-trait-system` and
`automatic-memory-management` in place, the collections can become truly generic,
own their elements, and drop them automatically. This removes the biggest source
of stdlib duplication and is what lets users store arbitrary types (including
`String` and structs) in containers.

## Proposal

Provide generic, owning, auto-dropping collections and iterator adapters:

- `Vec<T>`: growable array; `push`/`pop`/`get`/`set`/`len`/`insert`/`remove`/
  `iter`/`into_iter`; owns `T`, drops all elements on drop.
- `HashMap<K, V>` and `HashSet<T>`: hashing via the `Hash` + `Eq` traits;
  `insert`/`get`/`remove`/`contains`/`len`/iteration.
- `BTreeMap<K, V>` / `BTreeSet<T>`: ordered via `Ord` for deterministic
  iteration.
- `VecDeque<T>`: double-ended queue.
- **Iterator adapters** over `Iterator`/`IntoIterator`: `map`, `filter`, `fold`,
  `enumerate`, `take`, `skip`, `collect`, `count`, `sum`.

Elements are moved in on insert, reads borrow (`&T`) or clone (`T: Clone`), and
removal moves the element out. All containers implement `Drop` and free their
elements.

## What changes

- ADDED: generic `Vec<T>`, `HashMap<K, V>`, `HashSet<T>`, `BTreeMap`/`BTreeSet`,
  `VecDeque<T>` with owning, auto-drop semantics.
- ADDED: iterator adapters and `collect`.
- MODIFIED (additive): existing scalar helpers (`vec_new_i64`, `StringMapI64`,
  ...) remain source-compatible during the transition.

## Non-goals

- Concurrent/lock-free collections (belongs to `concurrency-safety-and-async-io`).
- A full `std`-scale algorithm library; this delivers the core containers and
  the common adapters.
