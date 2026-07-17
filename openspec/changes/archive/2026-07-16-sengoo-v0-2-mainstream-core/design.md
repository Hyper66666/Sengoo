## Context

The language-maturity roadmap is archived for its agreed native and
experimental-scalar scope. The v0.2 program begins from that baseline and closes
the remaining default-path inconsistencies without reopening already archived
programs.

## Decisions

### D1: Native production is the reference semantics

LLVM-text plus the supported native runtime remains the production reference.
Experimental WASM, bytecode, and Cranelift work may continue independently but
cannot substitute for an M0-M4 gate.

### D2: Use one umbrella plus five archivable children

Each milestone has one child change and one archive gate. The umbrella contains
only cross-milestone requirements. This avoids a single epic that cannot archive
and prevents duplicate capability ownership.

### D3: Retain existing owners

- `native-debug-info` owns debug metadata and debugger transcript requirements.
- `http-production-serving` owns HTTP handlers, keep-alive, response streaming,
  and TLS server behavior.
- `v0-2-developer-loop` and `v0-2-production-stdlib` consume those archives and
  own integration only.

### D4: Stop adding breadth until M1 is complete

Before M1 archives, new language syntax requires a blocking defect or explicit
replacement of an M1 requirement. Tests and diagnostics for existing constructs
take priority over additional syntax.

### D5: Support claims require real entry points

A capability counts only when exercised through the same installed CLI/runtime
path users receive. Unit tests remain necessary but are not sufficient for an
umbrella archive claim.

## Dependency order

```text
M0 baseline
  -> M1 language coherence
  -> M2 developer loop
  -> M3 production stdlib
  -> M4 stability contract
  -> umbrella integration/archive
```

M2 may prepare in parallel with M1 but cannot archive until M1 diagnostics and
syntax contracts are frozen. M3 implementation may proceed independently after
M0, but its public API archive follows M1. M4 fixtures begin at M0 and close last.

## Archive gate

The umbrella archives only when:

1. all five child changes are archived;
2. `native-debug-info` and `http-production-serving` are archived or explicitly
   deferred with support claims left unchanged;
3. one commit SHA passes the complete native safety, compatibility, performance,
   toolchain, realworld, and OpenSpec verification wave;
4. the language reference and support matrix cite that evidence;
5. no required evidence exists only in an untracked file or obsolete branch.
