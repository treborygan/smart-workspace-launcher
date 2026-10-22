# Current implementation status

Updated 2026-10-22 02:48 UTC. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). Main contains the foundational implementation through SW-013 (PR #37). Cross-platform advanced work is progressing in stacked feature branches. The current environment is Debian 13 x86_64 and is not a Windows acceptance environment.

## Completed / merged foundations

SW-001..SW-011 are merged through PR #36; SW-013 is merged through PR #37. SW-014 and SW-015 implementations are complete on PRs #38 and #41. SW-016 and SW-017 implementations are complete on draft PRs #42 and #43. SW-018 is implemented and independently QA-reviewed on PR #44.

## Active pull requests and review state

- PR #38 (SW-014) OPEN against `main`, head `fb22dff`; review and inline-thread APIs returned no submissions at 2026-10-22 02:47 UTC. Independent QA read-only review found no actionable blocker.
- PR #41 (SW-015) OPEN, stacked on #38, head `c4c8f47`; review and inline-thread APIs returned no submissions at 2026-10-22 02:47 UTC. Independent QA read-only review found no actionable blocker; non-blocking scale concern: CSV export holds the shared DB lock while writing a potentially large unlimited-retention export.
- PR #42 (SW-016) OPEN_DRAFT, stacked on #41/#38. Migration regression assertion is corrected for schema v10 in `d90204f`.
- PR #43 (SW-017) OPEN_DRAFT, stacked on #42. Its branch includes SW-018 contract documents as an explicit base for the next feature.
- PR #44 (SW-018) OPEN_DRAFT, stacked on #43. Implementation is on `feature/sw-018-discovery-import`; code head `1ec5a7a`.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44 at head `9a984e5`; implementation and independent QA are complete. Rust tests are blocked before compilation by missing GLib/GObject; no Windows acceptance.
- PR #45 (SW-019) OPEN_DRAFT, base PR #44, head `9a984e5`; implementation and independent QA are complete. Rust test compilation remains blocked by missing GLib/GObject; no Windows acceptance.
- PR #46 (SW-020) OPEN_DRAFT, base `feature/sw-019-widget-templates`, integrated head `3a12ab7`. Implementation and independent QA are complete; frontend 126 tests/typecheck/build, Rust formatting, diff and handoff checks pass. Rust test compilation is blocked by missing GLib/GObject, Windows GNU check by missing MinGW. V1 remains BLOCKED.
- PR #47 (SW-021) OPEN_DRAFT at `e186f3c`, base `feature/sw-020-device-paths` / PR #46. It contains the registry/policy foundation plus a frozen, independently reviewed sandbox runtime contract (`2ca5ea4`). Core package/runtime and corresponding UI implementation are active in isolated worktrees; it does **not** yet execute extension code. Issue #21 remains in progress. GitHub previously reported the stacked PR as not mergeable pending its base chain; do not merge out of dependency order.
- PR #39 was closed as superseded without merging. No PR review requests or comments were sent. Do not merge stacked work before review and dependency-order checks.

## SW-020 implementation — issue #20

SW-020 adds random per-user local device identity, device labels, approved-by-construction path-prefix rules, exact preview/transactional commit/delete and launch-time-only mapping. Mappings are not exported with profiles, and copied portable data uses a different local identity on the destination machine. Drive-letter/UNC paths only; mapping is boundary-safe and Unicode case-insensitive, with most-specific prefix precedence. Only active file/folder target URIs are translated in the explicit launch copy; stored paths, arguments, working directories and history/export identity remain untouched. No scanning/copying/moving/automatic launching occurs. UI review fixed delayed-preview divergence and keyboard focus; Core review fixed Unicode comparisons.

## SW-021 contract and implementation — issue #21

The integrated SW-021 foundation adds schema v12, validated extension manifests, explicit grants, enablement and confirmed removal, with independent QA. It is registry/policy only; it does not run package code. The sandbox runtime contract was then frozen and independently QA-approved in commit `2ca5ea4`: exact `.swext` entries/size bounds; preview-bound package hashes; no Wasm imports or start sections; strict ABI and resource bounds; explicit user-only execution; capability-to-input-field schema pairing; UUID-ordered metadata within a 64 KiB serialized budget; and bounded text-only suggestions. See `docs/EXTENSION_SDK.md` and the extension schemas. Core works in `/workspace/sw021-runtime-core`; UI works in `/workspace/sw021-runtime-ui`. Their implementation is not yet integrated or verified. SW-021 remains in progress. V1 remains BLOCKED.

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

At 2026-10-22 02:47 UTC, PR #38 and #41 review and inline-thread APIs returned empty. Independent read-only QA found no actionable blocker; #41 has a non-blocking DB-lock duration concern during large CSV export.

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

No Shell, `.lnk`, mapped-drive, DPI/multi-monitor, portable-package or W-001..W-014 acceptance has run on Windows. V1 / SW-012 remain BLOCKED; there is no tested Windows EXE, V1 Release asset or READY notice. No GitHub Actions were added by this continuation.

Integrated SW-021 branch checks: `npm run typecheck` — PASS; `npm test -- --run` — PASS, 17 files / 132 tests; `npm run frontend:build` — PASS with existing nonfatal Vite `INEFFECTIVE_DYNAMIC_IMPORT` warning; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS; `git diff --check` — PASS; `python scripts/verify_handoff.py` — PASS schema/fixture consistency only. `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml extension_registry --lib` — BLOCKED before project compilation: `glib-2.0.pc` / GLib >=2.70 dev files absent (exit 101); Rust regressions are authored but NOT_RUN. `python scripts/check_release_gate.py` — BLOCKED (`NOT READY: BLOCKED`, exit 2). No Windows 11 x64 acceptance.

## Next executable tasks

1. Continue SW-021 with a sandboxed extension runtime and permission-checked host API; do not mark issue #21 done or start dependent SW-022 until executable behavior and its tests pass.
2. Continue tracking PR #38/#41 external review; refresh their review state and run blocked Rust tests on capable hosts.
3. Continue SW-022..SW-025 in dependency order. Keep V1/SW-012 BLOCKED until real Windows 11 x64 acceptance.

Canonical agent state: [`AGENT_STATUS.json`](AGENT_STATUS.json). Session continuation: [`CODEX_RESUME.md`](../CODEX_RESUME.md).
