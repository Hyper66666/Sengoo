## MODIFIED Requirements

### Requirement: Cheap storage and checkpoint handles

The compiler SHALL store type information in long-lived maps, symbol tables, the trait/impl registry, and inference checkpoints through cheap interned handles rather than repeatedly deep-cloning recursive `Ty` trees. Owned `Ty` values MAY be materialized on demand only where a caller requires a snapshot (for example, diagnostics or an unmigrated adapter boundary).

#### Scenario: Substitution checkpoint cloning

- **WHEN** type inference creates and restores a substitution checkpoint during unification
- **THEN** the checkpoint clones compact type handles instead of recursively cloning all nested type structure

#### Scenario: Environment symbol storage

- **WHEN** the type environment stores variable, function, constant, static, or named type information
- **THEN** the stored representation holds interned type handles rather than owned `Ty`, and an owned `Ty` is materialized only when a caller explicitly needs a snapshot

#### Scenario: Trait and impl registry storage

- **WHEN** the trait/impl registry records method or function signatures (parameter types and return type) and impl target types
- **THEN** those long-lived records store interned type handles rather than owned `Ty`, materializing an owned `Ty` only at lookup boundaries that still require it
