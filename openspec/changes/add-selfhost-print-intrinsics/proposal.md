# Add bounded typed print lowering to the SG compiler

The independent native01 probe accepts ordinary main42 but fails at lowering
5002 for print in both called and uncalled functions. Importing std::option
reaches this failure in Option<i64>::expect. Semantic analysis resolves the
callee to compiler builtin binding kind 13, but ordinary MIR lowering treats
that binding as a local after finding no declared MIR function.

This child adds checked print, println and eprintln lowering for signed i64 and
shared str only. It retains existing whole-module body checking and lowering.
Other printable types remain explicit unsupported lowering cases. This is a
development prototype, not M3/M6/M8 completion or a new public language subset.

Implementation owner: add-selfhost-print-intrinsics. The add-selfhost-semantics
child tracks this work as an integration dependency without duplicating it.
The strings-and-formatting canonical requirement owns language behavior. No
runtime implementation or public ABI changes are proposed.
