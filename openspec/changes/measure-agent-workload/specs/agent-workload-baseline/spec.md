## ADDED Requirements

### Requirement: Reproducible project build observations
The benchmark SHALL build one fixed external locked registry consumer through
new-project, unchanged-hot and business-body-only phases, reporting actual costs and reuse evidence.

#### Scenario: Isolated project cache and one-file change
- **WHEN** the new external workspace has no project build cache and then only its business body changes
- **THEN** source/lock hashes prove the phase boundaries and actual emitted compiler counters are retained
- **AND** elapsed time and owned process RSS are measured without claiming OS caches were flushed

### Requirement: Measured four-stream lifecycle
The benchmark SHALL drive four real SDK jobs using fragmented local synthetic
streams, then cancel four jobs and verify complete resource restoration.

#### Scenario: Completion and cancellation observations
- **WHEN** all four streams receive fragmented Unicode and later cancellation is requested for a new wave
- **THEN** canonical output and concurrent worker observations are verified and timing distributions preserve their raw samples
- **AND** cancellation returns to all captured resource baselines before the trial succeeds

#### Scenario: Queue metric boundary
- **WHEN** the runtime has no public native byte-queue occupancy getter
- **THEN** native high-water is unavailable/null while configured capacity and sampled SDK decoder pending are reported separately
- **AND** capacity or application pending bytes cannot be labelled a measured native queue high-water
