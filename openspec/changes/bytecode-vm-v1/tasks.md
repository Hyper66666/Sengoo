## 1. Entry and value review

- [ ] 1.1 Confirm coordinator MIR semantic and portable runtime ABI gates.
- [ ] 1.2 Measure packaged native, WASM, and the existing scalar `SGB1`
  prototype; record a go/no-go decision for a production VM.
- [ ] 1.3 If no-go, create and archive the replacement cancellation decision;
  otherwise continue.

## 2. Format and verifier

- [ ] 2.1 Specify magic/version/ABI/checksum, type/layout, function/block,
  instruction, constant, host-call, and resource-limit sections.
- [ ] 2.2 Implement serializer/deserializer and verifier for bounds, types, CFG,
  initialization, moves, calls, Drop plans, and limits.
- [ ] 2.3 Add malformed/truncated/mutated bytecode corpus and fuzz target.

## 3. Interpreter and ownership

- [ ] 3.1 Implement scalar/control-flow/call/aggregate operations required by
  core conformance.
- [ ] 3.2 Implement String/generic values, move/borrow state, exact Drop, and
  early/error cleanup.
- [ ] 3.3 Implement bounded versioned host calls and reject dynamic native FFI.

## 4. CLI and differential conformance

- [ ] 4.1 Promote or replace the experimental `sgc build --target bytecode` and
  clang-free `sgc run --target bytecode` paths.
- [ ] 4.2 Run core conformance identically on native and VM, including stable
  failure categories.
- [ ] 4.3 Prove clang is absent/unusable during VM e2e on Windows, Linux, macOS.
- [ ] 4.4 Record startup/artifact/throughput/value metrics and capability matrix.
- [ ] 4.5 Run `openspec validate bytecode-vm-v1 --strict` and all strict.
