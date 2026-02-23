# Milestone M4: Network Runtime

## Goal

Deliver a practical runtime networking baseline for compiler/runtime validation and developer demos.

## Current State

- TCP/UDP APIs: complete
- HTTP baseline over TCP (`http://`): complete (`GET`/`POST`, status/body, chunked decode)
- WebSocket baseline (`ws://`, text frame path): complete (handshake checks, ping/pong, close path)
- HTTP server runtime baseline: complete (HTTP/1.1 parsing, route dispatch, middleware guard, WS upgrade echo path)
- Protocol-level network error mapping: complete (`sengoo_net_last_error` + error-name copy API)
- Cross-platform smoke coverage (HTTP + WS echo): complete

## Next Hardening Targets

- [ ] HTTPS / WSS strategy (separate scope)
- [x] Better protocol-level error typing
- [ ] Throughput/latency regression gates

## Planned Critical Gap: HTTP Server Runtime

- [x] HTTP/1.1 server protocol parsing
- [x] Route system (method + path dispatch)
- [x] Middleware support (ordered pipeline + short-circuit)
- [x] WebSocket upgrade from HTTP request path

## Exit Criteria

- [x] Runtime network smoke tests pass on local/CI paths
- [x] HTTP and WS baseline APIs documented
- [x] Protocol errors are diagnosable without relying on only `0/-1` sentinel values
