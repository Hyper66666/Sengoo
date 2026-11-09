# SG M2 syntax architecture

## Tokens and source

The lexer retains trivia and exact byte spans. Keyword/operator identities are
derived from the reference token vocabulary; token text stays in owned source
storage. EOF is explicit. Invalid input produces a byte span and stable internal
error rather than disappearing into a skipped character. String, raw, byte,
character and formatted literals retain source spelling and use checked decoding.

## AST and grammar

`sh_parse(text, source, max_nodes)` returns `ShParsed { ast, error, start, end }`.
Zero error means the entire program parsed, not merely balanced delimiters.
Errors retain the first failing byte span. The bootstrap-internal AST stores
seven words per node: kind, source, start, end, payload, first edge and edge
count. A separate append-only edge arena holds child IDs. Public helpers check
node/field/child indices and return an invalid sentinel for absent indices.

Declarations own name/type/parameter/body children; blocks own statements;
expressions own operands and call/index/member relationships; types and patterns
have structural children. Names/literals retain spans so later semantic passes
can recover spelling. Operator payloads use lexer kinds. Pratt precedences and
assignment associativity follow the reference, including comparison precedence
relative to bitwise operators. Nested generic closing `>>` is split only in
type context. `if`/`while`/`match` conditions suppress ambiguous struct literals.

AST role distinctions are encoded rather than inferred again from source:

| Node | Payload/children |
| --- | --- |
| 58 tuple/paren | 0 parenthesized expression, 1 tuple (including empty and one-element trailing-comma tuples) |
| 80 composite pattern | Opening token kind distinguishes tuple and slice patterns |
| 35 generic parameter below 34 | Bit 0 indicates bounds; bit 1 indicates the last child is its default type |
| 30 import | Alias/selective/wildcard add 16/32/48 to declaration visibility flags |
| 23 field | Public declaration field uses 2; struct-update base uses 1 with one expression child |
| 74 function type | Token kind 101, plus 1000 when the final child is a return type |
| 76 associated type binding | Left type/name and right bound type children |
| 69 formatted string | Parsed interpolation-expression children with absolute source byte spans |
| 89/90 raw token/group | Exact leaf-token payload or delimiter payload with nested group children |
| 91/92 macro definition/invocation | Name/path and token-tree children; expansion remains a separate stage |
| 21 parameter | Bit 0 reference, bit 1 mutable, bit 2 actual leading self receiver; a normal parameter named self is not a receiver |
| 44 block constant | Name, required type and initializer children |

Declaration frames distinguish ordinary top-level declarations, impls, traits,
classes, extern declaration blocks and extern function definitions. Ordinary
and trait/class/impl functions require bodies; only extern-block `fn` items
are prototypes. `fn` is not a top-level synonym for `def`. Bodyless associated
types belong to traits, and extern-block statics omit initializers. Plain
parameters and local `let` names follow the reference identifier grammar;
unsupported destructuring bindings and non-self reference parameter syntax
remain rejected. Return/break/yield are expression nodes, including match arms.

Parser recursion and node/token allocations have explicit bounds. Invalid
maximum values are checked before multiplication. Every loop either consumes
tokens or reports an error; failure must not spin at EOF. This first parser
fails fast with a partial tree and invalid root rather than claiming recovered
success. Depth/capacity diagnostics are separate from syntax errors.

## Transformations and formatting

Raw attributes need structured nodes, but parity with the production pipeline
also requires declarative macro expansion, cfg/deprecation processing, derive,
formatted-expression parsing and synthesized Formatter/Hasher protocols. Keep
those requirements open until tested. Unsupported transformations must not be
silently accepted as ordinary expanded code.

The formatter validates syntax, lays out trivia/tokens and checks the complete
nontrivia token sequence after formatting; idempotence and comments are tested.
Formatting is separate from macro expansion and does not execute user code.

