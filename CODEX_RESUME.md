# Resume Smart Workspace Launcher implementation

## Checkpoint — 2026-10-22 02:40 UTC

Repository: `treborygan/smart-workspace-launcher`. Canonical state: `backlog/AGENT_STATUS.json` and `backlog/STATUS.md`. Coordinator worktree `/workspace/sw020-core`, branch `feature/sw-021-permissioned-extensions`, integrated code head `6073856ad3f150f61970d8f11fb1a9967ba2cf2c` (includes local status checkpoint commit to follow). SW-021 implementation branches are based on frozen contract `d7d33cf`; Core commit `01d16f7`, UI commit `fa89c60`, both independently reviewed. Current host is Debian 13 x86_64, not Windows.

## SW-021 current scope and next work

Issue #21 is NOT COMPLETE. The current implementation is the validated manifest registry and stored-grant policy foundation only. It adds migration v12; Rust typed list/register/grant/enable/remove commands with strict ID/name/version/capability/unknown-field/4 KiB validation; and an accessible UI for manual metadata registration, requested/granted capabilities, explicit enablement, and confirmed removal. The UI clearly says the app does not load or execute third-party code and saved grants have no runtime effect. Independent QA passed contract, Core (after fixing invalid hyphenated test ID), and UI.

Next executable work must implement a separately reviewed sandbox runtime and permission-enforced host API before SW-021 can be declared done or dependent SW-022 can start. Maintain no arbitrary native code/script, filesystem, process, network, launch, shell, or secret access. Freeze an exact module/package ABI, resource/fuel bounds, no-WASI policy, permission data surface and output validation before delegating Core work. Current `wasmi` 2.0.0 was checked against crates.io and declares Rust 1.86 minimum; no dependency has been added yet. No external module has been run.

## Integrated checks and blockers

On integrated SW-021 code head `6073856`:

- `npm run typecheck` — PASS.
- `npm test -- --run` — PASS, 17 files / 132 tests.
- `npm run frontend:build` — PASS with existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS.
- `git diff --check` — PASS.
- `python scripts/verify_handoff.py` — PASS schema/fixture consistency only; no compiled-app evidence.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_registry --lib` — BLOCKED before project compile because `glib-2.0.pc` / GLib >=2.70 dev files are absent (exit 101); Rust regressions authored but NOT_RUN.
- `python scripts/check_release_gate.py` — BLOCKED, `NOT READY: BLOCKED` (exit 2).

No Windows 11 x64, PowerShell, MinGW, Windows Shell, packaging, or W-001..W-014 acceptance is available. V1/SW-012 remains BLOCKED. Do not claim a working EXE, release, or READY notice. No GitHub Actions.

## PR review tracking and feature chain

At 2026-10-22 02:33 UTC, review and inline-thread APIs were empty for PR #38 (SW-014, head `fb22dff5201418f53d0c8dfa9c1f5a463d0a365c`) and PR #41 (SW-015, head `c4c8f4726677855ed7074f95f4e0853aa3488494`). Static QA found no blocker; PR #41 has a non-blocking shared DB lock duration concern for large CSV exports. Do not merge stacked changes ahead of review/dependencies.

PR #46 (SW-020) is open draft at `3a12ab7f4a2250acd38b3f9d11699eccc3681438`, based on PR #45. SW-020 implementation/QA complete; Rust tests blocked by GLib and Windows GNU check by MinGW. Draft PR #47 is open at `7614a31ece437bb40c91d05a62259411ba437389`, stacked on `feature/sw-020-device-paths` / PR #46. GitHub currently reports #47 not mergeable because its base chain is unmerged; do not merge out of dependency order. No PR should close issue #21 while sandbox execution is absent.

## Continue

1. Commit/push canonical status and resume checkpoint after recording PR #47.
2. Freeze the sandbox runtime contract, then implement and test that SW-021 continuation. Keep issue open until the extension call path and permissions are real and tested.
3. Continue SW-022..SW-025 in dependency order only after their prerequisites; keep Windows V1 gate blocked pending real acceptance.
