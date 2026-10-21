# Current implementation status

Updated 2026-10-21 19:28 UTC. Coordinator: `feature/sw-001-bootstrap`. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). Selected issue: [SW-001 #1](https://github.com/treborygan/smart-workspace-launcher/issues/1). Draft integration PR: [#26](https://github.com/treborygan/smart-workspace-launcher/pull/26); review and additional implementation are pending.

| Work item | State | Evidence / remaining work |
|---|---|---|
| SW-001 bootstrap | IN PROGRESS | Tauri 2 + React/TS package scaffolding, pinned npm dependencies, Vite/Tauri config, shared typed IPC contract, local PowerShell scripts. Needs reviewed PR and integration verification. |
| SW-002 database/domain | PARTIAL | Initial SQLite WAL schema and resource/profile Tauri commands in commit `745a0de`; lockfile generated. Rust tests need a Linux host with GTK/WebKit system libraries or Windows. DTO and domain behavior follow-up active. |
| SW-003 resource/launch/history | PARTIAL | CRUD/search, HTTPS/absolute Windows path validation, ShellExecuteW handoff and request-ID event reservation in `745a0de`. Analytics DTO, OS launch tests, application target semantics and full command coverage remain incomplete. |
| SW-004 profiles/layouts | PARTIAL | Initial profile create/list/switch. Profile memberships are optional on create. Layout persistence and complete profile export/import are not implemented. |
| SW-006/SW-010 UI | PARTIAL | React dashboard, search, Explorer, Inspector, local browser preview and typed IPC adapter in `ef99e21` + `fabf03b`. Native layout persistence, Dock, true OS global hotkey, import/export controls and metadata remain. |
| SW-008 import/export QA | QA HARNESS ONLY | `scripts/qa/profile_archive.py`; 7 tests cover ZIP limits, paths, executable content, schemas, URL schemes, layout references and asset hashes. The production Rust importer and transactional collision handling are not implemented. |
| SW-011 integration | NOT STARTED | Depends on complete resource/profile/import/hotkey/UI behavior and security review. |
| SW-012 Windows acceptance/release | BLOCKED | No PowerShell or Windows 11 x64 execution host. No EXE or portable artifact produced. `releases/v1/readiness.json` remains BLOCKED. |
| SW-013..SW-025 | NOT STARTED | V1.1, V1.2 and V2.0 remain in the executable queue after V1 gates. |

## Executed checks

Environment: Debian 13 x86_64, Node 24.19.0/npm 11.9.0, Rust 1.99.1 stable. No PowerShell or Windows runner is available.

- `python scripts/verify_handoff.py` — PASS (handoff/schema fixture consistency only).
- `python -m unittest discover -s tests -v` — PASS, 7 tests (Python ZIP/schema QA helper only; not production importer tests).
- `npm ci` — PASS (88 packages installed from checked-in lockfile).
- `npm run lint` / `npm run typecheck` — PASS.
- `npm test` — PASS, 2 UI tests.
- `npm run frontend:build` — PASS.
- `npm run tauri -- info` — PASS config inspection; reports missing Linux `webkit2gtk-4.1` and `rsvg2` development prerequisites.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` — BLOCKED by absent `glib-2.0`/GTK development packages.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check` — initial check reported Rust formatting differences; Core follow-up will format and rerun.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu` — BLOCKED by absent `x86_64-w64-mingw32-gcc`; not Windows verification.
- `.github/workflows/` remains absent. No GitHub Actions were added.

## Risks and next executable tasks

1. Integrate Core's active follow-up, resolve contract mismatches and run Rust unit/integration tests on a provisioned native build host.
2. Implement OS-global Quick Launch hotkey, persistent layout/Dock, background bounded metadata, real profile export/import preview/transaction/rollback, and exact-once launch analytics.
3. Add Windows 11 local verification evidence, including default Shell handlers, tray/hotkey, Dock, DPI/multimonitor, offline and standard-user portable execution. Until then V1 is BLOCKED; no ready notice or release upload.
4. Open a draft GitHub PR with exact commands/results and remaining risks; merge only after review and acceptance gates.

Canonical agent state: [`AGENT_STATUS.json`](AGENT_STATUS.json). Resumption notes: [`CODEX_RESUME.md`](../CODEX_RESUME.md).
