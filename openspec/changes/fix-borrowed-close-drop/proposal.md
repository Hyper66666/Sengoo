# Preserve ownership across borrowed close/free methods

The MCP SDK exposed an owning client whose `close(&mut self)` terminates and
reaps its process but deliberately leaves field handle release to automatic
Drop. MIR incorrectly treated every method named close/free as a move,
regardless of its signature, suppressing all caller field cleanup.

Keep explicit destructor dispatch and established runtime handle release
compatibility; custom borrowed methods retain ownership. The compiler owner
handles lowering and native evidence. No package-specific manual field cleanup,
ABI change or claim of complete ownership soundness is included.
