## 0. Generic storage contract

- [x] 0.1 Freeze type descriptors, size/alignment, move/drop/hash/eq/order
  callbacks, borrow invalidation, failure invariants, and wrapper migration in
  `design.md`.
- [x] 0.2 Add runtime ABI versioning and compiler-generated callback tests before
  exposing the generic constructors.
  - `SENGOO_COLLECTIONS_ABI_VERSION == 1` freezes the internal descriptor
    layout and callback signatures in `runtime_shared.h`; native sgc coverage
    proves the split runtime exports that version. Compiler IR coverage proves
    an erased callback is synthesized and passed across the C ABI for an owned
    user struct, and native Rc coverage proves the callback drops owned fields
    exactly once after the last release.

## 1. Vec<T>

- [x] 1.1 Generic, owning `Vec<T>` over the runtime growable buffer; `push`,
  `pop`, `get` (`&T`), `set`, `len`, `is_empty`, `insert`, `remove`, `clear`.
  - Transitional `Vec<i64>`, `Vec<bool>`, and `Vec<String>` expose
    concrete mutators. `Vec<String>` now routes `len`, `is_empty`, `clear`,
    `free`, `set`, and `insert` through the string-vector runtime rather than
    the scalar i64 vector compatibility path. `Vec<i64>` and `Vec<bool>` now
    also expose `insert` through the scalar vector runtime. The concrete Vec
    method families use borrowed receivers, so calls cannot consume and
    prematurely release the owning handle.
  - The ABI-v1 type-erased `RawVec` core implements aligned growth, push,
    borrowed slot lookup, set, insert, pop, remove, clear, and free with
    descriptor-driven move/drop callbacks. Public `Vec<T>` lowering carries
    concrete size/alignment/move/drop callbacks across that ABI.
  - `vec_new<T>()` now infers `T` from the expected `Vec<T>` return type,
    preserves phantom generic identity through typeck/HIR/MIR, synthesizes
    per-element move/drop callbacks, and creates an ABI-v1 RawVec on the native
    path. Function-address references are reachability edges, so callback
    thunks survive MIR pruning.
  - Arbitrary concrete `T` now supports descriptor-driven `push`, `set`,
    `insert`, `len`, `is_empty`, `clear`, explicit `free`, and synthesized owner
    Drop. Native `Vec<Payload<String>>` coverage proves replacement drops the
    previous element, insertion transfers ownership, rejected consuming writes
    drop their incoming value, and clear returns owned String handles to the
    baseline. Generic `get -> &T`, pop, and remove use the same runtime, and a
    live element borrow rejects moving/reallocating mutations until its lexical
    scope ends.
- [x] 1.2 `Drop` for `Vec<T>` drops each live element then frees the buffer.
  - `vec_new<T>()` synthesizes a concrete RawVec owner-drop helper when the
    instance has no existing compatibility Drop; the ABI probe checks exact
    multi-element drop counts after growth/remove/pop/clear/free, and native
    `Vec<Payload<String>>` coverage proves scope exit returns the String
    live-handle count to its baseline after ownership was moved into the Vec.
- [x] 1.3 `iter() -> impl Iterator<Item = &T>` and `into_iter() -> impl Iterator<Item = T>`.
  - `RawVecIter<T>` owns an ABI-v1 runtime cursor, implements
    `Iterator<&T>`, and yields borrowed slots while holding a lexical borrow of
    the Vec. `RawVecIntoIter<T>` owns the Vec, implements `Iterator<T>`, moves
    elements out through pop, and drops any unconsumed remainder.
- [x] 1.4 Tests: `Vec<String>`, `Vec<struct>`, drop-of-elements leak check.
  - Compiler and native `sgc` runtime tests cover `Vec<String>`, bool/i64
    transitional mutators, and arbitrary `Vec<Payload<String>>`
    push/set/insert/get/remove/pop/clear with live-handle leak checks.
  - Native partial-iteration coverage consumes one `Payload<String>` and proves
    dropping the owning iterator releases every unconsumed payload.
  - Runtime ABI coverage now exercises a 64-byte-aligned element through two
    reallocations and checks exact move/drop counts across insert, replacement,
    remove, pop, clear, and free. User-level struct coverage proves exact owned
    String release, and a compiler negative test rejects mutation while an
    element borrow is live.

## 2. Hash and ordered maps/sets

