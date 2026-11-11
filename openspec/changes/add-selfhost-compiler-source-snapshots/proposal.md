# Compiler-owned retained source snapshots

The actual SG native compiler cannot currently load its own source graph. Its
module loader retains public FileSnapshot owners, whose published process-wide
limit is 32. Opening a duplicate import needs a temporary additional owner, so a
32-module graph can fail before semantic analysis even without a 33rd identity.

Add an SG-owned compiler source arena backed by a narrow Windows retained-handle
adapter. Preserve the public FileSnapshot limits and its existing API. The arena
owns copied source bytes, source identities and every file/ancestor guard until
the compilation finishes. SG controls admission, caller budgets, deduplication,
complete reads, EOF, metadata comparison, UTF-8 and SHA-256. Use the already
qualified SG UTF-8 and streaming SHA implementations rather than another C hash.

Integrate the arena into the real module loader and all consumers of its source
witnesses. Qualify the actual full compiler graph, boundary/duplicate cases,
source mutation controls and first-call resource recovery. A later semantic
rejection is recorded as a remaining self-build dependency, not compiler success.

## Non-goals

This child does not complete the native fixed point, replace every runtime file
API, weaken the retained-identity contract or modify public snapshot capacity.
Final SDK admission, complete self-build and M8 remain separate program gates.
