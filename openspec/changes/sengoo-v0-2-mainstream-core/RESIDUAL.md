# Residual after 2026-07-16 v0.2 mainstream-core archive

Honest residual items that are **not** claimed complete by this archive:

1. **`http-production-serving` (open owner)**  
   Handlers, keep-alive, response streaming, and TLS server remain unproven.
   Matrix row for HTTP server dynamic serving records these as residual.

2. **M0 baseline multi-host Actions**  
   Local gates for baseline reconciliation are green; four-host Actions evidence
   for the baseline SHA remains remote residual (PR Actions after push).

3. **Two consecutive v0.2 RC installed matrices**  
   Fixtures and policy are in-tree (`examples/compat/v0.1.0-rc.1`,
   `v0.2.0-rc.1`). Full four-host consecutive RC evidence is remote residual.

4. **Full sanitizer/fuzz/performance matrix on one SHA**  
   Focused M1–M4 gates were run locally; full remote matrix is Actions residual.

Do not cite these residuals as Supported without new evidence.
