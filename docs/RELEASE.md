# Reproducible Windows release requirements

Only create a final release after actual source, unit/integration tests, native Windows 11 x64 tests, and security review. Build self-contained portable app where feasible; accurately document Microsoft WebView2 runtime and permitted installation model. Normal user rights, offline launch, default Shell file associations, global hotkey/tray/dock, DPI and multiple monitors must be tested.

Required deliverables: GitHub tag v1.0.0 (then versioned followups), downloadable portable Windows ZIP (not just source archive), SHA256 checksum, known limitations, manifest of dependencies/SBOM as appropriate, reproducible build commands, machine/OS evidence, and fresh-download smoke evidence.

Use reproducible local PowerShell, approved interactive Windows 11 testing, and **explicitly approved GitHub Actions Windows x64 hosted CI/preview builds**. GitHub-hosted Windows Server CI can establish build/test evidence and supply downloadable unsigned preview artifacts, **not the W-001..W-014 Windows 11 qualification or a V1 release**. If native Windows 11 acceptance or signing is absent, keep release BLOCKED and disclose unsigned builds. Publish V1-ready alert only after the full documented gate passes; continue V1.1, V1.2 and V2. See `docs/CI_WINDOWS.md`.
