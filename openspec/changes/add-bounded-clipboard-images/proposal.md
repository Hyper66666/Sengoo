## Why

Sencode needs explicit paste of image data, file paths and Unicode text. A
generic Windows clipboard boundary avoids CLI-specific native code while
keeping attachment policy and provider encoding in Sengoo.

## What Changes

- Add read-only image availability, Unicode text and one-snapshot file-list APIs.
- Add bounded PNG export for clipboard PNG, DIB/DIBV5 and bitmap formats.
- Use exclusive file creation and strict path/size validation, preserving all
  existing clipboard and terminal interfaces and runtime ABI 1.
- Verify with isolated clipboard state and real Windows image conversion.

## Capabilities

- New capability: `bounded-clipboard-images`, owner: Sengoo foundation.
- CLI gesture handling, original-path references and TEMP-cache policy remain
  application requirements, owned by the Sencode integration.

## Non-Goals

No automatic clipboard polling, project file copies, attachment persistence,
image display, provider serialization, POSIX clipboard support or package
installation. Tests must not replace the interactive user's clipboard.

## Impact

Additive `std::terminal` functions use dynamically loaded System32 libraries.
No additional compiler or application link flags are required. The Windows
private-window-station integration probe must record unavailable isolation as
unverified, without falling back to the interactive station.
