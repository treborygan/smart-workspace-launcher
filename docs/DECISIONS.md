# Approved product decisions (binding)

- Windows 11 x64, Tauri 2 + React + TypeScript + Rust + local SQLite.
- Hybrid surfaces share resources, profiles, usage database: editable resizable Dashboard grid; global keyboard Quick Launch; optional floating Dock, plus Explorer/Inspector.
- Profile persistence local; manual versioned .swprofile import/export; no mandatory sync; theme supports both light/dark and custom colors.
- V1 balanced and complete, not a prototype; must create tested Windows portable release. Continue V1.1, V1.2, V2 advanced full roadmap after V1 alert.
- GPT-6 Luna Medium root/Core/UI/QA/Release; Luna Low for narrow research. Three active subagents maximum plus coordinator, with actual runtime validation. Need explicit spawning.
- GitHub is source of truth, user repository treborygan/smart-workspace-launcher. **2026-10-22 explicit exception:** GitHub Actions allowed for Windows-hosted CI, preview builds and evidence artifacts. No automatic certified V1 release, no corporate data, and separate real Windows 11 acceptance still required.
- V1 event-based launcher usage; do not claim all external Windows file open history.
- Default OS handlers; imported commands disabled. Windows release requires direct real standard-user testing.
- Naming provisional: do not lock app IDs to a final brand. Working name Smart Workspace Launcher.
- When details conflict, prioritize newer explicit user instructions and document the tradeoff.
