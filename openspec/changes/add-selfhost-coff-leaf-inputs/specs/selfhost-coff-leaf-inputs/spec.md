## ADDED Requirements

### Requirement: Bounded source-locked native COFF leaf input profile
The SG linker SHALL admit its qualified bounded AMD64 leaf profile with zero-initialized BSS, validated ordinary section/file auxiliaries and validated discardable LLVM address-significance metadata. It SHALL preserve primary symbol identity and relocation targets, and SHALL NOT silently accept other metadata or COMDAT/weak/common/directive features under this profile.

#### Scenario: Real leaf BSS and relocated data
- **WHEN** a source-locked clang leaf object defines nonempty BSS, initialized data and an initialized address pointing to BSS
- **THEN** its self-linked PE SHALL execute the same zero/read/write/address behavior as an independent LLD PE
- **AND** its PE raw and virtual BSS extents SHALL match the explicit zero-initialization contract

#### Scenario: Malformed or unsupported records
- **WHEN** auxiliary ownership, extents, symbol indices, address-significance encoding or BSS flags are malformed or an unsupported selection is present
- **THEN** linking SHALL fail with the exact malformed/unsupported diagnostic without replacing prior output
- **AND** an auxiliary continuation SHALL NOT become an ordinary relocation target
