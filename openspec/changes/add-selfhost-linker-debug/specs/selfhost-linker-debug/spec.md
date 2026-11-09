## ADDED Requirements

### Requirement: Native object input SHALL be bounded and validated

The SG linker SHALL read supported COFF, archive and import-library records
with checked counts, offsets, names and ownership. Unsupported or malformed
records SHALL fail explicitly before a successful image is published.

#### Scenario: A malformed or unsupported object is linked
- **WHEN** an object contains truncated tables, invalid symbol/relocation targets or an unsupported record kind
- **THEN** the linker reports a stable failure without unchecked access or a successful partial image
- **AND** repeated failed and successful scopes recover tracked resources exactly

### Requirement: SG image linking SHALL resolve and relocate deterministically

The linker SHALL resolve object-local and external symbols and generate PE32+
images directly in SG, including supported AMD64 relocations and native
imports/exports. Equal inputs and options SHALL produce equal unsigned bytes.

#### Scenario: Multiple native objects form an executable
- **WHEN** supported objects define and reference functions/data across object boundaries
- **THEN** the SG-generated executable executes the reference result on Windows
- **AND** independent inspection confirms valid sections, imports and relocations
- **AND** the SG link process does not invoke an external compiler, assembler or linker

### Requirement: Windows unwind and native debug artifacts SHALL be usable

The linker SHALL preserve supported unwind metadata and link CodeView source,
type and local records into deterministic PDB artifacts required by the frozen
Windows support corpus.

#### Scenario: A developer debugs an SG-linked program
- **WHEN** a supported debug build is linked through the SG linker
- **THEN** a real debugger sets a source breakpoint, steps and reads expected local values
- **AND** unwind and PDB artifacts identify the same linked code and source inputs

### Requirement: Complete linker qualification SHALL remain explicit

Partial executable or metadata fixtures SHALL not be presented as complete M5
or final self-hosting proof. The complete supported EXE/DLL/SDK/debug corpus
and final process/equality/performance gates SHALL retain their own evidence.

#### Scenario: An early linker checkpoint passes
- **WHEN** only a bounded object/image subset has executable evidence
- **THEN** its limits and remaining linker/debug tasks stay visible
- **AND** M5 remains open until its full gates pass


#### Scenario: A supported indexed native import library is linked
- **WHEN** a standard COFF archive index identifies required AMD64 short import members
- **THEN** the linker validates its indices and selects required members without external link tools
- **AND** an SG-produced executable calls named native DLL imports through both a code thunk and an IAT
- **AND** unsupported selected regular COFF or import record forms fail explicitly

#### Scenario: Lazy archive dependencies cross library order
- **WHEN** the entry and its strong unresolved references reside in separate supported indexed archives
- **THEN** repeated selection resolves the required closure in deterministic object order
- **AND** unused duplicate members do not introduce false duplicate-definition failures


#### Scenario: A supported SG debug image and PDB are emitted
- **WHEN** checked source, function, line and local records describe supported linked code
- **THEN** SG writes the MSF/PDB streams and matching PE RSDS without an external PDB producer
- **AND** equal inputs and options yield equal PE/PDB bytes with a source-bound identity
- **AND** an independent debugger proves source breakpoints, stepping and expected local values
- **AND** unsupported debug type/module forms and invalid address/lifetime ranges fail explicitly


#### Scenario: Supported relocatable CodeView reaches a PDB
- **WHEN** supported COFF contributions contain CodeView procedure, local and source-line addresses with SECTION/SECREL relocations
- **THEN** SG resolves those addresses and reads the relocated records into the PDB model
- **AND** an independent COFF reader and real debugger verify the resulting debug behavior
- **AND** unknown record forms, invalid scopes/ranges and source-checksum mismatches fail explicitly


#### Scenario: A fixed-frame function has multiple returns
- **WHEN** a supported local occupies a stable SP-relative location across disjoint live ranges separated by epilogues
- **THEN** COFF and PDB preserve the ordered ranges without extending the local through the stack-changing gaps
- **AND** a real debugger reads expected live values and reports the local unavailable in a declared epilogue gap
- **AND** Windows runtime unwinding restores caller state throughout both epilogues
