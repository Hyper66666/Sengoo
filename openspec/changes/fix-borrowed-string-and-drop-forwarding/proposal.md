# Borrowed String views and destructor receiver forwarding

## Problem

The Sencode terminal screen cannot obtain a borrowed `String` view through
`&String.as_str()`, although MIR already supports the pointer receiver. The
worktree owner also exposed a native crash when a destructor forwards `self`
to a helper expecting `&mut Owner`: its legacy value ABI leaked into its body.

## Capabilities

Resolve `as_str` for shared and mutable String references without changing
borrow escape checks. Give destructor bodies an addressable reference to the
legacy by-value receiver storage, preserving explicit borrowing and field Drop.

## Non-Goals

No runtime ABI, edition, String representation, lifetime-system or public
destructor ABI change. No broad implicit argument coercion.

Owner: root. Baseline `d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus existing
working-tree changes; independent tests identify the modified compiler.
