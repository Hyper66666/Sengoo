# Design

New `json_parse_limits`, `json_parse_buffer_limits` and
`json_doc_object_limits` accept 1..16MiB and 1..65536 nodes/container links.
Limits are validated before input reads. Explicit node exhaustion returns13;
legacy constructors retain their4096-node parse10 behavior. Copies roll back
new nodes and resource charges on failure. Serialization retains byte bounds.

Reservations cover byte payload/builders plus the greater of4MiB or
`max_nodes * (2*sizeof(Node) + 8*sizeof(Member) +32)` structural space.
The existing512MiB/64-document aggregate admission remains. Close, failed
parse and Drop release the reservation. No thread-safe access is introduced.
