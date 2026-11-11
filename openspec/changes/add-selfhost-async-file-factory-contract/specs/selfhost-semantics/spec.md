## ADDED Requirements

### Requirement: AsyncFile Future factories SHALL carry proved source and ABI authority

A runtime-owned Future factory SHALL retain distinct validated physical source,
language signature, provider declaration/ABI, body producer and lifecycle origin
facts. A name, nominal spelling, standard filename or integer representation
SHALL NOT confer the factory capability.

#### Scenario: The canonical file wrapper is checked
- **WHEN** the selected stable std::file source and resolved AsyncFile wrapper satisfy the complete source, receiver, output, C provider and body contract
- **THEN** checking SHALL admit that factory's exact returned Future and its exact final raw-provider-result bridge
- **AND** ordinary method-body checking and unrelated Future escape restrictions SHALL remain active

#### Scenario: A source spoofs or mutates the factory
- **WHEN** source identity, declaration, receiver, output, physical signature, producer operands or complete wrapper body differs from the authorized contract
- **THEN** no Future factory or integer-to-Future capability SHALL be issued
- **AND** the source SHALL retain a located rejection before native emission

#### Scenario: Saved factory facts are cloned, decoded or corrupted
- **WHEN** factory facts pass through a supported HIR snapshot or are changed individually
- **THEN** valid facts SHALL retain their exact owned identities and invalid, missing, duplicate, out-of-range or incompatible facts SHALL be rejected by pure validation
- **AND** authorization SHALL NOT be reconstructed from source or function names

#### Scenario: Mutable module selection records are changed together
- **WHEN** a same-byte local module is relabeled through standard flags, requested symbols, source-request rows or standard-module IDs
- **THEN** factory selection SHALL still require the selected retained standard path and a proved actual standard import request
- **AND** neither mutable record consistency nor a recomputed source anchor SHALL substitute for physical module-consumer validation

#### Scenario: A selected Windows path differs in case
- **WHEN** a selected standard path differs from its retained canonical spelling only under Windows ordinal case comparison
- **THEN** matching SHALL require case-insensitive lookup proved from the already retained ancestor handles without reopening the source
- **AND** sensitive or unknown directory policy SHALL reject case-folded matching while an exact canonical spelling remains eligible

#### Scenario: A valid source factory has no native consumer
- **WHEN** a reachable call requires a factory whose native producer/consumer/provider contract is not implemented and validated
- **THEN** lowering SHALL return a located unsupported diagnostic
- **AND** import-only semantic admission SHALL NOT authorize raw handle erasure, missing cleanup or a native async support claim
