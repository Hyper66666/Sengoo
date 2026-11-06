# Design

The terminal already normalizes native Windows Tab plus SHIFT_PRESSED as
`kind=1`, `code=9`, `modifiers=1`. Its common ANSI byte parser currently does
not recognize BackTab, so redirected byte streams and POSIX input yield an
Escape followed by literal characters instead.

Recognize exactly `ESC [ Z` after bracketed-paste handling and before the
existing basic navigation table. Consume exactly three bytes and set only the
normalized key code and Shift bit. A prefix of one or two bytes remains pending
under the existing 25 ms escape disambiguation; no extra allocation, unbounded
lookahead or waiting policy is introduced. Bracketed-paste contents remain
literal data. Ordinary Tab and following bytes retain their events.

The C probe first reproduces the missing decoder mapping on the original
source. It then verifies complete input, both split positions, EOF, ordinary
Tab, following bytes and paste literal preservation. An owned hidden Windows
console validates native Shift+Tab and Tab produce the same normalized pair;
no user console is read or modified. A compiled public Sengoo consumer verifies
the same event fields through redirected stdin, independent of the private C
parser test. Windows evidence qualifies common-byte parsing and native Windows
event parity only, not a live POSIX terminal host.
