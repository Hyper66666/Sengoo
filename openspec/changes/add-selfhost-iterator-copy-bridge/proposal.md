# Checked scalar iterator copy bridge

The actual core entry passes SG typing but retains ownership diagnostics in
collection iterators. The legacy scalar wrappers store the borrowed result of
`raw_vec_iter_next<T>` and then dereference-copy i64 or bool. That operation is
implemented by a Rust frontend intrinsic; its source body and the SG checked
facts do not currently express the provider, returned borrow or storage origin.
Granting a wrapper-name exception or treating an arbitrary eight-byte slot as
Copy would hide this missing contract.

This child owns a narrow checked bridge for the existing scalar specializations,
the source/runtime contract, adversarial qualification and integration ordering.
Native SDK owns caller-pinned provider admission. M3 owns checked facts, ownership,
HIR and lowering. Runtime algorithm migration remains separately owned. The
initial work is a reviewable design and executable target-only proof; production
runtime/collections/borrow sources remain unchanged until coordinated review.

Preserve the public legacy raw wrappers. Prefer new internal wrappers with
explicit `&VecIter<i64>` or `&VecIter<bool>` input if they can use existing
borrowing and source summary rules without changing published semantics. Their
physical helper retains a fixed C `(i64, i64) -> i64` signature. Admit that helper
only through a pinned provider and actual source/declaration binding, and attach
a checked call contract proving the precise receiver projection. A separate
borrowed Option intrinsic route remains available if the typed wrapper route
cannot preserve existing behavior; it must carry explicit pointer/Option and
backing-owner facts rather than reuse source spelling.

The earlier target discovery measured 226 baseline diagnostics, 119 with done
contracts, 165 with scalar helpers alone and 34 together. These counts are
development discovery, not completion of core, M3, the SDK or self-hosting.

The strict target is a partial proof. Its canonical-only source rewrite leaves
75 diagnostics and admits a forged canonical iterator reconstructed from an
owning-element handle. That required rejection remains open. A separate
clear-call-table control distinguishes the rewrite's ordinary reference effects
from provider-driven ownership authority; table validation alone does not
complete the latter integration.