- [x] 2.1 `HashMap<K, V>` / `HashSet<T>` using `Hash` + `Eq`.
  - `HashSet<i64>`, `HashSet<bool>`, and `HashSet<String>` remain
    as transitional wrappers over the existing i64 hashmap and copied
    string-key map runtimes, with borrowed methods and automatic scope-exit
    cleanup. `HashMap<String, bool>` and
    `HashMap<String, String>` now exist as transitional copied-key wrappers
    alongside `HashMap<String, i64>`.
  - `hashmap_new<K: Hash + Eq,V>()` and `hashset_new<T: Hash + Eq>()` now use an
    ABI-v1 type-erased map core with independent key/value size, alignment,
    move, and Drop callbacks plus compiler-generated Hash/Eq thunks. Insert
    consumes ownership, duplicate keys replace exactly once, get borrows,
    remove moves values out, and scope Drop releases remaining keys/values.
- [x] 2.2 `BTreeMap<K, V>` / `BTreeSet<T>` using `Ord` (deterministic iteration).
  - `BTreeMap<String, i64>`, `BTreeMap<String, bool>`,
    `BTreeMap<String, String>`, and `BTreeSet<String>` now expose an ordered
    transition surface over the existing copied-key runtime.
    `BTreeMap<i64, i64>`, `BTreeMap<i64, bool>`, and `BTreeSet<i64>` now use an
    independent sorted integer runtime rather than exposing hash-map slot
    order. Inserts use binary-search placement, replacement preserves one key,
    and key iteration is deterministic ascending numeric order (or unsigned
    UTF-8 byte order for strings) regardless of insertion order. Both integer
    maps and the integer set cover lookup, removal, len/clear, and automatic
    `Drop`.
  - `btreemap_new<K: Ord,V>()` and `btreeset_new<T: Ord>()` use the generic
    storage core with compiler-generated compare thunks. Insert positions follow
    Ord, duplicate keys replace, borrowed key cursors iterate deterministically,
    and generated Drop handles arbitrary owned keys and values.
- [x] 2.3 `VecDeque<T>` double-ended queue.
  - Transitional `VecDeque<i64>` and `VecDeque<bool>` expose
    `push_front`, `push_back`, `pop_front`, `pop_back`, `front`, `back`, `len`,
    `clear`, manual `free()` compatibility, and automatic scope-exit `Drop`
    over the existing i64 vector runtime.
  - `vecdeque_new<T>()` now shares the ABI-v1 RawVec descriptor path for
    arbitrary concrete `T`; front/back borrow slots, push/pop at both ends move
    ownership correctly, and generated owner Drop releases remaining elements.
    Native `VecDeque<Payload<String>>` coverage proves a moved-out front value
    and the unconsumed remainder are each dropped exactly once.
- [x] 2.4 Tests: string-keyed map with struct values; ordered iteration; drop of
  keys and values.
  - Partial: compiler and native `sgc` tests cover `HashMap<String, i64>` and
    `HashMap<String, bool>` / `HashMap<String, String>` insertion,
    replacement, lookup, removal, and deterministic key iteration through the
    copied-key runtime. The stdlib example now inserts ordered-map/set string
    keys out of order and observes byte-sorted iteration through owned key
    vectors. Dedicated compiler-surface and `tools/sgc/tests/ordered_collections.rs`
    native tests cover out-of-order signed integer keys, replacement, lookup,
    removal, clear, automatic owner cleanup, and deterministic ascending key
    iteration for `BTreeMap<i64, i64/bool>` and `BTreeSet<i64>`. Receiver/Drop
    regressions also cover scalar and string HashSet handles. Struct values and
    generic key/value element-drop coverage now run natively: a derived
    struct-keyed `HashMap` owns String payloads through replacement/remove, and
    a custom Hash/Eq key containing an owned String proves key replacement,
    removal, and remaining-key Drop. A custom Ord key containing an owned String
    is inserted out of order into generic BTreeMap/BTreeSet; native coverage
    proves ascending borrowed iteration, duplicate replacement, moved-out
    values, and exact Drop of all remaining keys and values.

## 3. Iterator adapters

