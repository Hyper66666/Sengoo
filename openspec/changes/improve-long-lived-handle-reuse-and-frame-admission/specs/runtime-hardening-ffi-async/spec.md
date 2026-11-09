## ADDED Requirements

### Requirement: Constant-time reuse of released generational handles
Released String, Buffer and opaque handle slots SHALL be reused without scanning live
slots, preserving generation, domain, invalid-handle and cleanup behavior.

#### Scenario: Long-lived retained data and temporary allocations
- **WHEN** temporary strings or buffers are created and dropped with 20000 retained values
- **THEN** freed-slot lookup takes constant time and retained values remain valid
- **AND** stale handles cannot free/read replacements and final live counts return to baseline

### Requirement: Geometry-checked terminal frame delivery
Terminal SHALL offer additive frame writing checked against live dimensions
without owning application layout or consuming input events.

#### Scenario: Geometry mismatch or concurrent resize
- **WHEN** the expected dimensions differ before or after frame output
- **THEN** frame delivery returns false so the caller invalidates its cache
- **AND** ordinary output and queued input retain their existing contracts

#### Scenario: Same-size Windows buffer invalidation
- **WHEN** a Windows buffer notification arrives after an away-and-back geometry change
- **THEN** a resize event is delivered even if final dimensions equal the previous poll
