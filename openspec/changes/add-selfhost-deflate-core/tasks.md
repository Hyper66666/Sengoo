## Raw decoding

- [ ] Implement bounded SG bit input and canonical Huffman decoding with exact limit failures.
- [ ] Implement stored, fixed and dynamic blocks, repeats, length/distance decoding and overlapping/cross-block copies.
- [ ] Keep C adapters limited to memory access and verify every SG access against caller extents.
- [ ] Execute independent compressor/decoder comparisons and explicit malformed/truncated/extent controls.
- [ ] Qualify actual SG object/PE production, two fresh builds and complete source/tool/host/revision evidence.
- [ ] Validate this child and all specs strictly.

## Framing and tool integration

- [ ] Implement and qualify gzip framing/checksums and the actual package consumer's framing policy.
- [ ] Integrate tar/package workflows with unchanged public limits and failures.
- [ ] Qualify final SDK packaging, self-building and performance before replacing the reference package path.
