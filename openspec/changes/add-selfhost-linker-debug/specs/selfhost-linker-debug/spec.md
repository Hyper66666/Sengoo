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


#### Scenario: Native compiler metadata supplies automatic source debugging
- **WHEN** supported MIR source/name/type records and checked native machine-code/stack/epilogue maps are supplied
- **THEN** SG derives the CodeView procedure, source-line and initialized-local ranges without fixture machine addresses
- **AND** actual source compilation reaches a deterministic object, PE and PDB using SG alone
- **AND** a real debugger verifies source stops, stepping and expected locals at nonzero function offsets and multiple returns
- **AND** incomplete/conflicting code maps and unsupported or unproven local lifetimes fail explicitly


#### Scenario: Supported scalar prototypes and parameters are debugged
- **WHEN** supported functions declare builtin scalar parameter, result and local types
- **THEN** SG interns and remaps their real CodeView prototypes and preserves typed parameter/local records in the PDB
- **AND** a real debugger observes the declared scalar parameter and local values while stepping
- **AND** malformed type references, counts, parameter flags or prototype mismatches fail before outputs are published


#### Scenario: Supported pointer and reference records preserve their meaning
- **WHEN** supported scalar-pointer or reference types carry checked const qualifiers and backward type references
- **THEN** SG remaps the real LF_POINTER/LF_MODIFIER records and retains their pointer/reference distinction
- **AND** a real debugger observes readonly and mutable referenced values and opaque raw pointer bits correctly
- **AND** unsupported sizes/modes, dangling references and unrepresentable reference combinations fail explicitly
- **AND** the 64-wrapper limit includes const modifiers and rejects an additional wrapper before mutating the type pool

#### Scenario: Supported aggregate fields retain target layout
- **WHEN** supported nongeneric struct, tuple, array and tagged-enum locals carry checked field names and target layouts
- **THEN** SG preserves their real field offsets, sizes, array elements and enum tag/payload layout in COFF and PDB types
- **AND** a real debugger reads initialized struct fields, tuple/array elements and a known active enum payload while stepping source
- **AND** malformed member references, counts, extents, names, padding and stack-frame overruns fail before output publication
- **AND** unsupported recursive, class, generic, function and native C++ property forms fail explicitly

#### Scenario: Concurrent edits cannot replace a compiler producer input
- **WHEN** a native-debug qualification run captures its SG module and standard-library/runtime.c producer closure
- **THEN** the seed compiler reads the recorded immutable snapshots through explicit module/runtime paths
- **AND** the report retains original and snapshot hashes with every producer tool and command
- **AND** later checkout changes are distinguished from any snapshot or guarded runtime/tool mutation

#### Scenario: One SG compilation unit debugs multiple physical files
- **WHEN** a validated physical-source MIR and its actual native metadata describe supported functions from multiple immutable source snapshots
- **THEN** one object and PDB module preserve each file's path and SHA-256 with checked C13 checksum references and DBI file records
- **AND** a real debugger resolves exact physical paths, crosses a source-file boundary while stepping and reads expected locals
- **AND** every input file block agrees with the independent typed-MIR function-source registry
- **AND** malformed, duplicate, unresolved, missing or excessive file/registry records fail before output publication
- **AND** the existing single-source API remains available while mixed-file function origins remain unsupported; independent input COFF units use their separately qualified module contract


#### Scenario: Independent supported SG objects retain separate PDB modules
- **WHEN** every object in a bounded SG link supplies an independently checked source registry, COFF debug records and actual native code facts
- **THEN** PDB output SHALL retain a separate module symbol/C13 stream for every object and remap each object's local type indices into the shared TPI
- **AND** same-named static procedures SHALL remain scoped to their own module while real external calls resolve across objects
- **AND** a shared physical file SHALL retain each contributing module's DBI/FileInfo association with its exact source bytes
- **AND** a real debugger SHALL cross an object boundary and read each module's correctly typed locals
- **AND** invalid types, source ownership, module/stream metadata and incomplete or poisoned programs SHALL reject before output publication
- **AND** one supported all-global unit SHALL preserve legacy raw PE/PDB bytes at identical input paths/options
- **AND** this SHALL NOT claim nondebug SDK/archive units, automatic compiler internalization, different source checkout roots or complete M5/M6/M8 qualification


#### Scenario: A nondebug SDK object is explicitly admitted beside checked debug units
- **WHEN** at least one independently registered debug unit and supported source-locked nondebug COFF objects are submitted for the same actual linked image
- **THEN** every actual object SHALL retain its own module/contribution identity and complete bound symbols, sections and relocations
- **AND** a nondebug submission SHALL reject any `.debug`-prefixed section instead of discarding unsupported or partial CodeView
- **AND** its DBI module SHALL contain zero source files, typed procedures, locals and C13 line bytes, while real external addresses remain in publics and static symbols remain bound metadata
- **AND** missing, duplicate, mislabeled, mismatched, poisoned and all-nondebug programs SHALL fail explicitly before output publication
- **AND** real SG-to-C calls and SG source debugging SHALL be independently executed, with public-address observations distinguished from any unsupported debugger name lookup
- **AND** this SHALL NOT claim arbitrary MSVC aux/COMDAT/CodeView input, invented C type/source records or complete SDK/debugger qualification
