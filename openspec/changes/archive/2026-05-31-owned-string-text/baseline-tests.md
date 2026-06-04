# Baseline tests (must stay green)

These existing suites cover borrowed `&str` and managed `Buffer` behavior. They must not regress while landing owned `String`.

## Compiler

- `cargo test -p sengoo-compiler string_`
- `cargo test -p sengoo-compiler --lib stdlib_surface_tests::string_module`

## SGC stdlib integration

- `cargo test -p sgc stdlib_string` (if present)
- `cargo test -p sgc "stdlib_"` for full stdlib surface

## Examples (Buffer / &str)

- `examples/stdlib/18_status_buffer.sg`
- `examples/stdlib/18_json.sg`
- `examples/stdlib/19_process_capture.sg`
