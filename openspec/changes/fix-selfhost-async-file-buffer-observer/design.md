# Restricted native buffer observation

The implementation adds `sh_b_async_file_read_contract` in
`selfhost/src/borrow.sg`, consulted by `sh_b_contract_observes_mode`. It requires
a function binding with the exact extern C marker, no SG body, a fixed function
signature, signed 64-bit parameters and return, exactly three arguments, and
position one. The existing general native-contract table is unchanged.
Neither the AsyncFile handle in position zero nor capacity in position two
receives a new ownership exception.

The inspected reference provider is
`runtime/src/async_runtime/async_file.rs::sengoo_async_file_read_into`. It creates
a local mutable byte slice, calls `File::read` while holding the file-registry
mutex, and returns before releasing that local borrow. It stores neither the
pointer nor slice in the registry or a queued task. This is a synchronous buffer
contract even though the file type also supports readiness futures.

The source bridge spells its pointer parameter as i64; the actual C ABI has a
mutable byte pointer in that position. The independent development readiness
provider gate executes the real read function with that physical signature,
checks five-byte reads and invalid pointer/capacity combinations, and is
separately pinned by this correction's formal evidence. The source bridge does
not change the provider's physical pointer type.

Verification must exercise the actual standard method after factory semantic
admission and reject wrong conventions, arities, widths, return types, unrelated
names and retaining wrappers. Ordinary Buffer move/escape checks remain active.
The native provider probe must preserve its exact C ABI pointer type and record
its development Rust origin; it is not a completed SG runtime port.
