# Current implementation status

Updated 2026-10-22 03:35 UTC. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). Main contains the foundational implementation through SW-013 (PR #37). Cross-platform advanced work is progressing in stacked feature branches. The current environment is Debian 13 x86_64 and is not a Windows acceptance environment.

## Completed / merged foundations

SW-001..SW-011 are merged through PR #36; SW-013 is merged through PR #37. SW-014 and SW-015 implementations are complete on PRs #38 and #41. SW-016 and SW-017 implementations are complete on draft PRs #42 and #43. SW-018 is implemented and independently QA-reviewed on PR #44.

## Active pull requests and review state

- PR #38 (SW-014) OPEN against `main`, head `fb22dff`; review and inline-thread APIs refreshed 2026-10-22 03:35 UTC: both empty. Independent QA read-only review found no actionable blocker.
- PR #41 (SW-015) OPEN, stacked on #38, head `c4c8f47`; review and inline-thread APIs refreshed 2026-10-22 03:35 UTC: both empty. Independent QA read-only review found no actionable blocker; non-blocking scale concern: CSV export holds the shared DB lock while writing a potentially large unlimited-retention export.
- PR #42 (SW-016) OPEN_DRAFT, stacked on #41/#38. Migration regression assertion is corrected for schema v10 in `d90204f`.
- PR #43 (SW-017) OPEN_DRAFT, stacked on #42. Its branch includes SW-018 contract documents as an explicit base for the next feature.
- PR #44 (SW-018) OPEN_DRAFT, stacked on #43. Implementation is on `feature/sw-018-discovery-import`; code head `1ec5a7a`.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44 at head `9a984e5`; implementation and independent QA are complete. Rust tests are blocked before compilation by missing GLib/GObject; no Windows acceptance.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44, head `9a984e5`; implementation and independent QA are complete. Rust test compilation remains blocked by missing GLib/GObject; no Windows acceptance.
- PR #46 (SW-020) OPEN_DRAFT, base `feature/sw-019-widget-templates`, integrated head `3a12ab7`. Implementation and independent QA are complete; frontend 126 tests/typecheck/build, Rust formatting, diff and handoff checks pass. Rust test compilation is blocked by missing GLib/GObject, Windows GNU check by missing MinGW. V1 remains BLOCKED.
- PR #47 (SW-021) OPEN_DRAFT at current checkpoint `a882998` (implementation code head `d5344b4`), base `feature/sw-020-device-paths` / PR #46. Runtime and UI are integrated. Focused extension tests pass 13/13; scanner tests pass 2/2 on Linux. Full Rust library suite compiles but fails 90 passed / 25 failed; fixture and behavior triage/fixes are active. Independent security QA passed the restored Windows reparse/junction checks; Windows-only junction test remains NOT_RUN. Do not merge out of dependency order.
- PR #39 was closed as superseded without merging. No PR review requests or comments were sent. Do not merge stacked work before review and dependency-order checks.

## SW-020 implementation — issue #20

SW-020 adds random per-user local device identity, device labels, approved-by-construction path-prefix rules, exact preview/transactional commit/delete and launch-time-only mapping. Mappings are not exported with profiles, and copied portable data uses a different local identity on the destination machine. Drive-letter/UNC paths only; mapping is boundary-safe and Unicode case-insensitive, with most-specific prefix precedence. Only active file/folder target URIs are translated in the explicit launch copy; stored paths, arguments, working directories and history/export identity remain untouched. No scanning/copying/moving/automatic launching occurs. UI review fixed delayed-preview divergence and keyboard focus; Core review fixed Unicode comparisons.

## SW-021 contract and implementation — issue #21

SW-021 integrates schema v13 package storage, validated `.swext` review/install, explicit grants and enablement, an explicit-run Wasm sandbox, and the matching UI. Contract, Core, UI and extended security QA passed. Opaque one-use preview tokens, hostile package/output and profile tests, deterministic 64 KiB input/count truncation, Wasm structural caps, explicit compile/run, and SQLite lock boundaries are covered. Frontend typecheck, 17 files / 137 tests, production build, Rust formatting, Cargo metadata and handoff checks pass. With an extracted Debian Tauri sysroot, the Rust application target now compiles; focused extension tests pass 13/13 and directory-scanner tests pass 2/2. The full Rust library suite is 90 passed / 25 failed. Several failures are nonportable fixtures; workspace-sequence recovery/stop behavior and relocation need resolution. PR #47 code head `d5344b4` is open draft, stacked on PR #46. SW-021 remains in progress and SW-022 remains gated until full suite/behavior checks are acceptable. V1 remains BLOCKED.

## Completed cross-platform implementation — SW-018 (issue #17)

- Contract is in `docs/DECISIONS.md` / `docs/DATA_MODEL.md`; shared typed IPC commands `resource_import_scan` and `resource_import_commit` are in `packages/contracts/src/index.ts`.
- The UI provides explicit source selection, scan, provenance and duplicate-match preview, candidate selection, duplicate policy, and explicit commit. It locks controls during commit. Nothing imported is launched.
- Core scans only user-selected local directories/Start Menu folders or Netscape-format bookmark HTML. It uses `cap-std`/`cap-fs-ext` 4.0.3 capability-relative traversal, no-follow descendant/file opens, max depth 4, max 1,000 entries and max 4 MiB bookmark input; `.url` and bookmarks accept HTTPS only. Windows `.lnk` metadata reads reject arguments and never resolve or execute the target. Network roots are rejected including mapped drives on Windows.
- Commit rescans the bounded source, verifies a token bound to the candidate/source snapshot, destination profile and duplicate match map, validates selection, then inserts resources/targets/profile memberships transactionally. Failure rolls back the batch. Duplicate policy is skip-existing or create-duplicate.
- Independent QA passed with no merge blocker. A medium same-user path race remains for initial root opening and Windows `.lnk` metadata loading by absolute path through `IPersistFile::Load`; descendants and ordinary leaf reads use capability-relative no-follow handles. This limitation is documented and no race-free-isolation claim is made.

## SW-020 verification on integrated PR #46 head 2216bd6

- `npm ci --cache /tmp/swl-npm-cache` — PASS.
- `npm run typecheck` — PASS.
- `npm test -- --run` — PASS, 16 files / 126 tests.
- `npm run frontend:build` — PASS; existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS.
- `git diff --check` — PASS.
- `python scripts/verify_handoff.py` — PASS consistency only; not app compile/runtime proof.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml device_mapping --lib` — BLOCKED before app compilation: GLib/GObject >=2.70 (`glib-2.0.pc`) missing; tests are authored but NOT_RUN.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu --lib` — BLOCKED: `x86_64-w64-mingw32-gcc` missing.
- `python scripts/check_release_gate.py` — BLOCKED (`NOT READY: BLOCKED`, exit 2).
- Independent QA re-reviewed UI and Core fixes and found no remaining blocker. No Windows acceptance.

## Verification and release status

At 2026-10-22 03:35 UTC, GitHub review and inline-thread APIs returned empty for PR #38 and #41. PR #47 is open draft at `a882998`, with implementation code head `d5344b4`. Independent read-only QA found no actionable blocker for PRs #38/#41; #41 has a non-blocking DB-lock duration concern during large CSV export.

- `npm ci --cache /tmp/swl-npm-cache` — PASS.
- `npm run typecheck` — PASS.
- `npm test -- --run` — PASS, 14 files / 115 tests.
- `npm run frontend:build` — PASS; existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS.
- `git diff --check` — PASS.
- `python scripts/verify_handoff.py` — PASS; catalog/schema/fixture consistency only, not application runtime evidence.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml resource_import --lib` — BLOCKED before crate compilation because GLib/GObject >=2.70 development files (`glib-2.0.pc`) are absent.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu --lib` — BLOCKED because MinGW (`x86_64-w64-mingw32-gcc`) is absent.
- `python scripts/check_release_gate.py` — BLOCKED as expected (`NOT READY: BLOCKED`, exit 2). PowerShell and real Windows 11 x64 are unavailable.

Latest SW-021 Rust verification on code head `d5344b4` (Debian 13 x86_64, temporary extracted Tauri sysroot): `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS; `git diff --check` — PASS; `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_ --lib` — PASS, 13 tests; `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml resource_import_directory_scan_ --lib` — PASS, 2 tests; full `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — FAIL, 90 passed / 25 failed. The failures are tracked in `backlog/AGENT_STATUS.json`; full output was `/tmp/sw021-full-integrated.log` in this environment. Independent QA PASS on reparse-point handling; actual Windows test remains NOT_RUN. A Core worktree is fixing fixtures and investigating sequence/relocation behavior.

No Shell, `.lnk`, mapped-drive, DPI/multi-monitor, portable-package or W-001..W-014 acceptance has run on Windows. V1 / SW-012 remain BLOCKED; there is no tested Windows EXE, V1 Release asset or READY notice. No GitHub Actions were added by this continuation.

Integrated SW-021 registry branch checks before runtime UI: `npm run typecheck` — PASS; `npm test -- --run` — PASS, 17 files / 132 tests; `npm run frontend:build` — PASS with existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS; `git diff --check` — PASS; `python scripts/verify_handoff.py` — PASS schema/fixture consistency only. `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_registry --lib` — BLOCKED before project compilation: `glib-2.0.pc` / GLib >=2.70 dev files absent (exit 101); Rust regressions are authored but NOT_RUN. `python scripts/check_release_gate.py` — BLOCKED (`NOT READY: BLOCKED`, exit 2). No Windows 11 x64 acceptance.

## Next executable tasks

1. Finish Rust library suite remediation on `feature/sw021-suite-fixes`, preserving all security constraints. Focused SW-021 tests pass; full suite is not green (90/25). Resolve fixture defects and evaluate sequence recovery/stop and relocation behaviors.
2. Independently review and integrate suite fixes into PR #47; update exact test evidence. SW-022 stays gated until this suite/behavior gate is acceptable.
3. Continue tracking PR #38/#41 reviews; latest API refresh at 2026-10-22 03:13 UTC returned empty. Keep V1/SW-012 BLOCKED until real Windows 11 x64 acceptance.

Canonical agent state: [`AGENT_STATUS.json`](AGENT_STATUS.json). Session continuation: [`CODEX_RESUME.md`](../CODEX_RESUME.md).

SW-021 UI integration at `4041757`: `npm run typecheck` — PASS; `npm test -- --run` — PASS, 17 files / 137 tests; `npm run frontend:build` — PASS with the existing nonfatal Vite warning; `git diff --check` — PASS. Independent UI review passed. Core runtime tests have not run yet; the current host lacks GLib/GObject development files.

Independent Core QA findings on `b8a48ff`: integration is held pending opaque one-time preview token enforcement, hostile ZIP/output regression coverage, and an exact final 64 KiB invocation cap when truncation is marked. Core is remediating in its isolated worktree.

Integrated SW-021 Core follow-up: `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`, `cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --no-deps --format-version 1`, `python scripts/verify_handoff.py`, `git diff --check` — PASS. `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_ --lib` — BLOCKED before app compilation because `glib-2.0.pc` / `gobject-2.0.pc` >=2.70 are absent and `PKG_CONFIG_PATH` is unset; tests authored but NOT_RUN. Independent contract/Core/UI QA PASS. Release gate remains BLOCKED.

Latest extended SW-021 security review (03:12 UTC) PASS after Core commit `d23d008`: Wasm structure limits are enforced before semantic validation; Wasmi compiles once only on explicit run; package parsing and guest execution do not hold the app-wide SQLite mutex. Prior token, ZIP, output, pointer/profile, and truncation remediations remain covered. PR #47 remains a draft because app Rust tests cannot compile on this host.

SW-021 integrated verification (b47e916): frontend typecheck, 17 files / 137 tests, production build — PASS; Rust fmt, Cargo metadata, handoff verification and diff checks — PASS. `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_ --lib` BLOCKED before crate compilation: missing `glib-2.0.pc` and `gobject-2.0.pc` >=2.70 (tests authored but NOT_RUN). Windows GNU `cargo check` BLOCKED at bundled SQLite due absent MinGW GCC. Independent contract/Core/UI/extended security reviews PASS. Release gate remains BLOCKED.
