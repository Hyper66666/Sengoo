## ADDED Requirements

### Requirement: Interned type allocation

The compiler SHALL provide a session-local type interner that allocates compiler type shapes and returns stable `TyId` handles for the duration of one type-checking session.

#### Scenario: Reusing an existing structural type

- **WHEN** the same primitive or composite type shape is interned more than once in the same session
- **THEN** the interner returns the same `TyId` for each request

#### Scenario: Distinguishing different structural types

- **WHEN** two type shapes differ by kind, child type, mutability, ADT name, generic argument, function signature, array length, or type variable ID
- **THEN** the interner returns distinct `TyId` values

### Requirement: Interned type lookup

The compiler SHALL expose lookup APIs that resolve a `TyId` to its interned type kind without cloning the full recursive type tree.

#### Scenario: Looking up a composite type

- **WHEN** a caller looks up the `TyId` for a tuple, function, ADT, reference, pointer, slice, array, or future type
- **THEN** the caller can inspect the type kind and child `TyId` handles without constructing owned recursive `Ty` copies

#### Scenario: Looking up an invalid type ID

- **WHEN** a caller attempts to resolve a `TyId` that is not owned by the current interner
- **THEN** the compiler reports an internal type lookup error instead of silently treating it as another type

### Requirement: Compatibility with existing type behavior

The compiler SHALL preserve current type-checking behavior while introducing interned type handles.

#### Scenario: Existing tests continue to pass

- **WHEN** the interned type baseline is implemented
- **THEN** the existing compiler, sgc, runtime, and example smoke tests pass without source-language changes

#### Scenario: Diagnostics remain human-readable

- **WHEN** a type mismatch, undefined type, inference failure, or FFI validation diagnostic references an interned type
- **THEN** the emitted message contains the same user-facing type display information as the owned `Ty` implementation

### Requirement: Cheap storage and checkpoint handles

The compiler SHALL support storing type information in long-lived maps and inference checkpoints through cheap handles rather than repeatedly deep-cloning recursive `Ty` trees.

#### Scenario: Substitution checkpoint cloning

- **WHEN** type inference creates and restores a substitution checkpoint during unification
- **THEN** the checkpoint clones compact type handles instead of recursively cloning all nested type structure

#### Scenario: Environment symbol storage

- **WHEN** the type environment stores variable, function, constant, static, or named type information
- **THEN** the stored representation can reuse interned type handles for repeated type shapes

### Requirement: Incremental migration boundary

The compiler SHALL keep a compatibility path for existing `Ty` and `TyKind` consumers during the baseline migration.

#### Scenario: Unmigrated helper uses owned type API

- **WHEN** an unmigrated helper still expects `Ty` or `TyKind` input
- **THEN** the type checker provides an adapter or compatibility view without requiring a repository-wide rewrite in the same change

#### Scenario: New interned APIs are preferred at storage boundaries

- **WHEN** new code stores type information across type-checker phases or in inference checkpoints
- **THEN** it uses `TyId` or an interner-backed handle unless an owned snapshot is required for diagnostics

### Requirement: No source-language behavior change

The interned type baseline SHALL be an internal compiler representation optimization and MUST NOT change Sengoo source syntax, typing rules, runtime ABI, or generated program behavior.

#### Scenario: Compiling existing programs

- **WHEN** existing Sengoo programs, examples, and tests are compiled after the interning baseline
- **THEN** accepted programs remain accepted, rejected programs remain rejected for the same user-facing reasons, and generated runtime behavior is unchanged
