## ADDED Requirements

### Requirement: Borrowed String views preserve ownership
The compiler MUST resolve `as_str` on `&String` and `&mut String` using the
existing view ABI while retaining borrow escape diagnostics.

#### Scenario: Borrowed receiver
- WHEN a helper calls `text.as_str()` for its borrowed String parameter
- THEN the call compiles and reads the live owner's bytes without releasing it.

#### Scenario: Local owner escape
- WHEN a returned view refers through a borrow alias to a local String
- THEN borrow checking rejects the escaping view.

### Requirement: Destructor body references preserve the value ABI
The compiler MUST keep the existing by-value Drop entry ABI and use an
addressable receiver for direct and explicit helper borrows in its body.

#### Scenario: Forwarding and field release
- WHEN a destructor calls a mutable helper with `self` and `&mut self`
- THEN each helper receives a pointer to the same receiver storage and remaining
  owned fields are released exactly once after the body.
