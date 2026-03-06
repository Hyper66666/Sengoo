## ADDED Requirements

### Requirement: `llm_scheduler_bench.py` SHALL provide a deterministic CI smoke preset
The benchmark script SHALL provide a fixed tiny preset that completes quickly while still exercising the existing light and heavy scheduler scenarios.

#### Scenario: Running the CI smoke preset
- **WHEN** a user runs `python bench/llm_scheduler_bench.py --preset ci-smoke`
- **THEN** the script uses fixed tiny workload parameters that complete in seconds and still runs both predefined scheduler scenarios

### Requirement: The CI smoke preset SHALL validate parity between Python and Sengoo runners
Smoke execution SHALL only succeed when the Python and Sengoo scheduler runners produce consistent checksums for the same preset workload.

#### Scenario: Checksum mismatch fails the smoke run
- **WHEN** the Python runner and Sengoo runner produce different checksums for the smoke preset workload
- **THEN** the script exits non-zero and reports the mismatch instead of reporting a successful smoke result

### Requirement: The CI smoke preset SHALL emit the standard benchmark report schema
Smoke execution SHALL write the same JSON report structure used by the full benchmark flow so existing result consumers do not need a separate parser.

#### Scenario: Smoke run writes a normal report
- **WHEN** the smoke preset completes successfully
- **THEN** the script writes a JSON report containing the preset inputs and both scenario results using the standard benchmark report schema
