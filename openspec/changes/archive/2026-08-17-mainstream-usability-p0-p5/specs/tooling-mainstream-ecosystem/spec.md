## ADDED Requirements

### Requirement: Successful builds and runs SHALL produce quiet default output

`sgc build` and `sgc run` SHALL print only program-relevant output on success by
default. Compiler progress and cache instrumentation SHALL be available behind
`--verbose`.

#### Scenario: A successful run prints only program output

- **WHEN** `sgc run` compiles and executes a program without errors at default
  verbosity
- **THEN** the output contains the program's own output and a single result line
- **AND** it does not contain cache-miss lines, workset manifest paths, frontend
  session or scheduler statistics, generic-instance cache counters, or
  pass-through toolchain include-path warnings

#### Scenario: Verbose output restores full detail

- **WHEN** the same command runs with `--verbose`
- **THEN** the previously default instrumentation is printed

#### Scenario: Actionable diagnostics are never suppressed

- **WHEN** compilation produces an error or a warning about the user's program
- **THEN** the diagnostic is printed at default verbosity

#### Scenario: Machine-readable output is unaffected

- **WHEN** `--error-format json` is selected
- **THEN** the JSON diagnostic payload and its stable codes are unchanged by the
  verbosity contract

### Requirement: Block formatting SHALL respect the configured maximum width

`sgfmt` SHALL render a block across multiple lines when its single-line form
would exceed `max_width`, for every block form rather than only function bodies.

#### Scenario: A long conditional body is rendered across lines

- **WHEN** an `if`, `while`, `for`, `loop`, `match` arm, `async`, `parallel`, or
  `try` block would exceed `max_width` on one line
- **THEN** the block is rendered with one statement per line

#### Scenario: A short block stays inline

- **WHEN** the same block fits within `max_width` on one line
- **THEN** it is rendered inline, unchanged from current behavior

#### Scenario: The configured width takes effect

- **WHEN** `max_width` is set through `--max-width` or `sgfmt.toml`
- **THEN** the chosen value determines where blocks break
- **AND** the default remains 100

#### Scenario: Idiomatic multi-line source is accepted as formatted

- **WHEN** `sgfmt --check` runs over a source file whose conditional bodies are
  written one statement per line and exceed `max_width` inline
- **THEN** the file is reported as already formatted

### Requirement: Compiler diagnostics SHALL use a single language

Diagnostic text emitted by the compiler, `sgc`, and `sglsp` SHALL be English.

#### Scenario: Previously localized diagnostics are translated

- **WHEN** a program triggers a diagnostic whose message was previously Chinese,
  such as an undefined variable, an unknown method, or an argument-count
  mismatch
- **THEN** the message is English
- **AND** the diagnostic keeps its existing stable code

#### Scenario: Stable codes and JSON shapes are preserved

- **WHEN** tooling consumes diagnostics through `--error-format json` or the LSP
- **THEN** stable codes, ranges, and payload shape are unchanged by the language
  unification
