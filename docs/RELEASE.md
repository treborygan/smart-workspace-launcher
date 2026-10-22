# Reproducible Windows release requirements

Only create a final release after actual source, unit/integration tests, native Windows 11 x64 tests, and security review. Build self-contained portable app where feasible; accurately document Microsoft WebView2 runtime and permitted installation model. Normal user rights, offline launch, default Shell file associations, global hotkey/tray/dock, DPI and multiple monitors must be tested.

Required deliverables: GitHub tag v1.0.0 (then versioned followups), downloadable portable Windows ZIP (not just source archive), SHA256 checksum, known limitations, manifest of dependencies/SBOM as appropriate, reproducible build commands, machine/OS evidence, and fresh-download smoke evidence.

Use reproducible local PowerShell, approved interactive Windows 11 testing, and **explicitly approved GitHub Actions Windows x64 hosted CI/preview builds**. GitHub-hosted Windows Server CI can establish build/test evidence and supply downloadable unsigned preview artifacts, **not the W-001..W-014 Windows 11 qualification or a V1 release**. If native Windows 11 acceptance or signing is absent, keep release BLOCKED and disclose unsigned builds. Publish V1-ready alert only after the full documented gate passes; continue V1.1, V1.2 and V2. See `docs/CI_WINDOWS.md`.

## Current candidate and evidence (2026-10-22)

The first hosted Windows x64 **unsigned preview** (`v0.1.0-preview.1`) exists from [build run 39723525976](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976). Download the [portable preview + checksum](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402740774) or [NSIS installer](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402700842). The inner portable ZIP SHA256 is `c209f2c27382f1c77215b0a4344515114124258117598373da78f8d727197f4c`. Evidence: [successful Windows CI](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525925) and [successful build](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976).

**This is not a final GitHub Release.** CI executed on hosted Windows Server 2025 x64. The required interactive Windows 11 x64 acceptance cases **W-001–W-014 are NOT_RUN/BLOCKED**. Record standard-user, default-handler, hotkey/tray, Dock/multi-monitor/DPI, offline/profile, fresh-download/first-run evidence before publication. Do not turn `releases/v1/readiness.json` to PASS before all release gates pass.

GitHub Actions artifacts have a 14-day configured retention period. A permanent `v1.0.0` release should only be published after native acceptance, with the appropriate signed or unsigned disclosure, WebView2 prerequisites, permanent asset link, checksum, known limitations and fresh-download smoke evidence.
