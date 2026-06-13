## Why

The most complex programs written in Sengoo today are the SDL2 graphics packages
and smoke-sized `examples/realworld` fixtures. There is no non-trivial, real
application written in Sengoo that exercises the full language end to end. A
flagship app is how mainstream languages prove "you can actually build with
this," and it is the best regression test that P0–P1 actually compose: ownership
+ `Drop`, generics/traits, strings/formatting, numerics, collections, and
concurrency/async all used together in anger.

## Proposal

Build and maintain a **flagship reference application** in Sengoo — a non-trivial
program of real utility (for example: a static site generator, a JSON/CSV data
CLI with concurrency, a small HTTP service using the production serving path, or
a terminal app). Requirements for the chosen app:

- Uses owned `String` + formatting, generic collections, error handling via
  `Result`/`?`, traits, async/concurrency, and at least one stdlib IO domain
  (file/http/process) — with **zero manual `.free()/.drop()/.close()`** once P0
  lands.
- Ships as an `sgpm` package, builds under the locked workflow, has tests, and is
  exercised in CI on every change (acts as an integration gate).
- Is documented as a worked example and linked from the README.

It depends on P0 and the relevant P1 pillars; it is archived near the end of the
program as living proof of usability.

## What changes

- ADDED: a flagship `sgpm` application package + tests + docs.
- ADDED: a CI integration gate that builds, tests, and runs the flagship app.
- MODIFIED: README/examples index links to the flagship app as the canonical
  "real Sengoo program."

## Non-goals

- A production-operated service (this is a reference application, not a hosted
  product).
- Multiple flagship apps; one well-maintained app is the goal (others can follow).
