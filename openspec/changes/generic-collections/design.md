## Context

The current stdlib exposes useful scalar transition wrappers, but arbitrary
`T`, `K`, and `V` require a runtime representation that preserves alignment,
move semantics, callback dispatch, borrowed references, and exact Drop. Adding
more wrappers before freezing that representation would deepen the migration
cost.

## Decisions

### Decision 1: Typed public wrappers over type-erased runtime storage

Public APIs are monomorphized Sengoo types such as `Vec<T>` and
`HashMap<K,V>`. Runtime storage is type-erased and receives compiler-generated
descriptors/callbacks. Raw pointers and descriptors are not public API.

An element descriptor contains at least:

- byte size and alignment;
- move/copy operation where a byte move is not sufficient;
- exact-once drop callback;
- optional clone callback for clone-returning APIs;
- hash/equality/order callbacks where the container requires them.

### Decision 2: Collection operations have explicit ownership semantics

- insert/push moves ownership into the collection;
- `get`/iteration borrows and cannot outlive the collection;
- mutation that can reallocate invalidates outstanding element borrows and is
  rejected while such borrows are live;
- remove/pop moves ownership back to the caller;
- clear/drop releases every still-live element exactly once.

### Decision 3: Growth and callback failure preserve invariants

Allocation failure or callback failure must leave the original collection
valid. Partially moved/copied elements are tracked so cleanup neither leaks nor
double drops. Capacity arithmetic is overflow-checked.

### Decision 4: Iterator families are concrete monomorphized state machines

`iter` yields borrowed items; `into_iter` owns the collection and yields moved
items. Adapter types are monomorphized and preserve size hints where known.
`collect` is trait-driven but first closes `Vec`, map, and set targets required
by the spec.

Generic adapters must resolve `Iterator::Item` as an associated projection in
type checking and MIR specialization. They must not be approximated by eager
generic methods that recursively materialize `Vec<T>` operations: that shape
has been measured to enter non-terminating monomorphization on a basic
`map/filter/fold/collect` chain. Each adapter therefore gets a concrete lazy
state-machine type after associated-item projection is available.

### Decision 5: Scalar APIs become thin compatibility wrappers

Existing scalar constructors and named map/list types keep source compatibility
but route through the generic core after parity tests pass. They do not maintain
independent storage implementations indefinitely.

## Internal ABI sketch

The exact C/Rust names are implementation details, but the contract is
equivalent to:

```text
TypeDescriptor { size, align, move_fn, drop_fn, clone_fn? }
HashDescriptor { type, hash_fn, eq_fn }
OrderDescriptor { type, cmp_fn }
RawVec { data, len, cap, element_descriptor }
```

Map entries carry independent key and value descriptors. Compiler-generated
callbacks use the same runtime ABI version as normal Drop/dyn dispatch helpers.

## Test strategy

- arbitrary struct and owned String storage;
- over-aligned element layout;
- growth/reallocation and borrow invalidation;
- replacement/removal/clear/drop exact counts;
- map key/value callbacks and collision behavior;
- iterator early drop, partial consumption, and into-iteration;
- allocation/capacity failure invariants;
- legacy-wrapper differential tests.

## Archive gate

No task is completed by adding another scalar specialization. Archive requires
the real generic core, arbitrary user-defined values, exact Drop evidence, and
legacy wrappers proven as thin adapters.
