# Add conservative byte-stream write evidence and preserve repeated console keys

## Problem
An application cannot distinguish a connection failure before an HTTP write from
an ambiguous failure after a write attempt. Windows also merges repeated key-down
records; ignoring their repeat count loses navigation and text input.

## Capabilities
Add `NetByteStream.request_started()` as transport evidence, keeping retry policy
in Sengoo. Preserve every Windows console key repeat and discard pending repeats
at explicit input barriers. Root owns these changes; the ConPTY owner is separate.

## Non-Goals
No automatic network retry, proof of delivery, protocol parser, ABI version bump,
or change to terminal event kinds. Edition 2026 and v0.2 interfaces remain intact.
