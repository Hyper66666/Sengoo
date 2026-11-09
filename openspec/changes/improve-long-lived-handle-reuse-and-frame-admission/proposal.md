# Long-lived handle reuse and geometry-checked frame delivery

## Capabilities
Reuse freed String, Buffer and opaque handle slots without scanning every retained
live slot. Add `Terminal.write_frame(text,width,height)` with live geometry
checks and deliver same-size Windows buffer invalidations as resize events.
Native terminal calls only handle generic geometry/I/O, not application layout.

## Non-Goals
No handle encoding, published ABI version, strict text, lifetime or status-code
changes. No application state, permission or model logic in native code.

## Ownership
Sengoo integration owns runtime, SG surface and independent native regression.
The CLI owns layout, invalidation, settled repaint, history and input behavior.
