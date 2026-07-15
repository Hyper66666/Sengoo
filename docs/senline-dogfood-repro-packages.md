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

## Remaining for full 8.7

1. Repeat worker + HTTP dual package compare on Linux x64 with the installed
   archive from distribution packaging (not only Windows).
2. Prefer two independent clean target directories / cold builds when claiming
   PE/ELF bit-identity; Windows PE may still require `-AllowExecutableHashDrift`
   if linker non-determinism reappears.
3. Attach SBOM/provenance fields already produced by toolchain manifests to the
   consumer pin package set.
