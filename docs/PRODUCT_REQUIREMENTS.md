# Product requirements — Smart Workspace Launcher

## Locked V1 choices
Windows 11 x64 portable-first Tauri 2/React/TypeScript/Rust/SQLite app. **Hybrid experience**: Dashboard with resizable drag-and-drop grid, global hotkey Quick Launch, optional floating Dock, Resource Explorer/Inspector. Light and dark themes with custom colors. Multiple local profiles, manual transactional export/import. No cloud sync or remote-control requirement in V1.

## Required functional behavior
- Resource registry for local files, folders, EXEs, .lnk/.url shortcuts, HTTPS URLs, network paths and logical workspaces. Every resource has immutable UUID; multiple device/target alternatives allowed.
- Windows Shell default associations for documents/URLs, Explorer for folders. Structured safe executable arguments and working directory. Robust missing-target and unavailable-network errors.
- CRUD/edit/rename/duplicate/category/tag/alias/favorite/search/sort/filter for resources. Bulk drag-and-drop import and editable Resource Explorer table.
- Quick Launch available by configurable conflict-checked global hotkey, accessible keyboard navigation, fuzzy ranked local searches; no network required.
- Dashboard supports draggable/resizable persistent responsive grid, group cards, favorites and theme customization. Separate optional floating Dock allows per-profile pins, edge choice and monitor positioning.
- Profiles persist layout, memberships, theme, hotkeys, pins and settings; import/export uses versioned .swprofile ZIP with safe untrusted-data validation, preview/diff, collision policy, and atomic transaction.
- Detailed stats include file metadata (size, created/modified timestamps, accessibility) with provenance/cache status; launch count, last launched, per-source event history. Do not imply observed third-party file use or actual successful document opening.
- Optional local start-at-login setting, tray availability and configurable close/hide behavior if permitted by operating-system policy.

## Nonfunctional requirements
Local-first; non-admin operation; functional offline; no telemetry; no source/work files leaving machine; clear work/personal separation. Bounded async network/file checks and timeouts. Reproducible dependency locks. Testable targets: cold startup preferably under 2s, Quick Launch warm under 150ms, 10k search under 50ms, idle memory preferably under 150 MB; these are goals, not verified guarantees.

## Release definitions
V1.0 SW-001–012: everything above usable, tested and downloadable on Windows.
V1.1 SW-013–016: dynamic shortcuts, smart collections, workspace sequences, richer history and relationships.
V1.2 SW-017–020: duplicate/missing relocation, imports, reusable widgets, device-specific target mapping.
V2.0 SW-021–025: opt-in permissioned extension system, optional data sync, authenticated remote and AI adapters, hardening and native final release.
Do not start optional integrations or scripts without approval. See issue catalog and windows acceptance requirements.