- [x] 3.1 Implement `map`, `filter`, `fold`, `enumerate`, `take`, `skip`,
  `count`, `sum` over `Iterator`.
  - Partial: transitional iterator helpers now cover single-step `map_with`
    and `filter_with`, plus consuming `count`, `sum`, and `fold_with` for the
    existing runtime-backed i64 iterators. Bool iterators now cover `map_with`,
    `filter_with`, `fold_with`, `count`, and consuming `skip`/`take`; i64
    iterators also cover consuming `skip`/`take`. `VecStringIter` now covers
    consuming `count`, `skip`, and `take`. `VecIter<i64>` and `VecIter<bool>`
    now expose transitional `enumerate()` adapters backed by the runtime
    iterator cursor; `HashMapIter<i64>` and `HashMapIter<bool>` expose matching
    value enumeration backed by a yielded-item cursor. `HashSet<i64>` and
    `HashSet<bool>` now expose key iterators with consuming `count`, `skip`,
    `take`, `collect`, and `enumerate()` adapters backed by an i64 hashmap key
    cursor, and string-key maps/sets expose copied/owned key iterator helpers
    with `count`, `skip`, `take`, `collect() -> Vec<String>`, and
    surface-level `enumerate()` coverage. Fully generic adapter chains,
    generic `fold`, and lazy adapter types remain open. A generic eager adapter
    prototype was rejected after it reproducibly caused non-terminating
    monomorphization; the remaining implementation must first make associated
    `Iterator::Item` projections usable during generic MIR specialization and
    then lower concrete lazy adapter state machines.
  - The first normalization prerequisite is now in place: HIR preserves
    structured associated-type projections and impl `type Item = ...`
    bindings through lowering and generic substitution. Type checking now
    recursively resolves nested projections such as `Option<I::Item>`, MIR
    substitution keys include the declaring trait to avoid same-name
    collisions, and trait impls reject concrete associated-result signature
    mismatches. `option_none<T>()` infers its concrete payload from the
    expected `Option<T>` and lowers directly to a tagged aggregate, removing
    the placeholder argument needed by lazy adapters. A first concrete lazy
    `TakeIter<I,T>` now owns its source iterator and an RAII-managed runtime
    counter; `RawVecIntoIter<T>.take()` specializes and stops polling after the
    requested count for arbitrary user structs. Concrete `Option<T>` values now
    use synthesized tag-guarded Drop helpers, so an exhausted iterator's
    compiler-generated `None<T>` never drops its inactive placeholder while
    `Some<T>` and nested `Option<Option<T>>` delegate exactly once. Registered-HIR
    back-binding now recovers trait-qualified associated projections before
    specialization keys are formed, so `RawVecIntoIter<Payload>.skip(1).take(1)`
    terminates and emits concrete nested `SkipIter`/`TakeIter` state machines.
    Concrete named function values can also initialize function-typed struct
    fields. Function-signature substitution now binds generic parameter and
    return types, and explicitly typed lambdas are checked/lowered against
    their declared callable signature. Generic lazy `MapIter<I,T,O>` and
    `EnumerateIter<I,T>` state machines specialize for arbitrary user structs;
    compiler coverage exercises `skip -> take -> map` and indexed enumeration.
    Generic `FilterIter<I,T>` now borrows each candidate for its predicate,
    skips rejected items lazily, and preserves owning payloads without copying.
    Compiler and native coverage exercise `skip -> take -> map -> filter`, and
    an owned `Payload<String>` leak-counter test proves accepted, rejected, and
    unconsumed values are each released exactly once. All six owning adapter
    families now expose consuming `count()` and accumulator-generic
    `fold<A>(A, fn(A, Item) -> A)`; compiler and native tests cover both through
    lazy chains. Generic `sum()` is now item-preserving and available through
    the numeric `SumValue` contract; empty iterators use the compiler-provided
    numeric identity and chained adapters combine each yielded item once.
    Adapter-building `map` methods and terminal methods specialize on demand,
    preventing recursive eager type discovery while allowing `filter -> map`
    and repeated-map chains. Every lazy adapter family keeps the applicable
    `map`, `filter`, `take`, `skip`, and `enumerate` builders, and concrete
    return HIR is registered at materialization time so the next chained call
    can specialize without eager recursive type expansion.
