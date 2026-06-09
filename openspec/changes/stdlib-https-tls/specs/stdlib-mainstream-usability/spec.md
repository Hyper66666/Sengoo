## ADDED Requirements

### Requirement: HTTP client helpers accept HTTPS URLs with verified TLS

The `std::http` client surface SHALL support `https://` URLs using the host
platform trust store and SHALL reject insecure certificate verification bypass in
this phase.

#### Scenario: HTTPS GET succeeds against a trusted endpoint

- **WHEN** a program calls `http_client_get("https://example.test/...", timeout_ms)`
  against a test endpoint with a certificate trusted by the host store
- **THEN** the call returns `Ok(HttpResponse)` with a readable status code
- **AND** response body copy helpers behave identically to plain HTTP

#### Scenario: Plain HTTP behavior is unchanged

- **WHEN** a program calls `http_client_get("http://...", timeout_ms)`
- **THEN** behavior matches the pre-change plain HTTP implementation
- **AND** existing realworld and runtime tests continue to pass

#### Scenario: Untrusted or hostname-mismatched certificates fail with stable status

- **WHEN** a program calls `http_client_get` with an `https://` URL whose server
  presents an untrusted or hostname-mismatched certificate
- **THEN** the call returns `Err` with a stable TLS-related `std::status` code
- **AND** the failure is observable in both native tests and `sgc test` smoke paths

#### Scenario: Unsupported schemes remain unsupported

- **WHEN** a program uses non-HTTP schemes such as `ftp://`
- **THEN** the client returns `STATUS_UNSUPPORTED` as before
- **AND** the support matrix documents HTTPS as supported subset and FTP as unsupported

### Requirement: TLS failures SHALL map to stable status categories

HTTPS client failures SHALL use the existing positive `std::status` namespace.
This change SHALL add these stable categories unless a later accepted design
replaces the table before implementation starts:

| Name | Value | Meaning |
| --- | --- | --- |
| `STATUS_TLS_CERT_INVALID` | `15` | certificate chain is untrusted, expired, malformed, or otherwise invalid |
| `STATUS_TLS_HOSTNAME_MISMATCH` | `16` | certificate is valid but does not match the requested host |
| `STATUS_TLS_HANDSHAKE` | `17` | TLS negotiation failed after a backend was available |
| `STATUS_TLS_UNAVAILABLE` | `18` | TLS backend or trust-store capability is unavailable on the host |

#### Scenario: TLS categories are observable through std::status

- **WHEN** HTTPS fails for an untrusted certificate, hostname mismatch,
  handshake-level failure, or unavailable backend
- **THEN** `Result.error` uses the matching `STATUS_TLS_*` category when the cause
  is known
- **AND** `status_name_copy` and `status_message_copy` return stable names and
  human-readable messages for the new categories
- **AND** failures that cannot be distinguished portably return
  `STATUS_TLS_HANDSHAKE` rather than inventing unstable host-specific values

### Requirement: HTTPS scope is documented for production hosts

Sengoo SHALL document TLS client prerequisites (trust store, platform backends, and
CI skip policy) in the realworld support matrix and package README paths.

#### Scenario: Support matrix cites HTTPS proof

- **WHEN** this change archives
- **THEN** `examples/realworld/SUPPORT_MATRIX.md` moves TLS/HTTPS from Deferred to
  Supported subset with a concrete test or example path
- **AND** documented skips name the missing host capability rather than substituting
  fake TLS stubs

#### Scenario: HTTPS tests use real verification

- **WHEN** CI or local tests exercise a successful HTTPS request
- **THEN** the test endpoint certificate is trusted through a documented
  test-specific root-store or host trust setup
- **AND** tests do not pass by disabling certificate or hostname verification

#### Scenario: POSIX reference-host proof is required before archive

- **WHEN** this change reaches archive gate
- **THEN** POSIX/reference-host evidence covers trusted success, hostname
  mismatch, untrusted certificate, and HTTPS runtime roundtrip
- **OR** the support matrix records an evidenced skip that names the missing host
  capability and leaves the claim `Platform-specific`
- **AND** no archive claim relies on fake TLS stubs, `verify=false`, disabled
  hostname verification, or a plain HTTP fallback
