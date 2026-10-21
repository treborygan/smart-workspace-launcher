# Restart Codex development from GitHub state

## Current checkpoint (2026-10-21)

- Repository: `treborygan/smart-workspace-launcher`; verified environment is Debian 13 x86_64, Node 24.19.0/npm 11.9.0, Rust 1.99.1 stable. Loaded coordinator is GPT-6 Luna Medium. The 3-agent Core/UI/QA workflow was activated as documented.
- Coordinator branch: `feature/sw-005-quick-launch`, current commit `bb54896`. Selected issue SW-005 (#5).
- Draft PR #27: https://github.com/treborygan/smart-workspace-launcher/pull/27, based on draft PR #26 (`feature/sw-001-bootstrap`). PR #27 body has scope, exact verification, QA status and blockers. Neither PR is merged. PR #27 attachment to this Codex chat failed with an app service connection error; the PR itself was successfully updated.
- SW-005 integrated commits include `1174004` (native hotkey), `67670ec` (Quick Launch window config), `7ff39e7` + `443e5bc` (UI), `887375c` (typed configurable-hotkey contract), `32fbd62` (UI/settings), `2cd7a94` (Core profile hotkeys/fuzzy ranking), `c2afde4` (synthetic Linux preview), `994dec5` (per-profile draft bug fix) and `805ae07` (bounded search work).
- QA reviewed integrated shortcut conflict handling, profile settings draft behavior, launch failure UI and bounded fuzzy search. Initial profile-draft bug was fixed and re-reviewed; no remaining blocker in SW-005 review scope.
- Checks on this revision: `npm run lint` PASS; `npm test` PASS (15 frontend tests); `npm run frontend:build` PASS; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check` PASS; `git diff --check` PASS; Python archive QA harness PASS (11 tests); `python scripts/verify_handoff.py` PASS. `cargo check --target x86_64-pc-windows-gnu --tests` PASS only as a Rust type-check using Linux `cc`, fake `windres` and temporary placeholder icon; it produced no Windows binary and ran no tests. Native Rust tests are BLOCKED before execution because Linux lacks `glib-2.0 >= 2.70`/GTK development libraries.
- No PowerShell or real Windows 11 host exists. Windows global-hotkey/Shell behavior is unverified. `python scripts/check_release_gate.py` is BLOCKED as expected. V1 readiness is BLOCKED; no EXE, portable package, Release upload or READY notice exists. `.github/workflows/` remains absent.
- Synthetic browser screenshot: `artifacts/screenshots/quick-launch-linux-preview.png`; it is not a native desktop capture.

## Next executable work

1. Finish active SW-007 on `feature/sw-007-metadata`: bounded background metadata cache/health/provenance and local analytics summaries.
2. Finish active SW-009 on `feature/sw-009-dock`: separate per-profile Dock window, layout and shared state; Windows alignment/autohide remains NOT_RUN.
3. Implement SW-008 production profile export/import with schema/path/size checks, preview/diffs, duplicate/merge/replace, safe transaction rollback and no automatic script execution.
4. Finish SW-010 and SW-011 integration/security hardening.
5. Obtain a real Windows 11 x64 host for all W-001..W-014 acceptance cases in `docs/TEST_AND_ACCEPTANCE.md`. Keep SW-012 BLOCKED until evidence exists; do not claim an EXE or send a V1 READY notice.
6. Continue SW-013..SW-025 after V1 gates; V1 is not project completion.

Before continuing, inspect `git status`, fetch/reconcile PR #26/#27 and branches, read `backlog/AGENT_STATUS.json`, `backlog/STATUS.md`, `docs/MULTI_AGENT_WORKFLOW.md`, `docs/NOTIFICATIONS_AND_RELEASE_GATES.md` and `backlog/ROADMAP.md`. Keep the coordinator as sole integrator, cap active subagents at three, preserve feature PRs and do not add GitHub Actions without explicit approval.
