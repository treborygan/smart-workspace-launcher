# Reproducible Windows release requirements

Only create a final release after actual source, unit/integration tests, native Windows 11 x64 tests, and security review. Build self-contained portable app where feasible; accurately document Microsoft WebView2 runtime and permitted installation model. Normal user rights, offline launch, default Shell file associations, global hotkey/tray/dock, DPI and multiple monitors must be tested.

Required deliverables: GitHub tag v1.0.0 (then versioned followups), downloadable portable Windows ZIP (not just source archive), SHA256 checksum, known limitations, manifest of dependencies/SBOM as appropriate, reproducible build commands, machine/OS evidence, and fresh-download smoke evidence.

Use local PowerShell or approved interactive machine automation; no GitHub Actions. If Windows runner or signing is absent, mark blocked/tested unsigned disclosure instead of claiming verification or bypassing corporate policy. Publish V1-ready alert with real asset link and continue V1.1, V1.2, V2.
