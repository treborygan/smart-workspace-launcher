# Architecture

## Runtime topology
One Tauri 2 process with three native UI windows: Dashboard normal/resizable, Quick Launch borderless and transient with global hotkey/focus management, Dock optional/positionable and per-profile. System tray controls window lifetime. Shared React/TypeScript state observes typed Rust event bus, no independent databases per window.

Frontend: React, TypeScript, Tailwind/design tokens, accessible UI primitives, TanStack Table for Explorer, dnd-kit or equivalent grid resizing/reorder, chart components. Backend: Rust Tauri commands, native Shell integration, SQLite WAL on **local disk**, migrations, caching/indexing, validated import/export.

## Backend service boundaries
- ResourceService: immutable UUID identity, CRUD, targets, categories/tags/memberships, immutable typed DTO.
- LaunchService: request ID -> target resolution -> allowed scheme/command -> ShellExecuteEx on Windows -> exactly-once event -> UI notification. Handoff success is not proof target app loaded.
- SearchService: SQLite FTS5 plus ranking/fuzzy/recency and indexed aliases. Search never reads filesystem synchronously on hot path.
- ProfileService: local independent profile settings, layouts, memberships and safe profile switching.
- DevicePathService: random identity in per-user OS app-local settings, approved local path-prefix rules, exact preview/commit, launch-only substitution; never rewrites canonical targets or includes local mappings in profile archives.
- MetadataService: bounded async inspection, cached size/timestamps/accessibility/provenance, cancellation and timeouts.
- AnalyticsService: immutable launch events, timestamps in UTC, provenance, aggregates by profile/source/time.
- ImportExportService: strict versioned ZIP+JSON schema, absolute compressed/uncompressed byte limits, no zip slip, disallow scripts, preview and atomic apply; backups/rollback.
- WindowService: native hotkey/tray/windows, monitor and DPI behavior; no uncontrolled duplicate process.
- ExtensionHost is future-only, disabled in V1.

## Native launching
Use Windows Shell default handlers via ShellExecuteEx or equivalent proven native API. For executables arguments are strongly typed rather than shell-concatenated. Limit URL protocols to approved schemes (default HTTPS). Never automatically execute imported action definitions.

## Typed IPC
Front/back command set: resource.list/create/update/delete/search/inspect/launch, profile.list/create/switch/export/importPreview/importCommit, layout.get/save, analytics.query, dock.get/update. Use generated TS types matching Rust DTO and schema validation. IDs are immutable UUIDs. A typed launch event records requested/handed_off/failed once per request UUID.

## Storage
SQLite local DB with migrations, explicit transactions and safe backups. Do not synchronize a live SQLite WAL DB across computers. Portable mode can use data directory alongside EXE; installed mode per-user local AppData. Profile exports have canonical schema version. Use generated fake data only in repo.

## Agent boundaries
Freeze entity/IPC schema before concurrent Core and UI implementation. Root owns shared contracts and package manager/lockfiles. Core and UI feature branches do not edit shared files concurrently. See docs/MULTI_AGENT_WORKFLOW.md.