- [x] 3.2 `collect` into `Vec<T>` and into maps/sets.
  - Partial: terminal generic `collect() -> Vec<T>` now materializes
    `RawVecIntoIter`, `TakeIter`, `SkipIter`, `MapIter`, `FilterIter`, and
    `EnumerateIter` through the ABI-v1 RawVec path. Terminal collect methods are
    excluded from eager impl-type rediscovery and are specialized when called,
    preventing recursive `Vec -> into_iter -> collect` type growth. Native
    `Payload<String>` coverage proves collected ownership transfers without a
    leak or double drop. Transitional consuming `collect()` also materializes the existing
    runtime-backed `VecIter<i64>`, `VecIter<bool>`, `HashMapIter<i64>`, and
    `HashMapIter<bool>` into `Vec<i64>` / `Vec<bool>`. `VecStringIter.collect()`
    now clones the remaining iterator items into a new `Vec<String>` through a
    runtime bridge, and string-key map/set key iterators can collect owned key
    copies into `Vec<String>`, so the transition surface covers owned strings
    without borrowing handles from `Result<String>`. Explicit generic
    `collect_hashset()` and `collect_hashmap(projector)` sinks now materialize
    the ABI-v1 map core. The projector returns `MapEntry<K,V>`, so K/V are
    inferred from a callback argument rather than unsupported return-type-only
    `collect<C>()` inference.
  - Chained method result identity is now keyed by the complete source span,
    fixing a regression where `.iter().collect()` reused the inner iterator
    type as the outer call's expected return type and hid the correct trait
    method during MIR dispatch.
- [x] 3.3 Tests for adapter chains and `collect`.
  - Partial: compiler surface tests cover i64/bool Vec and HashMap
    `enumerate()` lowering, HashSet key enumeration, and string-key iterator
    enumeration lowering. `examples/stdlib/10_collections.sg` now runs i64/bool
    Vec and HashMap enumeration, i64/bool HashSet key enumeration and bounded
    `take`, and string-key iterator counting/take/skip/collect through `sgc run`, including the
    `HashMap<String, String>` key iterator transition surface. It also runs
    the transitional `VecDeque<i64>` / `VecDeque<bool>` push/front/back/pop
    path and `Vec<String>` set/insert/remove plus cloned iterator collection.
  - Generic compiler-surface coverage now exercises lazy `skip -> take -> map`
    over an owned user struct, verifies a generic `enumerate` state machine
    yields stable zero-based indices, and materializes a `map -> filter ->
    collect` chain. Native owned-String coverage proves filter and collect keep
    exact Drop balance. Generic count/fold execute natively over RawVec and
    filtered adapters. Native mixed-chain coverage now executes
    `skip -> take -> sum` and `filter -> map -> collect_hashset`, and explicit
    map collection uses a user-defined Hash/Eq key. Additional compiler/native
    coverage executes `map -> take -> skip -> count` and
    `filter -> skip -> take -> enumerate`; a negative compiler test rejects
    `sum` for non-`SumValue` items.

## 4. Migration and docs

- [ ] 4.1 Re-express the scalar helpers (`vec_new_i64`, `StringMapI64`, ...) as
  thin wrappers over the generic types, keeping their names source-compatible.
  - Partial: `Vec<String>` and `HashMap<String, i64>` concrete methods now use
    their string-backed runtimes for lifecycle, length, and core mutator
    operations, and `HashMap<String, String>` now wraps the existing
    `StringMapString` runtime while `HashMap<String, bool>` wraps
    `StringMapBool`. This reduces the transitional gap where generic-looking
    handles accidentally fell back to i64 runtime helpers. The full legacy
    scalar helper migration remains open.
- [x] 4.2 Update `tools/stdlib/README.md` collections section.
  - Documented the current transition surface: `Vec<String>`,
    including set/insert, `VecDeque<i64>`, `HashMap<String, i64>` /
    `HashMap<String, bool>` / `HashMap<String, String>` over copied-key string
    maps, legacy `StringMap*` compatibility, and the remaining fully generic
    collection gaps.
- [x] 4.3 Add `examples/stdlib/` programs using `Vec<String>` and a
  `HashMap<String, i64>`.
  - Extended `examples/stdlib/10_collections.sg` and its README row to cover
    `Vec<String>` mutators and the `HashMap<String, i64>` /
    `HashMap<String, bool>` / `HashMap<String, String>` transition spellings.
- [x] 4.4 Run `openspec validate generic-collections --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` and `cargo test -p sgc`
- New generic-collection examples compile, link, run, and leak-check clean
