# Observe the synchronous AsyncFile read buffer

After the checked AsyncFile factory passes semantic analysis, the real standard
file module fails ownership analysis at the second use of `buffer` in
`sengoo_async_file_read_into(self.handle, buffer.ptr(), buffer.len())`.
The compiler already proves the Buffer pointer projection, but the enclosing
call is absent from its nonretaining native-call contracts.

Add a restricted contract for argument one of the fixed three-i64 C declaration
returning i64. The reference provider synchronously reads into a local mutable
slice and does not retain the address. This correction owns only that observer
entry and its verification. The factory child owns Future source/HIR admission;
the native SDK child owns runtime provider/lifecycle qualification.

No production implementation or passed qualification is claimed yet.
