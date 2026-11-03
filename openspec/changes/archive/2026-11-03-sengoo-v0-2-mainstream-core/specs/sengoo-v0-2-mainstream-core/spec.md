## ADDED Requirements

### Requirement: Sengoo v0.2 SHALL prioritize one coherent production-native path

The v0.2 program SHALL close baseline, language, developer-loop, standard-library,
and stability milestones on the production native backend before claiming new
experimental backend breadth as mainstream support.

#### Scenario: Experimental work is available before a native milestone closes

- **WHEN** WASM, bytecode, Cranelift, GUI, or game work is implemented while an
  M0-M4 native milestone remains open
- **THEN** that work remains Experimental or separately scoped
- **AND** it does not satisfy or bypass the native milestone archive gate

### Requirement: Every v0.2 milestone SHALL have one owner and an independent archive gate

M0-M4 SHALL each be owned by a named child change whose capability deltas,
tasks, evidence, and deferrals are independently reviewable.

#### Scenario: Two changes claim the same requirement

- **WHEN** two active changes would modify the same canonical requirement
- **THEN** M0 assigns one implementation owner before either change edits code
- **AND** the other change records a dependency or integration task instead

### Requirement: v0.2 support claims SHALL be executable from installed artifacts

The final program SHALL prove its claimed default path using installed release
artifacts outside the source checkout on every supported host.

#### Scenario: The umbrella is proposed for archive

- **WHEN** `sengoo-v0-2-mainstream-core` is proposed for archive
- **THEN** all M0-M4 children are archived
- **AND** one commit SHA passes native language, runtime, toolchain, realworld,
  compatibility, safety, performance, and strict OpenSpec gates
- **AND** the language reference and support matrix cite that evidence
