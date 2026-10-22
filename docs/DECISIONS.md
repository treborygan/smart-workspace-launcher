# Approved product decisions (binding)

- Windows 11 x64, Tauri 2 + React + TypeScript + Rust + local SQLite.
- Hybrid surfaces share resources, profiles, usage database: editable resizable Dashboard grid; global keyboard Quick Launch; optional floating Dock, plus Explorer/Inspector.
- Profile persistence local; manual versioned .swprofile import/export; no mandatory sync; theme supports both light/dark and custom colors.
- V1 balanced and complete, not a prototype; must create tested Windows portable release. Continue V1.1, V1.2, V2 advanced full roadmap after V1 alert.
- GPT-6 Luna Medium root/Core/UI/QA/Release; Luna Low for narrow research. Three active subagents maximum plus coordinator, with actual runtime validation. Need explicit spawning.
- GitHub is source of truth, user repository treborygan/smart-workspace-launcher. No GitHub Actions. No confidential employer data.
- V1 event-based launcher usage; do not claim all external Windows file open history.
- Default OS handlers; imported commands disabled. Windows release requires direct real standard-user testing.
- Naming provisional: do not lock app IDs to a final brand. Working name Smart Workspace Launcher.
- When details conflict, prioritize newer explicit user instructions and document the tradeoff.

## SW-017 interpretation — duplicate and relocation assistant

The roadmap says “duplicate/missing relocation” but does not define whether relocation physically moves user files. To keep the assistant local, reversible and bounded, duplication creates a new profile resource and new target UUIDs while leaving filesystem contents untouched. Relocation is an explicit preview-and-confirm mapping of stored file/folder target path prefixes; it never copies/moves/deletes files or scans directories. Commit is bound to the exact preview and revalidates source URIs/ownership transactionally. Non-file/folder targets are excluded.
