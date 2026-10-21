# Current implementation status

Updated 2026-10-21 19:44 UTC. Coordinator: `feature/sw-001-bootstrap`. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). Selected issue: [SW-001 #1](https://github.com/treborygan/smart-workspace-launcher/issues/1). Draft integration PR: [#26](https://github.com/treborygan/smart-workspace-launcher/pull/26); SW-001 through SW-004 foundation is integrated; review and further implementation are pending.

| Work item | State | Evidence / remaining work |
|---|---|---|
| SW-001 bootstrap | IN PROGRESS | Tauri 2 + React/TS package scaffolding, pinned npm dependencies, Vite/Tauri config, shared typed IPC contract, local PowerShell scripts. Needs reviewed PR and integration verification. |
| SW-002 database/domain | PARTIAL | SQLite WAL schema, typed DTOs, timestamps, restartable migrations v2/v3, default profile, resource/profile/layout commands in `745a0de`, `458b0d3`, `b9ff722`, `50e8f04`, `a4285cb`. Rust unit tests are authored but cannot run without Linux GTK/GLib development libraries. |
| SW-003 resource/launch/history | PARTIAL | CRUD/search, HTTPS/absolute Windows path validation, ShellExecuteW handoff, request-ID idempotency, aliases/tags/favorites, history and analytics query in `745a0de` + `458b0d3`. Rust tests are authored and cross-target type-checked; they are unexecuted on this host. |
| SW-004 profiles/layouts | PARTIAL | Profile create/list/switch and per-profile dashboard/Dock layout SQLite commands, schema v3, geometry/UUID/membership/size/identity validation, rollback-safe saves and UI profile switching. UI ordering/card widths persist through typed commands; browser preview uses profile-scoped localStorage. Profile export/import remains SW-008. Backend tests are authored and cross-target type-checked but not executed. |
| SW-006/SW-010 UI | PARTIAL | React dashboard, search, Explorer, Inspector, local browser preview and typed IPC adapter in `ef99e21`, `fabf03b` and `bcb89a8`. Native layout persistence is integrated; true OS global hotkey, independent Dock, production import/export controls and bounded metadata remain. |
| SW-008 import/export QA | QA HARNESS ONLY | `scripts/qa/profile_archive.py`; 11 tests cover ZIP limits, paths, executable content, schemas, URL schemes, layout references, compression and asset hashes. The production Rust importer and transactional collision handling are not implemented. |
| SW-011 integration | NOT STARTED | Depends on complete resource/profile/import/hotkey/UI behavior and security review. |
| SW-012 Windows acceptance/release | BLOCKED | No PowerShell or Windows 11 x64 execution host. No EXE or portable artifact produced. `releases/v1/readiness.json` remains BLOCKED. |
| SW-013..SW-025 | NOT STARTED | V1.1, V1.2 and V2.0 remain in the executable queue after V1 gates. |

## Executed checks

Environment: Debian 13 x86_64, Node 24.19.0/npm 11.9.0, Rust 1.99.1 stable. No PowerShell or Windows runner is available.

- `python scripts/verify_handoff.py` — PASS (handoff/schema fixture consistency only).
- `python -m unittest discover -s tests -v` — PASS, 11 tests (Python ZIP/schema QA helper only; not production importer tests).
- `npm ci` — PASS (88 packages installed from checked-in lockfile).
- `npm run lint` / `npm run typecheck` — PASS.
- `npm test` — PASS, 5 UI/API tests.
- `npm run frontend:build` — PASS.
- `npm run tauri -- info` — PASS config inspection; reports missing Linux `webkit2gtk-4.1` and `rsvg2` development prerequisites.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` — BLOCKED by absent `glib-2.0`/GTK development packages.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check` — PASS after Core follow-up.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu --tests` — PASS Rust type-check including tests using host `cc`, fake `windres` and ephemeral placeholder icon; no Windows binary was produced and tests were not run. A direct check without these shims is BLOCKED by missing MinGW C compiler.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` — BLOCKED by missing Debian GTK/GLib development packages; Rust tests remain unexecuted.
- `python scripts/check_release_gate.py` — BLOCKED as expected (exit 2).
- `.github/workflows/` remains absent. No GitHub Actions were added.

## Risks and next executable tasks

1. Execute Rust tests on a provisioned native build host; cross-target type-check now passes with local compiler shims.
2. Implement SW-005 true OS-global Quick Launch hotkey/search, SW-007 bounded metadata, SW-008 production profile export/import preview/transaction/rollback, and SW-009 independent Dock.
3. Add Windows 11 local verification evidence, including default Shell handlers, tray/hotkey, Dock, DPI/multimonitor, offline and standard-user portable execution. Until then V1 is BLOCKED; no ready notice or release upload.
4. Draft PR #26 is open with exact commands/results and remaining risks; merge only after review and acceptance gates.

Canonical agent state: [`AGENT_STATUS.json`](AGENT_STATUS.json). Resumption notes: [`CODEX_RESUME.md`](../CODEX_RESUME.md).

Linux browser preview: [`artifacts/screenshots/dashboard-linux-preview.png`](../artifacts/screenshots/dashboard-linux-preview.png). It displays synthetic UI data and is not a native app or Windows acceptance screenshot.
