## ADDED Requirements

### Requirement: String conversion helpers SHALL parse and format decimal i64 values
The standard library SHALL provide a source-level `std::strconv` module for
portable decimal `i64` parsing and formatting.

#### Scenario: A program parses a decimal i64 string
- **WHEN** a Sengoo program imports `std::strconv`
- **AND** calls `strconv_parse_i64("  -42\n")`
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` with value `-42`

#### Scenario: A program parses bytes read into a managed Buffer
- **WHEN** a program has a managed `Buffer` containing decimal ASCII bytes
- **AND** calls `strconv_parse_i64_buffer(buffer, len)` with the number of
  meaningful bytes
- **THEN** the helper parses only that byte range
- **AND** returns an ok-shaped `Result<i64, i64>` with the parsed value

#### Scenario: Invalid or overflowing input is rejected
- **WHEN** a program parses empty input, non-numeric input, input with
  non-whitespace trailing characters, or an overflowing decimal integer
- **THEN** the helper returns an error-shaped `Result<i64, i64>`

#### Scenario: A program formats an i64 into a managed Buffer
- **WHEN** a program calls `strconv_format_i64(value, buffer)`
- **THEN** the helper writes the base-10 ASCII representation into the Buffer
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  written
- **AND** it does not append a NUL terminator

#### Scenario: Advanced conversion features remain explicitly deferred
- **WHEN** a future implementation needs floats, radix-specific parsing,
  locale-specific formatting, arbitrary precision integers, owned-string
  returns, or JSON/data-format conversion
- **THEN** it first updates OpenSpec with API shape, ownership constraints,
  portability constraints, and tests
