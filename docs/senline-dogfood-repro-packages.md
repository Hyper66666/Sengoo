# Worker / HTTP Package Dual-Build Compare (task 8.7 partial)

## Scripts

| Script | Role |
| --- | --- |
| `scripts/package-senline-worker.ps1` | Installed-toolchain worker package + `worker-manifest.json` |
| `scripts/package-senline-http.ps1` | Installed-toolchain HTTP dogfood package + `http-manifest.json` |
| `scripts/compare-senline-package-manifests.ps1` | Normalized dual-manifest compare (optional executable drift) |

Toolchain dual-build remains covered by `scripts/package-toolchain.ps1` +
`scripts/compare-distribution-manifests.ps1` and CI run `29419695542`.

## Local Windows x64 evidence (this host)

Installed toolchain used for packaging:

- `target/dist/sengoo-0.1.0-senline-dogfood-x86_64-pc-windows-msvc/bin/{sgc,sgpm}.exe`
- `sgc --version`: `sgc 0.1.0 (a96518ddb68f)` (package identity; not the dogfood branch tip)

### Worker dual package

```text
package A/B -> target/senline-pkg/worker-{a,b}/
compare     -> target/senline-pkg/worker-compare/comparison.json
result      -> ok=true identical_payload_count=31 executable_drift=0
```

All fixtures, lockfiles, docs, and the release worker executable matched across
two consecutive package invocations (second build was a cache hit; still two
package staging trees with independent manifests).

### HTTP dogfood dual package

```text
package A/B -> target/senline-pkg/http-{a,b}/
compare     -> target/senline-pkg/http-compare/comparison.json
result      -> ok=true identical_payload_count=4 executable_drift=0
```

## Dual-host CI (closes task 8.7 worker/HTTP gap)

GitHub Actions core-conformance run
[`29430796769`](https://github.com/Hyper66666/Sengoo/actions/runs/29430796769):

- `installed worker/HTTP (windows-latest)` green
- `installed worker/HTTP (ubuntu-latest)` green
- Artifacts: `senline-installed-packages-windows-x86_64`,
  `senline-installed-packages-linux-x86_64` (comparison + manifests)

Toolchain dual-build remains covered by run `29419695542`.
