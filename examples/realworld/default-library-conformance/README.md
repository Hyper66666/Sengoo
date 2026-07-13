# Default Library Conformance

This package is the Phase 1 default-library gate. It uses only generic
collection constructors for user-defined values, exercises lazy iterator
adapters and checked numeric conversion, and verifies that owned String fields
return to their baseline without manual release calls.

Run the locked package loop with `sgpm check --locked`, `sgpm test --locked`,
`sgpm fmt --check --locked`, `sgpm doc --locked`, and `sgpm build --locked`.
