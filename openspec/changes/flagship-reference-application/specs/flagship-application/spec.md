## ADDED Requirements

### Requirement: A flagship reference application SHALL exist and exercise the matured language

The project SHALL maintain one non-trivial application written in Sengoo that
uses owned strings + formatting, generic collections, `Result`/`?` error
handling, traits, at least one stdlib IO domain, and async/concurrency, with no
manual resource release.

#### Scenario: Flagship app exercises core capabilities

- **WHEN** the flagship application is built and run
- **THEN** it performs its documented real function
- **AND** its source uses owned `String`/formatting, generic collections,
  `Result`/`?`, traits, and an IO domain
- **AND** its source contains zero manual `.free()`, `.drop()`, or `.close()`
  calls

### Requirement: The flagship app SHALL be a CI integration gate

The flagship application SHALL be built, tested, and run in CI on every change so
it acts as a living integration test of the language.

#### Scenario: Flagship app gates CI

- **WHEN** a change is proposed to the repository
- **THEN** CI builds, tests, and runs the flagship app
- **AND** a failure in the flagship app fails the change

#### Scenario: Flagship app is discoverable

- **WHEN** a new user reads the README or examples index
- **THEN** the flagship application is linked as the canonical real Sengoo
  program with documentation
