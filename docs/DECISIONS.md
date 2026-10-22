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

## SW-018 interpretation — Start Menu, directory and bookmark import

Import is user-selected, read-only discovery followed by a preview and explicit transactional commit. It does not scan drives or network shares automatically and never launches a discovered item. A directory scan is bounded to depth 4 and 1,000 entries, uses a capability-opened root with no-follow child/file opens, rejects observed symlinks/reparse points, and discovers only folders, regular files, and `.url` shortcuts containing safe HTTPS targets. The selected root is initially opened from its validated ambient path, and Windows `.lnk` metadata is read with `IPersistFile::Load` by path, leaving a narrow same-user replacement race for those operations; this user-initiated scanner is not an isolation boundary. A Windows Start Menu folder uses the same bounded scanner; Shell Link metadata is never resolved or invoked. Bookmark import accepts Netscape-format HTML and HTTPS links only (4 MiB input maximum). Other protocols, scripts, command arguments and embedded actions are discarded.

Preview labels each row with source and provenance and marks same-target resources already in the destination profile. Users select rows and choose `skip existing` or `create another resource`; no implicit metadata merging occurs. Commit receives the source descriptor, selection keys, and preview digest; it repeats the bounded scan and rejects stale/mutated source, candidate, or duplicate-match state before a single SQLite transaction creates selected resources, targets, and profile memberships. A batch failure rolls back completely. No fixed preview TTL is imposed; a preview remains valid only while the recomputed state matches. This scan is not an ongoing watcher or a complete index of the device.
