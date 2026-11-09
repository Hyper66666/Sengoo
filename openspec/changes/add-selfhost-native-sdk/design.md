# Native SDK identity and declarations

Use a versioned manifest with a native bridge family, target triple, pointer
width, ABI version, component class and artifact/content hashes. Keep native
handle-based ABI 1 separate from portable logical-layout schema 1. Each export
records its real symbol, calling convention, physical parameter/result ABI,
source revision/path/hash and provider component. A caller supplies the pinned
manifest hash and expected target/ABI; a manifest does not authorize itself.

Declarations are generated or checked against the inventory. Missing,
conflicting or changed providers fail instead of becoming guessed bindings.
Source snapshots and provider artifacts must remain unchanged through admission.
Language signatures and physical C signatures stay distinct. String views
retain receiver layout and lifetime facts in addition to an i64 pointer-bits
result. Future families enumerate start/poll/result/cancel/drop prototypes,
handle origin, language outcome and lifecycle obligations. M3 validates HIR/MIR
bridge facts; SDK data alone does not authorize an integer-to-reference cast.

Reference-runtime components remain development-only. Final components are
reviewed OS/memory shims and permitted precompiled libraries with provenance.
Runtime algorithms migrate to SG under separate owners. Packaging must not
hide non-migrated algorithms or compiler logic behind a component label.
Existing installed-mode source-override rejection remains intact; rebuilding
uses explicit source-development mode and verified tools.

Qualification compares declarations to actual providers, checks deterministic
manifest/prelude bytes and wrong-target/ABI/hash failures, and independently
executes lifecycle/resource effects. Reports retain source/tool/provider hashes
and full revision intervals. Declaration admission does not close M7.

## Implemented inventory admission boundary

`selfhost/sdk/native_x64_abi1/manifest.json` is catalog schema one. It records
sixteen exports, fourteen immutable source fingerprints, exact raw prototype
byte ranges, a separate SG declaration prelude, two development reference
components and three language contracts. Physical type codes are zero unit, one
signed i64, two one-byte C bool and three a mutable pointer to the explicitly
declared readiness outcome. That C-layout outcome has size sixteen, alignment
eight and field offsets zero, one and eight. Its explicit output pointer is not
an integer argument or an implicit aggregate return.

`native_sdk_json.sg` implements bounded JSON parsing in SG, including strict
UTF-8, escape/surrogate handling, integer overflow classification, duplicate
decoded keys, trailing-input rejection, node limits and depth limits. The
admission algorithm does not delegate JSON policy or parsing to std::json.
`native_sdk.sg` retains FileSnapshot owners for the manifest, prelude and source
files, verifies caller pins and normalized actual provider prototypes, parses
the prelude with the SG parser and constructs a separate physical C type arena.
Hashing, immutable filesystem snapshots, String/Buffer storage and native test
construction still depend on the existing development runtime; their algorithms
are not claimed as migrated SG runtime implementations.

Inventory admission is separate from artifact selection. The checked-in
inventory has empty artifact lists and therefore authorizes no provider binary.
An explicitly caller-pinned development selection can add a COFF-object
descriptor. Selection verifies the independent caller artifact hash, immutable
file identity, AMD64 physical object header/bounds and actual defined external
function symbols. A name in a symbol table that is undefined does not supply an
export. This first reader accepts ordinary COFF objects; archive/static-library
selection, link dependency closure and release qualification remain unimplemented.
Defined function symbols must reference an existing section and have a value
inside that section's raw extent. Artifact certificate checks repeat catalog
component/ordinal/target/ABI/content-pin relationships, compare the actual
snapshot path against the catalog path anchored at the admitted prelude root,
verify the actual snapshot identity and digest, and rehash the mutable memory
buffer. File stability alone cannot authorize changed in-memory object bytes.
Every current component is `reference-runtime`; final mode rejects it with
5410 even when all supplied content hashes match. Relabeling these existing
algorithm providers as final shims also fails admission.

`native_sdk_bind.sg` creates owned declaration certificates carrying catalog
entry, actual prelude AST declaration, physical function type, prelude physical
source identity and manifest identity/hash. These IDs belong to the admitted
SDK's own AST/type namespaces, not the program's ShChecked binding/type IDs.
Validation is read-only, rechecks all nine entry-table fields and the physical
type graph, and does not intern types. A source import spelling or a same-named
ordinary function cannot provide this certificate. M3/module integration must
map the actual admitted prelude identity and declaration to checked binding IDs;
that mapping and HIR serialization are still open and are not bypassed by these
getters. General source FFI still rejects the aggregate output-pointer prototype.

Future family two enumerates sleep entries six through ten, and family three
enumerates readiness entries eleven through fifteen. The operations are start,
poll, result, cancel and drop. Their physical start result remains i64; their
language Future output and origin are catalog metadata requiring a separate
checked producer/lifecycle fact. Result, cancel and drop are alternative
consuming operations. Declaration certificates alone do not prove scheduler
effects, authorize a safe reference or turn an arbitrary integer into a Future.

`update_native_inventory.py` is an explicit development fingerprint refresh.
It fails on changed physical prototypes rather than rewriting their reviewed
ABI. The native gate checks regeneration bytes and exercises the SG code with
adversarial catalogs and isolated provider roots. Its development COFF selection
uses the exact independently executed runtime_string.c object and report; this
is content/symbol admission of an existing artifact, not a rebuilt or final SDK.
The test executable requires exactly one user argument at index zero. It emits
one completed fixture ID with both expected error codes only after all that
fixture's assertions pass. The Python runner checks the complete ordered unique
list against every prepared fixture; process exit and an overall marker alone
cannot establish that the external corpus ran.
