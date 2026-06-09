# package-release-loop

Realworld package release fixture for `sgpm` defaults. It exercises:

- a dependency alias (`release_alias` -> `release_helper`);
- two selected versions of the same registry package name (`shared_core` 1.x and 2.x);
- a local file registry dependency;
- locked metadata, deterministic dry-run packaging, local publish, and the normal
  check/test/fmt/doc/build package loop.

Run from this directory:

```powershell
sgpm update
sgpm metadata --format json --locked
sgpm publish --dry-run --locked --format json --output target/package
sgpm publish --registry local --locked --format json
sgpm update --check
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```
