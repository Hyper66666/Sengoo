# sglsp signature-help resolution

Signature help is recovered from the source prefix at the requested UTF-16
position without requiring a complete parse. The call walker skips quoted
strings, line comments, and nested block comments, and tracks parentheses,
brackets, braces, and generic delimiters independently. Only commas at the
active call's own delimiter depth advance the argument index. The innermost
unclosed callable wins, so nested and incomplete calls remain useful.

An active call records its callee, optional member (`.`) or namespace (`::`)
qualifier, argument index, and argument text. `WorkspaceIndex` supplies the
current document, dependency, and imported-standard-library signatures. Each
indexed signature keeps its canonical `module_path` separate from its optional
canonical `qualified_owner`; lexical declaration data is never used directly
as a cross-document identity. Dependency aliases and explicit import aliases
are expanded through the index. Member calls require a receiver type resolved
from the current document and match only receiver-bearing signatures owned by
that fully qualified type. Namespace calls match an exact canonical module or
associated-type owner. An unresolved or ambiguous qualified call returns no
signature instead of guessing from the final path segment.

Bare receiver types are resolved against exact indexed exports. An import
alias expands only when it is the leading qualifier; selective imports expose
only their named types; wildcard and simple imports contribute a type only when
that module actually exports it. The current module is evaluated as another
candidate. If the current module or more than one imported module exports the
same bare type, resolution returns no signature. Standard-library signatures
are qualified while their source module is collected (for example,
`std::net::HttpClient`), so merging them into a document never assigns the
current document's module identity.

Unqualified calls query a visibility-filtered signature view: current-module
functions plus functions exposed by the document's exact simple, selective, or
wildcard imports. Alias imports require an explicit qualifier. Receiver
resolution reuses completion's cursor-scoped binding and type-chain engine, so
`self`, lexical shadowing, fields, and zero-argument call chains have identical
semantics in completion and signature help; comments and strings cannot create
bindings. The language-server handler and tests both call the same
`signature_help_for_request` builder, including UTF-16 position conversion.

All viable overloads are returned in deterministic parameter-count, label,
and source order. Active-signature selection first compares arity and known
literal argument types, then uses the stable order as its tie-breaker. The
active parameter is clamped against the selected overload; receiver `self` is
shown in the signature label but is not counted as a caller-supplied parameter.
Callable `///` documentation and `/// @param name ...` documentation are
indexed with each signature and emitted through standard LSP documentation
fields.