The protocol stage is `sh_protocols(input: &ShExpanded, maximum: i64)` returning
another owning `ShExpanded`. It runs after macro/attribute/derive expansion and
uses the raw AST to locate top-level impls. A simple `Display`/`Debug` trait impl
with a self-taking `fmt` and no method named `to_string` receives the reference
Formatter-backed method; a simple `Hash` trait impl with a self-taking
`hash_into` and no `hash` receives the Hasher-backed method. Existing methods
suppress generation even if static. Qualified trait names, inherent impls and
methods without a self receiver do not trigger the stage, matching the two
post-parse reference passes.

AST-guided insertion of the exact method source before the impl closing brace
lets the existing parser produce normal method/body/name nodes without a second
synthetic AST format or lost virtual-source spelling. Unchanged segments retain
their input mapping mode; generated method segments map to the original impl
site through prior macro/derive mappings. Deprecation metadata is copied without
moving owners from the borrowed input. Reapplying the stage is byte-idempotent.
The required production order is expansion -> protocol synthesis -> final raw
parse; the formatter validates raw source without running these transformations.

## Evidence

Native checks inspect actual child relationships, arithmetic/bitwise precedence,
right-associative assignments, declarations, generic types, patterns and source
spans. Malformed input, EOF, maximum values and deep recursion test bounded
failure. Repeated parse/format scopes must restore runtime resource baselines.

The differential gate records source/tool identities and compares complete
token streams and accepted/rejected parse results against the frozen Rust
reference, including selfhost sources, standard library and the supported
Windows corpus. Passing a narrow fixture suite does not close M2 or later
self-hosting milestones. Reports keep every unsupported discrepancy visible.

## Seed implementation gap

The frozen seed fails to Drop the owning fields of an aggregate temporary when
only a scalar field is projected, for example `sh_parse(...).error`. Five such
negative-test projections retained ten opaque Vec handles. Tests bind each
`ShParsed` to a local owner before inspecting its error field; the unchanged
exact resource assertions then pass. No manual free or weakened baseline is
used. The SG frontend/ownership port must retain an independent regression for
owning-call field projection; this workaround is not proof the seed defect is
fixed. The lexer tests independently observed the same pattern for `ShLexed`.

The reference macro definition extractor also scans physical macro markers
inside SG string literals, and its derive extractor has a related attribute
marker boundary. SG native tests construct those markers from separate runtime
string pieces so the reference seed/formatter does not rewrite the test data
before execution. Preserve independent oracle fixtures for these behaviors;
do not reproduce accidental string/comment preprocessing as intended syntax.

Rust-embedded fixture extraction keeps each literal's original file, byte
offset, exact literal hash and decoded value without substituting format
parameters or appending delimiters. Some reference-positive strings are partial
source prefixes used to construct tests or LSP completion inputs: the reference
parser accepts EOF without a required closing brace. Keep those discrepancies
visible and distinguish them from well-formed supported-language failures.

The exact reviewed list lives in `selfhost/reference-exceptions.json`. Its 17
entries identify decoded source SHA-256 and all original Rust file/byte-range/
literal-SHA origins, a reason and an explicit completion suffix. The extraction
gate asserts reference acceptance, SG error 3001 at the actual EOF, the reviewed
delimiter residue and acceptance of the completed counterpart in both parsers.
Missing/changed entries or an unreviewed discrepancy fail verification. Raw
pre-classification failure reports remain retained under `target/selfhost/`.
This treats test builders and partial editor inputs as explicit negative cases;
it does not reduce support for complete programs or hide a supported mismatch.

`reference_fixtures.py --scan-reference-rejected` additionally records all
reference-rejected literal values in `negative-discovery.json`, retaining their
source, origin and both outcomes. This analysis includes error-message text,
format-string templates and incomplete builders; it is not an automatic blanket
policy for every string containing a declaration keyword. Reviewed complete
program failures such as prototypes, bare fn, bad operands/parameters and
uninitialized aliases become explicit native parser regressions.
