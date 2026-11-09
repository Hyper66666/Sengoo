# Move bounded UTF-8 text algorithms into SG

The self-host compiler still uses development C String/Buffer algorithms.
Begin the runtime migration with allocation-free UTF-8 validation and byte
boundary checks written in SG and compiled by its native backend. Installed
runtime and public stdlib behavior retain their separate storage, ownership
and provider integration gates.

This child owns these algorithms and independent execution tests. Native SDK
owns admission/packaging, and M3 owns type/ownership/lowering. A minimal C memory
adapter performs one byte load without Unicode policy. The unsafe caller
guarantees the complete readable range. Runtime registries, allocation,
normalization, owned String mutation and final replacement remain open.
