## ADDED Requirements

### Requirement: Required deterministic PDB type hash index
The SG single-module and independent-module PDB writers SHALL emit a required, bounded TPI hash/index stream for their existing supported canonical type records, preserving every original type record byte and TypeIndex. Hash and index ranges SHALL match the TPI header and LLVM-compatible rules. The updated producer profile SHALL participate in GUID identity while deterministic same-input and single-vs-N=1 equality remain required.

#### Scenario: Actual Windows symbol consumer loads the indexed product
- **WHEN** a supported SG PE/PDB pair is emitted with type hash/index records
- **THEN** an actual Windows DbgHelp fresh session SHALL load the intended PDB with matching LoadedPdbName SHA256, GUID and age and resolve actual typed SG and nondebug public function names
- **AND** the record bytes/IDs SHALL equal those of the same original type input

#### Scenario: Malformed indexes and hashless controls
- **WHEN** hash values, record/index offsets, stream references, ranges or record extents are corrupted
- **THEN** the independent verifier SHALL reject the candidate
- **AND** missing-hash-stream historical controls SHALL retain their observed DbgHelp failure without counting LLVM parser acceptance as debugger success

#### Scenario: Existing debug boundaries remain explicit
- **WHEN** the writer is qualified
- **THEN** typed SG source/name/address/step/unwind and multiunit/mixed-object regressions SHALL remain valid
- **AND** public-only C functions SHALL NOT acquire fabricated typed PROCREF/source information to bypass LLDB19's independently reproduced name-lookup limitation
