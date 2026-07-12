# Sengoo Registry Protocol

This document defines the reference HTTP protocol that `sgpm` registry support
will implement. It is intentionally small: path/git dependencies continue to use
the existing resolver, while registry dependencies use these endpoints and
content hashes.

## Objects

Package names are lowercase ASCII identifiers with optional `-` separators.
The reference registry reserves a package name for the first authenticated owner
that publishes or explicitly reserves it. Versions use semver.

A package version contains:

- `name`: package name.
- `version`: semver version.
- `manifest`: normalized `Sengoo.toml` metadata.
- `archive_sha256`: lowercase hex SHA-256 of the uploaded `.sgpkg` archive.
- `archive_size`: byte length of the uploaded archive.
- `yanked`: whether new resolution may select this version.
- `published_at`: RFC 3339 timestamp.
- `owners`: owner ids allowed to publish or yank the package.

## Authentication

The protocol uses bearer tokens:

```text
Authorization: Bearer <token>
```

Reference-server tokens are opaque. Production hosting may replace the token
issuer, but endpoint authorization semantics must stay compatible.

## Endpoints

### Reserve Name

```text
PUT /api/v1/packages/{name}/reservation
```

Authenticated. Reserves an unpublished package name for the caller. Returns
`201 Created` when reserved, `200 OK` when the caller already owns it, and
`409 Conflict` when another owner controls the name.

### Publish Version

```text
PUT /api/v1/packages/{name}/versions/{version}
Content-Type: application/vnd.sengoo.package+gzip
Digest: sha-256=<base64-sha256>
```

Authenticated. Uploads an immutable package archive. The server computes the
archive SHA-256 and rejects mismatched `Digest` headers. Publishing the same
`name@version` twice returns `409 Conflict`; use a new version instead.

Successful response:

```json
{
  "name": "example",
  "version": "1.2.3",
  "archive_sha256": "0123...",
  "archive_size": 12345,
  "yanked": false
}
```

### Yank Or Unyank Version

```text
PATCH /api/v1/packages/{name}/versions/{version}/yank
Content-Type: application/json

{ "yanked": true, "reason": "bad release" }
```

Authenticated owner-only endpoint. Yanked versions remain downloadable when a
lockfile already pins their hash, but new resolution must not select them unless
the user explicitly asks to allow yanked versions.

### List Versions

```text
GET /api/v1/packages/{name}/versions
```

Returns version metadata sorted by semver ascending. Resolver clients must
ignore yanked versions by default.

```json
{
  "name": "example",
  "versions": [
    {
      "version": "1.2.3",
      "archive_sha256": "0123...",
      "archive_size": 12345,
      "yanked": false,
      "published_at": "2026-07-07T00:00:00Z"
    }
  ]
}
```

### Version Metadata

```text
GET /api/v1/packages/{name}/versions/{version}
```

Returns the same metadata plus normalized manifest dependency information.

### Download Archive

```text
GET /api/v1/packages/{name}/versions/{version}/download
```

Returns the `.sgpkg` bytes. Clients must compute SHA-256 and compare it with the
metadata or lockfile hash before unpacking.

## Lockfile Entries

Registry lock entries record source identity and content hash:

```toml
[[package]]
name = "example"
version = "1.2.3"
source = "registry+https://registry.example/api/v1"
archive_sha256 = "0123..."
```

Resolution is deterministic: for a given manifest, registry index state, and
lockfile, `sgpm update` must choose the same highest compatible non-yanked
version. `sgpm build` must verify the locked archive hash and fail before
unpacking on mismatch.

## Error Responses

Errors use JSON:

```json
{
  "error": "name_reserved",
  "message": "package name is owned by another account"
}
```

Stable error names:

- `unauthorized`
- `forbidden`
- `not_found`
- `name_reserved`
- `version_exists`
- `checksum_mismatch`
- `invalid_manifest`
- `invalid_archive`
- `yanked`
- `server_error`

## Reference Server Scope

The reference server must implement all endpoints above with local durable
storage suitable for e2e tests. It does not need to be a production hosted
service, provide a search UI, or implement account management beyond opaque
owner tokens.
