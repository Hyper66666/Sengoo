# Sengoo package registry protocol v1

This document defines the HTTP contract implemented by the filesystem-backed
reference registry and consumed by `sgpm`. The reference server is suitable for
local development, CI, and protocol compatibility tests. It is not a hosted
multi-tenant service and does not replace production authentication, quotas,
replication, or abuse controls.

Start a local registry:

```text
sgpm registry serve --root target/sgpm-registry --listen 127.0.0.1:7878
```

Configure a package:

```toml
[registries.default]
url = "http://127.0.0.1:7878"
token_env = "SENGOO_REGISTRY_TOKEN"
```

All write operations require `Authorization: Bearer <token>`. On the first
successful publish, the package name is reserved to the SHA-256 hash of that
token. The raw token is never persisted. Later publishes, yanks, and unyanks
for that package must use the same token. A production registry may replace
this token-as-owner model with authenticated account identities while
preserving the status codes and route semantics below.

## Routes

### Publish a version

`POST /api/v1/packages/<package>/<version>`

Required headers:

- `Authorization: Bearer <token>`
- `Content-Type: application/gzip`
- `x-sengoo-package: <package>`
- `x-sengoo-version: <semver>`
- `x-sengoo-checksum: <lowercase SHA-256 hex of body>`

The body is the deterministic `.tar.gz` produced by `sgpm publish --dry-run`.
The reference server limits request bodies to 64 MiB, verifies the route and
headers, verifies the checksum, reserves the name, and writes a version
atomically. Existing versions are immutable and return `409 Conflict`.

### List versions

`GET /api/v1/packages/<package>`

Response:

```json
{
  "versions": [
    {
      "version": "1.2.0",
      "checksum": "<sha256>",
      "yanked": false,
      "features": []
    }
  ]
}
```

Versions are returned in ascending semantic-version order. `sgpm` selects the
highest non-yanked version satisfying the dependency requirement.

### Read version metadata

`GET /api/v1/packages/<package>/<version>`

Returns the same version object used by the index.

### Download a version

`GET /api/v1/packages/<package>/<version>/download`

Returns the published archive as `application/gzip`. `sgpm` verifies its
SHA-256 before unpacking. The cache stores both the archive checksum and a
deterministic hash of the extracted file tree; a later locked resolution
rejects missing or modified cache contents before invoking the toolchain and
does not contact the registry. An unlocked resolution may repair the cache by
downloading and verifying the immutable archive again.

Client extraction treats archives as hostile input. Protocol v1 permits at
most 64 MiB compressed bytes, 256 MiB total declared uncompressed bytes, and
10,000 entries. Absolute paths, `..` traversal, symbolic links, hard links,
special entry types, and duplicate normalized paths are rejected in an
isolated staging directory before cache publication.

### Yank or unyank

`POST /api/v1/packages/<package>/<version>/yank`

Optional JSON body:

```json
{"reason":"critical regression"}
```

`POST /api/v1/packages/<package>/<version>/unyank`

Both require the owner bearer token. Yanking prevents new unlocked resolution
from selecting the version. Existing lockfiles can still report the yanked
version so users receive a diagnostic and can update deliberately.

## Names, versions, and errors

Package names use lowercase ASCII letters, digits, `_`, or `-`. Versions are
valid semantic versions. Error responses are JSON:

```json
{"error":"human-readable diagnostic"}
```

The protocol uses:

- `400` for malformed names, versions, headers, JSON, archives, or checksums;
- `401` when a write request has no bearer token;
- `403` when another owner reserved the package name;
- `404` for unknown routes, packages, or versions;
- `409` when a published version already exists;
- `500` for storage or server failures.

## Lockfile contract

Registry entries in `Sengoo.lock` schema version 2 record the selected registry,
version, and archive checksum:

```toml
source.kind = "registry"
source.registry = "default"
source.version = "1.2.0"
source.checksum = "<sha256>"
```

`sgpm update --check` resolves the current registry index. Commands using
`--locked` instead follow schema-v2 dependency edges to exact package ids,
versions, registries, and checksums, validate the verified cache, and perform
no network request. A missing, incomplete, or content-tampered cache fails with
an instruction to run `sgpm update` while online. Path and git source formats
are unchanged.
