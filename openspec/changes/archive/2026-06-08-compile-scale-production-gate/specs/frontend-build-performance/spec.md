## ADDED Requirements

### Requirement: Scale optimizations preserve native runtime cache identity

Compile-scale memory work for production gates SHALL NOT weaken runtime bundle
fingerprinting or native artifact reuse rules defined by the canonical
`frontend-build-performance` specification.

#### Scenario: Fingerprint tests remain green after scale optimizations

- **WHEN** frontend memory optimizations land for ladder or 1000k workloads
- **THEN** runtime bundle fingerprint and cache-miss tests still pass
- **AND** `sgc` continues to treat runtime byte changes as cache misses
