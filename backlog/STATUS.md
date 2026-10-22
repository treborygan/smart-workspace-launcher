# Current implementation status

Updated 2026-10-22 03:40 UTC. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). Main contains the foundational implementation through SW-013 (PR #37). Cross-platform advanced work is progressing in stacked feature branches. The current environment is Debian 13 x86_64 and is not a Windows acceptance environment.

## Completed / merged foundations

SW-001..SW-011 are merged through PR #36; SW-013 is merged through PR #37. SW-014 and SW-015 implementations are complete on PRs #38 and #41. SW-016 and SW-017 implementations are complete on draft PRs #42 and #43. SW-018 is implemented and independently QA-reviewed on PR #44.

## Active pull requests and review state

- PR #38 (SW-014) OPEN against `main`, head `fb22dff`; review and inline-thread APIs refreshed 2026-10-22 03:39 UTC: both empty. Independent QA read-only review found no actionable blocker.
- PR #41 (SW-015) OPEN, stacked on #38, head `c4c8f47`; review and inline-thread APIs refreshed 2026-10-22 03:39 UTC: both empty. Independent QA read-only review found no actionable blocker; non-blocking scale concern: CSV export holds the shared DB lock while writing a potentially large unlimited-retention export.
- PR #42 (SW-016) OPEN_DRAFT, stacked on #41/#38. Migration regression assertion is corrected for schema v10 in `d90204f`.
- PR #43 (SW-017) OPEN_DRAFT, stacked on #42. Its branch includes SW-018 contract documents as an explicit base for the next feature.
- PR #44 (SW-018) OPEN_DRAFT, stacked on #43. Implementation is on `feature/sw-018-discovery-import`; code head `1ec5a7a`.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44 at head `9a984e5`; implementation and independent QA are complete. Rust tests are blocked before compilation by missing GLib/GObject; no Windows acceptance.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44, head `9a984e5`; implementation and independent QA are complete. Rust test compilation remains blocked by missing GLib/GObject; no Windows acceptance.
- PR #46 (SW-020) OPEN_DRAFT, base `feature/sw-019-widget-templates`, integrated head `3a12ab7`. Implementation and independent QA are complete; frontend 126 tests/typecheck/build, Rust formatting, diff and handoff checks pass. Rust test compilation is blocked by missing GLib/GObject, Windows GNU check by missing MinGW. V1 remains BLOCKED.
- PR #47 (SW-021) OPEN_DRAFT at code head `8aa19f8`, base `feature/sw-020-device-paths` / PR #46. Runtime and UI are integrated. Full Rust library suite passes 116/116 on Debian 13 with the extracted Tauri sysroot; focused extension tests pass 13/13 and scanner tests pass 2/2. Independent QA passed the launch-history purge fix; Windows-only junction test remains NOT_RUN. Do not merge out of dependency order.
- PR #39 was closed as superseded without merging. No PR review requests or comments were sent. Do not merge stacked work before review and dependency-order checks.

## SW-020 implementation — issue #20

SW-020 adds random per-user local device identity, device labels, approved-by-construction path-prefix rules, exact preview/transactional commit/delete and launch-time-only mapping. Mappings are not exported with profiles, and copied portable data uses a different local identity on the destination machine. Drive-letter/UNC paths only; mapping is boundary-safe and Unicode case-insensitive, with most-specific prefix precedence. Only active file/folder target URIs are translated in the explicit launch copy; stored paths, arguments, working directories and history/export identity remain untouched. No scanning/copying/moving/automatic launching occurs. UI review fixed delayed-preview divergence and keyboard focus; Core review fixed Unicode comparisons.

## SW-021 contract and implementation — issue #21

SW-021 integrates schema v13 package storage, validated `.swext` review/install, explicit grants and enablement, an explicit-run Wasm sandbox, and the matching UI. Contract, Core, UI and extended security QA passed. Opaque one-use preview tokens, hostile package/output and profile tests, deterministic 64 KiB input/count truncation, Wasm structural caps, explicit compile/run, and SQLite lock boundaries are covered. Frontend typecheck, 17 files / 137 tests, production build, Rust formatting, Cargo metadata and handoff checks pass. On Debian 13 x86_64 with an extracted Debian Tauri sysroot, the Rust library suite passes 116/116; focused extension tests pass 13/13 and directory-scanner tests pass 2/2. Independent QA passed the launch-history purge fix; Windows-only junction testing is NOT_RUN. PR #47 code head `8aa19f8` is open draft, stacked on PR #46. SW-021 cross-platform test gate is green; V1 remains BLOCKED.

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

At 2026-10-22 03:39 UTC, connected GitHub review-submission and inline-thread APIs returned empty for PR #38 (SW-014) and PR #41 (SW-015). Both remain open and mergeable. Independent static QA found no actionable blocker; PR #41 retains a non-blocking shared DB-lock duration concern for very large CSV exports. PR #47 is open draft, head `8aa19f8`, stacked on PR #46.

Latest SW-021 verification on Debian 13 x86_64 with the temporary `/tmp/swl-sysroot` Tauri dependencies:

- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — PASS, 116 passed / 0 failed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_ --lib` — PASS, 13 passed / 0 failed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml resource_import_directory_scan_ --lib` — PASS, 2 passed / 0 failed on Linux. The Windows junction test is cfg(windows), NOT_RUN.
- `npm run typecheck` — PASS.
- `npm test -- --run` — PASS, 17 files / 137 tests.
- `npm run frontend:build` — PASS; existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning.
- `git diff --check` — PASS.
- `python scripts/verify_handoff.py` — PASS; handoff/catalog/schema/fixture consistency only, not app runtime or Windows proof.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu --lib` — BLOCKED because `x86_64-w64-mingw32-gcc` is absent.
- `python scripts/check_release_gate.py` — BLOCKED as expected (`NOT READY: BLOCKED`, exit 2).

The real host is Debian 13 x86_64; PowerShell and Windows 11 x64 are unavailable. There is no Windows acceptance, tested portable EXE/package, release upload, or V1 READY notice. V1/SW-012 remains BLOCKED. No GitHub Actions were added.

## Next executable tasks

1. Freeze a security-reviewed SW-022 selective-sync contract before implementation. The issue and roadmap do not specify provider/protocol, encryption, conflict/deletion behavior, selectable entities, compatibility, or local-only data treatment. Repository constraints prohibit live SQLite/WAL sync and require explicit opt-in.
2. Continue SW-022 and later work in roadmap dependency order; update issues/PRs with evidence.
3. Keep Windows V1 blocked until real Windows 11 x64 acceptance and a tested portable package.
