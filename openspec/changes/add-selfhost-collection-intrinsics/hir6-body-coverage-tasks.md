# HIR6 complete standard graph focused checkpoint

- [x] Classify every actual complete-library call predicate failure and separate unvisited trait defaults from checked mapper signatures.
- [x] Derive strict unique AST/binding-based deferred coverage without changing HIR6 wire, bounds or source/ownership witnesses.
- [x] Compare substituted callable structure read-only with explicit work/depth limits and exact ABI/flags/nominal/argument identity.
- [x] Check ordinary-call, deferred metadata, owner/AST sharing, type/signature, provenance, ownership and wire5 rejection controls.
- [x] Build and roundtrip the actual complete standard graph with fresh scoped ownership and independent source/provider pins.
- [x] Run first-call resource scopes, readonly arena checks, strict specs, identity-negative control and exact source/tool qualification.

Executable default-body checking, generic jobs, full library ownership/NLL and
safe optional runtime borrow authority remain later gates.

## Qualified focused evidence

Final report: target/selfhost/m3-hir-body-coverage/checkpoint/evidence.json,
SHA256 050b69b69dbe2c7a0736023eab91803cce37e7246c2f9efaf9bdddd2f467fbca.
Actual revision 2d3d2b87c34f6d13799a49faf7b83ee03d9178fa at both start/end;
Windows11 10.0.26300-SP0/Python3.12.14. Actual UTC
2026-11-10T02:31:25.856258+00:00 through
2026-11-10T02:35:15.615297+00:00. Six commands pass: pinned seed/clang/actual
rustc/cargo versions, native SG test build and complete-library runtime test.
All607 input hashes,139 producer origins,141 immutable snapshots/artifacts,
four tools and external Cargo config candidates remain unchanged. The producer
checkout change list is empty too. Actual runtime uses only captured SG/stdlib
inputs and independently qualified provider manifest/object/source pins.

Three scopes record String/Buffer/opaque/FileSnapshot handle and byte baselines
before the first call; each returns exactly with no warmup. Observed compiler
peak working set216530944 bytes, analysis210083840 bytes; this is not M8 or
performance qualification. Each scope runs mini and actual full standard-module
HIR6 lift, exact actual module+ownership validation, encode/decode and deep clone
within the unchanged four-million-word budget. Original typed words/arguments
and all serialized AST, checked and ownership data remain exact.

The complete actual graph has10 default bodies with4 unvisited calls. Every
node in those body subtrees retains the semantic producer's four -1 metadata
sentinels and has unique incoming syntax membership. The focused source with an
otherwise unresolved call in a genuine trait body is accepted only as deferred
syntax; function-body state2 remains non-executable. Ordinary function calls with
missing callable facts reject5503. Altered deferred callable/type metadata,
wrong owner/trait membership and attaching one default method to an additional
root parent reject; restored state validates again. No nominal name or source
span assigns coverage.

The2 mapper(item.value) call signatures retain distinct raw interned Fn IDs but
have equal recursively resolved structure. Flipping their valid ABI data or
replacing a return argument with bool rejects5503; validation does not append
types or mutate substitutions. The depth/work cap remains128/100000 per
comparison. Actual ownership scalar changes fail5509, module witness mutation
rejects, old wire5 rejects5502, and all22 ownership fields plus old builtin6 /
deferred4 / action8,9,10 tables retain their existing serialized meanings.

The stale-pass wrong-seed control in
 target/selfhost/m3-hir-body-coverage/identity-negative/control.json
runs no tool steps, exits1 and replaces passed with failed. Failed report SHA
249529e32939f65c32ef8fedea875e044c79f7ee705913c8ef763fb9c1b8ca3a.
The first formal attempt is preserved at failed-immutable-test-binding: the new
ownership-scalar poison required a mutable test HIR binding; it is not a product
pass or discarded failure. The final source-captured run executes that control.
Earlier development also preserved the S0 inferred-zero-Vec error; the product
uses an explicitly typed zero owner for Err rather than changing the seed.

HIR source SHA256
90569cf13c0a948fa343c038b695bf718ff9b0421e157e2f4b3c312278e2c70b.
The qualified collection ownership source stays6bc62103... and oldborrow stays
990fb053...; no old semantic, source-origin/loan/NLL, HIR origin codec or
MIR/native source is changed by this focused phase. This ledger is excluded from
input guards to avoid report self-reference; design/spec remain exact guarded
inputs. Child/all OpenSpec strict108 passed/0 failed, formatter and diff check pass.

Body-state0 denotes absent usable block-body coverage and grants no external
implementation certificate; external consumers still need actual ABI/declaration
admission. Current codegen entry-function bodies require state1. State2 defaults,
uncovered closures, concrete generic jobs, complete library ownership/NLL and
provider/constructor/MIR27 runtime authority remain independently gated.
