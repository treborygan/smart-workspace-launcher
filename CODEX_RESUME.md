# Resume Smart Workspace Launcher implementation

## Current checkpoint — 2026-10-22 03:13 UTC

Repository: `treborygan/smart-workspace-launcher`; worktree `/workspace/sw020-core`; branch `feature/sw-021-permissioned-extensions`; HEAD `84a9b74c7a61faa13650a8ee61690d1a7062c4a2` (pushed). Canonical status is `backlog/AGENT_STATUS.json` and `backlog/STATUS.md`. Current host is Debian 13 x86_64, not a Windows acceptance environment. No PowerShell, MinGW, WebView2 or real Windows 11 x64 is available. V1/SW-012 is BLOCKED.

## PR review tracking

- PR #38 / SW-014: open against `main`; latest GitHub review + inline-thread API refresh 2026-10-22 03:13 UTC returned no reviews or threads. Prior independent static QA found no actionable blocker.
- PR #41 / SW-015: open stacked on #38; same API refresh returned no reviews or threads. Prior independent static QA found no actionable blocker; non-blocking concern: shared DB mutex may be held during long CSV export.
- PR #47 / SW-021: open draft at current head `84a9b74`, stacked on PR #46 / SW-020. GitHub review/thread APIs returned empty. PR body has current scope, evidence and blockers.

## SW-021 gate

The runtime, package checks and UI are integrated and independent contract/Core/UI/security reviews pass. Frontend typecheck, 17 files / 137 tests, production build, Rust format, Cargo metadata, handoff check and diff check passed. The Cargo extension tests remain NOT_RUN/BLOCKED before application compilation because GLib/GObject >=2.70 development files are missing. A read-only investigation confirmed that `--no-default-features` still reaches Tauri GTK system dependencies; testing these app-specific DB/command/runtime paths here would require a deliberate Core/Tauri split, not a flag tweak. Windows GNU check is blocked by missing MinGW. Do not mark SW-021 complete or start dependent SW-022 until focused application Rust tests pass on a capable host.

SW-021 runtime hardening includes opaque one-use five-minute path/digest-bound preview tokens, hostile ZIP/output/pointer/profile coverage, byte/count truncation, Wasm structural caps, explicit compile/run and no SQLite mutex during parsing or guest execution. PR #47 documents detailed test commands and results.

## Next actions

1. Provide/use a Linux host with GLib/GObject >=2.70 and run `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_ --lib`; record exact result.
2. If it passes, complete SW-021 status and proceed with SW-022 after SW-020/SW-021 dependency checks. Keep each feature stacked and reviewed.
3. Continue refreshing PR #38/#41 external reviews; do not merge the stack out of order.
4. Keep Windows V1 blocked until real Windows 11 x64 acceptance; no EXE/READY claim or release without it.

No GitHub Actions were added. Latest review refresh used the connected GitHub review and thread APIs; local `gh pr view` GraphQL and `gh api` REST calls returned `Forbidden` in this runtime.
