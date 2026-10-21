# Restart Codex development from GitHub state

## Current checkpoint — 2026-10-21

- Repository: `treborygan/smart-workspace-launcher`; workspace `/workspace/smart-workspace-launcher`; coordinator GPT-6 Luna Medium. Current main: `51d8fec` (README update after SW-008 merge).
- Merged PRs: #26 SW-001..004, #27 SW-005, #29 SW-009, #30 SW-007 backend, #33 SW-007 UI and #34 SW-008 archive import/export. PR #32 corrective atomicity fix was included through #26; PR #31 was superseded by #33. PR #34 merged as `60fd44f`; QA approved the backend and UI.
- SW-008 provides bounded archive validation/export, preview-bound commit with digest/workspace token, strategy-aware Duplicate/Merge/Replace, collision-safe ID remapping, native file dialogs, transactional rollback and no-clobber export. Frontend checks passed: typecheck, 38 tests, production build, diff check. Rust tests remain authored but not run due missing GLib development files.
- SW-010 is in draft PR #35 from `/workspace/ui-sw010`, branch `feature/sw-010-ui`; code head `773ede2`, QA-approved. Branch includes latest main README sync plus status checkpoint. The implementation addresses the main Explorer/theme gaps; QA found no contract blocker. UI implementation is at `fbf1edf`: Explorer sort/filter/CSV/bulk Favorites/columns, tags/aliases/targets editor, profile-persistent themes via the theme API, and responsive card sizing/order. Follow-up commit `773ede2` adds target priority/arguments/workdir editing, whitespace-prefixed CSV formula protection, and profile switching/system behavior tests; 56 frontend tests pass, QA final review passed at `773ede2`. Bulk delete/duplicate and draggable/resizable column controls remain uncovered.
- SW-011 Core work is in `/workspace/core-sw011`, branch `feature/sw-011-backend-security`, based on main. Core pushed theme API commit `3d76767` and hardening commit `f767844`; QA confirmed `b42ff26` closes the replay-intent finding; residual caveat: replaying a formerly valid explicit target after deactivation returns NotFound, with no duplicate launch. Backend includes Windows path validation, HTTPS credential rejection, launch replay/membership checks, active-profile attribution, profile name bounds and SQLite hardening. UI cherry-picked the theme API as `559e207` into SW-010. SW-011 remains separate; do not merge it before SW-010.
- Environment: Debian 13 x86_64, Node 24.19/npm 11.9, Rust 1.99 stable; no PowerShell or actual Windows 11 x64 host. Linux Rust test compilation is blocked by missing `glib-2.0 >=2.70`; Windows GNU check is blocked by missing MinGW compiler. `python scripts/verify_handoff.py` passes. `python scripts/check_release_gate.py` returns BLOCKED.
- No Windows binary or EXE, portable package, Release upload, or V1 READY notice exists. `.github/workflows/` remains absent. V1 is BLOCKED pending real Windows 11 acceptance.

## Next executable work

1. Finish/review SW-010 UI, then integrate it with the theme contract.
2. Review/rebase/integrate SW-011 backend hardening after SW-010, in dependency order.
3. Run complete Windows 11 x64 acceptance and portable packaging only on a real host; do not claim or release an unverified binary.
4. Continue SW-013..SW-025 after V1 release gates.

Keep `backlog/AGENT_STATUS.json`, `backlog/STATUS.md`, PR descriptions, and `CODEX_RESUME.md` current. Use feature branches and reviewed PRs; no GitHub Actions.
