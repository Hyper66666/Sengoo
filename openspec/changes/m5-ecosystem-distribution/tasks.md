# Implementation and verification

- [x] 1. Build deterministic official package archives, checksums and a portable local registry bundle (two runs reproduced all SHA-256 values).
- [x] 2. Publish a curated package catalog with versions, prerequisites, supported hosts and examples (repository documentation only).
- [x] 3. Run two consumer projects outside the checkout including tool-free wheel installation (Windows installed native index and CPython 3.12 two-module wheels; other hosts remain task 4).
- [ ] 4. Integrate required new gates into the existing same-revision four-host release evidence.
- [ ] 5. Prepare a separate human pilot protocol and record actual user observations without fabricating results.
- [x] Validate this change and repository OpenSpec strictly (51/51).

Native/Python workflows are configured under the same required gate names;
new-candidate remote execution is pending. The pilot protocol is prepared in
`docs/daily-development-pilot.md`; no invitations or user observations are
claimed. Local delivery assembly does not satisfy either external gate.
