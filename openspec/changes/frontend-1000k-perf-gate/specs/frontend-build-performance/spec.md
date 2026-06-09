## ADDED Requirements

### Requirement: Frontend performance work preserves native runtime cache identity

Frontend memory and compile-time optimizations for the 1000k workload SHALL NOT
weaken runtime bundle fingerprinting or native artifact reuse rules defined by
the canonical `frontend-build-performance` specification.

#### Scenario: Runtime fingerprint tests remain green after perf changes

- **WHEN** frontend memory optimizations land for the 1000k workload
- **THEN** all existing runtime bundle fingerprint and cache-miss tests still pass
- **AND** `sgc` continues to treat runtime byte changes as cache misses
