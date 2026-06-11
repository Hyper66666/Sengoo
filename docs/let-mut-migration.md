# `let mut` Migration

Sengoo local bindings are immutable by default. Code that reassigns a local
must declare that binding with `let mut`.

```sg
let mut count = 0;
count = count + 1;
```

Assignments through an indexed or field place also require the root local to be
mutable:

```sg
let mut values = [1, 2, 3];
values[0] = 4;
```

The compiler reports immutable reassignment with the stable diagnostic code
`immutable-assignment` and highlights the assignment target. Function
parameters already accept the same `mut` modifier.

This change is source-incompatible only for programs that previously reassigned
plain `let` bindings. Read-only declarations require no changes.
