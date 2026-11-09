# Semantic module fixtures

These are deliberate positive and negative inputs for `tests/semantic_modules.sg`.
Do not classify every `.sg` file in this directory as a supported positive corpus.

| Fixture | Expected use |
| --- | --- |
| modules/root.sg, middle.sg, leaf.sg | Dependency-first diamond; shared physical leaf |
| modules/mapped.sg, 中文.sg | Unicode module-map target |
| modules/identity.sg | Two names mapped to one physical input |
| modules/standard.sg | Supported standard import |
| modules/cycle_a.sg, cycle_b.sg | Cycle rejection 5202 |
| modules/missing.sg, standard_missing.sg | Missing import rejection 5201 |
| modules/error_root.sg, error_leaf.sg | Located dependency type error 5104 |
| modules/invalid_utf8.sg | Intentional raw FF byte; UTF-8 rejection 5207 |
| modules/bom_root.sg, bom_leaf.sg | Physical-file leading BOM and original byte mapping |
| modules/embedded_bom.sg | Interior BOM remains a lexical rejection |

The native test also checks malformed maps and module/byte limits, and recovers
String, Buffer, opaque and FileSnapshot resources exactly after ten rounds.
