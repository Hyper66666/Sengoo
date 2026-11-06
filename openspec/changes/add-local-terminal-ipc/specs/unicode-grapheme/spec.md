## ADDED Requirements

### Requirement: Extended grapheme boundaries SHALL use pinned Unicode data in Sengoo
`std::unicode` SHALL expose a reusable grapheme state whose scalar push reports
the boundary before that scalar. It SHALL implement Unicode 17 UAX #29 extended
grapheme rules including Indic conjuncts, emoji ZWJ and regional indicators, and
pass all 766 official GraphemeBreakTest cases. Data and Unicode license SHALL be
vendored with source checksums; builds SHALL not download property data. State,
property lookup and boundary algorithms SHALL remain Sengoo source code.

#### Scenario: Chunked emoji and Indic input
- **WHEN** scalars from a family emoji, flag or Indic conjunct arrive in chunks
- **THEN** preserving the grapheme state produces the same boundaries as one input
- **AND** invalid scalar values are rejected without corrupting the state

### Requirement: Grapheme cursor and cell APIs SHALL retain owned UTF-8 length
Next/previous boundary operations SHALL borrow String and accept byte offsets
that are existing grapheme boundaries. They SHALL reject invalid offsets. The
width policy SHALL count W/F East Asian text and emoji presentation as two cells,
ambiguous width as one, and combining/default-ignorable marks as zero, combining
width per grapheme. It SHALL document that terminal fonts/shaping, sanitization,
tab expansion and bidi layout remain outside this policy.

#### Scenario: Deleting a composed character
- **WHEN** the previous boundary is requested after a combining sequence,
  emoji family or flag
- **THEN** the returned byte offset addresses its entire preceding grapheme
- **AND** embedded NUL does not truncate the owned input length
