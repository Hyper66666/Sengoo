# async-channel-smoke

`async-channel-smoke` is a package-shaped async fixture. It uses the public
`std::async` channel, mutex, and public cleanup helpers together with
cooperative async `sleep`, `spawn`, and `select`.

It intentionally avoids unsupported/deferred surfaces: no user-defined
`Future::poll` lowering, owned-fd readiness claim, task cancellation API, or
select-loser cancellation claim. Runtime-level tests cover cleanup/drop wakeups;
this package smoke proves the public cleanup wrappers compile and run through
`sgpm`.

Run:

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```
