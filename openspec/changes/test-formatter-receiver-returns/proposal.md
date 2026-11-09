# Verify formatter receiver-return semantics

SDK formatting exposed an old locally built sgfmt whose embedded parser split
`return self.read()` into a bare return and an unreachable call. Current parser
source already includes SelfLowerKw in expression starts. Add a formatter AST
regression and rebuild the tool from the current source; do not alter SDK logic
to accommodate a stale formatter or claim a new parser fix that already exists.
