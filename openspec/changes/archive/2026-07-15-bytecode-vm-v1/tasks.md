## 1. Entry and value review

- [x] 1.1 Confirm coordinator MIR semantic and portable runtime ABI gates.
  - Entry tasks 1.2–1.6 closed on `wasm-and-bytecode-backends`.
- [x] 1.2 Measure packaged native, WASM, and the existing scalar `SGB1`
  prototype; record a go/no-go decision for a production VM.
  - Evidence and decision: `docs/bytecode-vm-value-review.md` → **NO-GO**.
- [x] 1.3 If no-go, create and archive the replacement cancellation decision;
  otherwise continue.
  - Cancellation recorded; tasks 2+ intentionally not implemented.

## 2. Format and verifier

- [x] 2.1–2.3 **Cancelled** by the NO-GO decision (no production format/verifier).

## 3. Interpreter and ownership

- [x] 3.1–3.3 **Cancelled** by the NO-GO decision.

## 4. CLI and differential conformance

- [x] 4.1–4.5 **Cancelled** for production promotion. Experimental
  `sgc build/run --target bytecode` remains a non-supported research path only.
- [x] 4.5 Run `openspec validate bytecode-vm-v1 --strict` (cancellation archive).
