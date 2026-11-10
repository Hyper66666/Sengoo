# Development collection provider, namespace two

This extension preserves the existing base SDK's sixteen entries. Its six
exports are the existing C runtime's raw Vec constructor, iterator operations,
destruction and push. C implements the collection algorithms in this development
provider; they have not yet migrated to SG.

The checked-in manifest contains no object. An independently selected build must
bind the complete C/header/tool/link closure and publish its actual COFF hash.
The loader takes independent manifest and object pins, checks all six definitions,
and refuses final mode. A manifest is an inventory, not compilation evidence.

The prelude contributes only the three declarations absent from the standard
collections module. The checked program must physically load that prelude and
the independently pinned standard module. Their bindings and type IDs remain
separate from export ordinals and COFF symbols. The mapping alone grants no
handle, element, lifetime or safe-reference permission.

`sh_collection_provider_load` requires a caller-selected Windows x64 target,
ABI one and explicit development mode. `sh_collection_provider_map` and its
read-only validator require those independent context values again, together
with the selected root, manifest/object hashes and standard module identity/hash.
A previously opened development object cannot silently enter final mode.
Five immutable source snapshots are checked against their full owned text and
actual physical identities. The object snapshot and mutable in-memory bytes
must both match the independent object hash.

Each successful mapping has six rows of eight words:
`[namespace, export, program binding, physical source, declaration AST,
program function type, provider source unit, required contract]`.
The last word declares a precondition; it does not assert that the compiler has
proved it:

| Export | Required contract bit | Compiler obligation |
| --- | ---: | --- |
| new_parts | 1 | Concrete layout and valid move/drop bodies |
| iter_new | 2 | Canonical same-element live backing vector |
| iter_next | 4 | Live iterator, backing lease and element type |
| iter_free | 8 | Valid owned iterator or the allowed zero handle |
| vec_free | 16 | Owned backing with no remaining conflicting loan |
| push | 32 | Valid element transfer, mutation permission and failure cleanup |

The actual declaration's unsafe flag is checked separately. Definition checks
require one external function symbol with an in-bounds address in an executable
code section. The complete C implementation, dependent runtime objects,
headers, selected linker and libraries still need the named build qualification;
this small inventory loader cannot establish compilation provenance by itself.

The six-export local qualification is recorded in
`target/selfhost/m3-collection-provider-ast-checkpoint/evidence.json`
(SHA256 `a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3`).
It passes nineteen commands, actual C runtime execution and five SG first-call
resource scopes. The broader compiler integration tasks remain open.
