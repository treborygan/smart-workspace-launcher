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

## GitHub Actions Windows CI
Approved automated test and preview-build jobs run on **GitHub-hosted Windows Server 2025 x64**, not consumer Windows 11. They exercise Rust, frontend, Python/contract tests and native packaging; evidence is retained as run artifacts. W-001..W-014 still require separate real Windows 11 x64 tests (including default Excel/PDF app handlers, focus/hotkey/tray, multi-monitor/DPI, non-admin and fresh-download usage). Mark any unperformed case BLOCKED/NOT_RUN. See `docs/CI_WINDOWS.md`.

## Gate
No verified real Windows 11 acceptance = release BLOCKED, even if GitHub Actions pass. Include evidence paths and logs. No untested EXE labeled V1 ready. See docs/NOTIFICATIONS_AND_RELEASE_GATES.md.

## Recorded hosted-Windows automated verification (2026-10-22)

- On `main` SHA `98b1432`, [GitHub Actions Windows CI run 39723525925](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525925) **PASS**: 73 frontend tests in 11 files, 63 Rust tests (0 failures), Python/contract and handoff checks, TypeScript typecheck, and frontend production build.
- [Windows preview build run 39723525976](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976) **PASS**: mandatory verification, native Tauri `.exe` compilation, NSIS installer build, portable ZIP creation, archive-contents check, SHA256 verification and artifact upload.
- This is **Windows Server 2025 x64 hosted CI**, **not** a Windows 11 standard-user interactive session. There is no verified execution evidence for W-001, W-002, W-003, W-004, W-005, W-006, W-007, W-008, W-009, W-010, W-011, W-012, W-013 or W-014 on an actual Windows 11 x64 desktop. All remain **NOT_RUN / BLOCKED** for release qualification.
- Download instructions, package hash, CI evidence links and expiry are in [CI_WINDOWS.md](CI_WINDOWS.md). **Never translate unit coverage into a GUI acceptance PASS.**
