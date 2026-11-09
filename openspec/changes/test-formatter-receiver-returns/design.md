# Design

Integration owner owns tools/sgfmt/src/lib.rs and this regression. Assert both
formatted text and reparsed AST: the forwarding method contains exactly one
return expression with a method-call operand. Native SDK builds validate the
resulting formatted code. Pin the formatter's binary identity separately from
the compiler; a version string alone does not prove matching development state.
