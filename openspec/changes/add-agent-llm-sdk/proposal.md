# Shared three-protocol model SDK

## Why
Sencode's tested Provider, HTTP and SSE logic is reusable, but currently lives
inside a terminal application. Independent Sengoo applications need the same
stream parsing and failure semantics without its configuration, state or UI.

## Capabilities
Add experimental `sgllm` 0.1.0 with Chat Completions, Responses and Anthropic
Messages, bounded incremental HTTP/SSE, validated complete tool calls, usage,
caller-owned model/capability configuration, cancellation and structured failures.
Retain the original protocol symbols so Sencode can use thin package adapters.

## Non-Goals
No hosted credential store, model registry, implicit provider fallback, automatic
tool execution, automatic retries, pricing or claims of live-service coverage.
No new language syntax, ABI or standard-library default import.

## Ownership
The sgllm owner owns `packages/sgllm` and this change. The integration owner owns
Sencode adapters, package catalog/distribution and complete cross-package gates.
