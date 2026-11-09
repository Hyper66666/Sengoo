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
