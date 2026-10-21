# Restart Codex development from GitHub state

## Current checkpoint — 2026-10-21

- Repository: `treborygan/smart-workspace-launcher`; workspace `/workspace/smart-workspace-launcher`; coordinator GPT-6 Luna Medium.
- Main is `4873414e1fb1c2e096fedd3d7b44b576831434f4`. Merged PRs: #26 SW-001..004, #27 SW-005, #29 SW-009, #30 SW-007 backend, #33 SW-007 UI. PR #32 corrective resource/profile atomicity fix was included through #26; PR #31 was closed as superseded by #33.
- Draft PR #34 is active for SW-008 on `feature/sw-008-profile-archive`. The QA-approved backend commit is `adaab02`; the UI was cherry-picked as `6a02e59`, with current status/docs update pending. It implements bounded profile archive export, strict ZIP validation, preview, transactional duplicate/merge/replace, ID remapping and no-clobber finalization. Independent QA found two blockers: preview and commit reread the path separately, so commit must be bound to the previewed digest or immutable staged content; Merge can silently reuse a colliding resource ID with differing local content, so preview must expose conflict snapshots and define behavior. Core fixed both backend findings at `adaab02` and added Tauri dialog plugin support. QA re-reviewed and approved the backend changes. UI integration now provides native dialogs, preview/diff, strategy-aware collision resolution, explicit confirmation, and hash/token commit binding. QA reviewed backend and UI with no blocker.
- Linux frontend checks previously passed on #33: `npm ci`, `npm run typecheck`, `npm test` (4 files/31 tests), `npm run frontend:build`, `git diff --check`. PR #34 Rust formatting and diff checks passed; authored Rust tests are not executed because Debian lacks `glib-2.0 >= 2.70`. Windows GNU cross-check cannot run because `x86_64-w64-mingw32-dlltool` is missing.
- Environment: Debian 13 x86_64, Node 24.19.0/npm 11.9.0, Rust 1.99.1 stable. No PowerShell or real Windows 11 x64 host. Native Windows launch, Shell, hotkey, filesystem, DPI and multi-monitor behavior is unverified. `python scripts/verify_handoff.py` passes and confirms no compiled app/Windows runtime tests. `python scripts/check_release_gate.py` returns BLOCKED (exit 2).
- `.github/workflows/` remains absent. No EXE, portable distribution, Release upload or V1 READY notice exists. V1 is BLOCKED pending actual Windows 11 acceptance.

## Next executable work

1. Commit durable status docs with the integrated SW-008 UI and push PR #34; update its description with exact checks/results and blockers. Backend/UI QA approval is complete.
2. Complete SW-010 and SW-011 integration/security hardening.
3. Obtain a real Windows 11 x64 host and run `docs/TEST_AND_ACCEPTANCE.md`; do not claim a Windows build/runtime or publish READY without evidence.
4. Continue SW-013..SW-025 after V1 gates; V1 is a milestone, not project completion.

Coordinator owns integration and durable status. Maintain `backlog/AGENT_STATUS.json` and `backlog/STATUS.md`, use feature branches/PRs, keep GitHub Actions absent, and record exact test results and unresolved risks.
