# Tests, performance and Windows acceptance

Tests require **real executed evidence** with commands, outputs, environment, timestamps, PR/revision and failing cases. Planning-only schema checks do not qualify as application verification.

## Automated cross-platform
- Unit: model IDs, target resolution and precedence, FTS/fuzzy ranking, profiles, status/provenance, launch-event idempotency, SQLite migrations.
- Rust integration: handler abstraction and mock native Shell, wrong/absent path, network timeout, concurrency, rollback.
- TypeScript: Dashboard grid drag/resize/persist, Search keyboard focus/hotkey conflict, Dock profile switching, Explorer CRUD, accessibility and themes.
- Import/export: valid schema and round-trip, malformed ZIP, traversal, excessive compression, duplicated IDs/targets, merge/replace errors, no action execution, rollback and recovery.
- Performance with reproducible fixtures: 10,000 index results, bounded async inspection, startup and memory targets tracked with actual hardware.
- Security: no arbitrary external URL handlers; scripts disabled by default; secrets absent from reports.

## Windows 11 x64 acceptance (required before V1)
- W-001: open xlsx with default Excel handler; W-002: open PDF with default reader; W-003: open HTTPS in default browser.
- W-004: open folders in Explorer and Windows .lnk; W-005: missing path/unavailable UNC shows failure without UI freeze.
- W-006: global hotkey works/fails gracefully if taken; W-007: tray/window show-hide-single process lifecycle.
- W-008: Dock alignment/autohide/secondary monitor; W-009: 100%,150%,200% scaling.
- W-010: grid drag/resize persisted across restart; W-011: profile export/import round-trip on fresh user account.
- W-012: search, tagging, analytics and event once semantics; W-013: offline use/no external network required.
- W-014: portable ZIP freshly downloaded, extracted and run as standard Windows non-admin user with WebView2 runtime documented.

## Gate
No Windows real test environment = release BLOCKED. Include evidence paths and logs. No untested EXE labeled V1 ready. See docs/NOTIFICATIONS_AND_RELEASE_GATES.md.
