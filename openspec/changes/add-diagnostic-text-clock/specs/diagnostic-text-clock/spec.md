## ADDED Requirements

### Requirement: Lossy diagnostic decoding SHALL be an explicit opt-in

`string_from_utf8_lossy` SHALL preserve valid UTF-8 and replace each maximal
ill-formed subsequence with U+FFFD. It SHALL read only the requested initialized
prefix, preserve embedded NUL, leave its input alive and unchanged, and reject
invalid lengths. Existing strict constructors SHALL still return status 20 for
malformed input. The decoder SHALL be implemented in Sengoo.

#### Scenario: A diagnostic has a valid prefix and malformed suffix
- **WHEN** a Buffer contains valid text, malformed bytes and more valid text
- **THEN** only malformed subsequences become U+FFFD and valid spans survive
- **AND** independent UTF-8 vectors, embedded NUL and input lifetime checks pass

#### Scenario: An explicit prefix excludes malformed initialized trailing bytes
- **WHEN** the caller requests only the valid initialized prefix
- **THEN** the result contains that prefix without inspecting excluded bytes
- **AND** negative or beyond-initialized lengths are rejected with status 2

### Requirement: Elapsed time SHALL use an independent monotonic clock

`time_monotonic_ms` SHALL expose nondecreasing milliseconds with an unspecified
origin using the host monotonic clock. It SHALL return -1 when unavailable and
SHALL NOT fall back to calendar time. Existing calendar and elapsed APIs and
runtime ABI SHALL remain compatible.

#### Scenario: Monotonic reads span a sleep
- **WHEN** a native consumer repeatedly reads monotonic time and sleeps
- **THEN** values do not decrease and the elapsed interval advances
- **AND** calendar time continues to be available independently
