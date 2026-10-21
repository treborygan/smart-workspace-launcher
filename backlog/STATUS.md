# Current implementation status

Updated 2026-10-21 22:24 UTC. Repository: [`treborygan/smart-workspace-launcher`](https://github.com/treborygan/smart-workspace-launcher). SW-011 PR #36 merged to `main` as `0a212d8541328d3b41033ba36496871c2b05d8dc`; SW-013 PR #37 merged to `main` as `77b3b1b95f077d53622848256a62662e9a17a54`. SW-014 implementation is on `feature/sw-014-action-sequences`, rebased atop merged main, and awaiting final independent QA.

## Environment and release status

Debian 13 x86_64; Node 24.19.0/npm 11.9.0; Rust 1.99.1 stable. PowerShell and a real Windows 11 x64 host are unavailable. Linux Rust tests stop before project compilation because GLib >=2.70 development files are missing; Windows GNU checks cannot run without MinGW. No Windows binary, portable package, installer, GitHub Release upload, or V1 READY notice exists. `releases/v1/readiness.json` remains BLOCKED. `.github/workflows/` remains absent.

## Completed implementation

- SW-001..004 foundation, profiles, CRUD, launch/history and layouts (#26).
- SW-005 Quick Launch (#27).
- SW-007 metadata and analytics backend/UI (#30/#33).
- SW-008 safe profile archive import/export (#34).
- SW-009 independent Dock (#29).
- SW-010 Resource Explorer, Inspector, profile themes and responsive dashboard (#35; merged `9c8f8a7`).
- SW-011 security hardening (#36; merged `0a212d8`): Windows path checks, HTTPS credential rejection, profile/resource launch authorization, replay intent/target validation, bounded profile names, and SQLite timeout/trusted schema settings.

## In progress

- SW-013 (issue #12) is complete and merged in PR #37 (`77b3b1b`).
- SW-014 (issue #13) is implemented on `feature/sw-014-action-sequences`: profile-scoped named sequences of 1–20 existing-resource steps, optional active owned targets, manual launch through the validated command, durable per-step progress/idempotent request replay, stop on first failure, and inert archive round trips with transactional collision remapping. Final independent QA passed without blocker. PR #38 is open against `main`, head `fb22dff`; GitHub review is pending. This is cross-platform V1.1 work; its SW-012 Windows acceptance dependency remains unmet.


## In progress

- SW-015 (issue #14) is active on `feature/sw-015-usage-retention`, stacked on SW-014 PR #38. The TypeScript contract is frozen for profile-local UTC filters, pagination, scoped recording/retention/exclusions, explicit purge, and CSV export. UI is integrated (`fb2146f`), with 88 frontend tests passing. Core is correcting backend findings: CSV must not leak a path used as a resource display name, and workspace sequence run storage must honor opt-out/purge/retention while preserving minimal replay guards. Backend commits integrated locally through `19b3c75`; draft PR #39 is open on a divergent branch. PR #38 review remains pending.

## Verification evidence

- Integrated SW-015 UI: `npm run typecheck && npm test -- --run && npm run frontend:build && git diff --check` — PASS; 12 files/88 tests. Existing nonfatal Vite dynamic import warning. `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` and `python scripts/verify_handoff.py` — PASS. `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib usage` — BLOCKED before app compile because GLib >=2.70 development files / `glib-2.0.pc` are absent. Independent QA cleared UI after fixes but backend approval is held on CSV path-like display names and detailed sequence-run storage after privacy controls. `python scripts/check_release_gate.py` — BLOCKED; no Windows 11 x64 runtime.

- SW-014 current tree: `npm run typecheck && npm test -- --run && npm run frontend:build && git diff --check` — PASS (82 frontend tests/12 files, build); `python tests/test_profile_archive_contract.py -q` — PASS (14); `python scripts/verify_handoff.py` — PASS; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS. Focused Rust sequence tests are authored but cannot compile: GLib/GObject >=2.70 dev files are absent. Final independent QA review passed without blocker. PR #38 is open; the PR artifact connector returned connection closed and needs a retry. `python scripts/check_release_gate.py` — BLOCKED as expected; real Windows 11 x64 is unavailable.
- SW-013 current tree: `npm run typecheck && npm test -- --run && npm run frontend:build` — PASS (73 tests across 11 files; production build, existing non-fatal Vite dynamic-import warning). `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check && git diff --check` — PASS. `python tests/test_profile_archive_contract.py -q` — PASS (11 tests); `python scripts/verify_handoff.py` — PASS. `python scripts/check_release_gate.py` — BLOCKED as expected. Focused Rust tests including Replace/remap and injected transaction rollback are authored, but blocked before compilation by missing GLib/GObject >=2.70 development packages. Windows GNU checks remain blocked by missing MinGW; Windows acceptance unavailable. Final QA found no SW-013 blocker.
- On integrated SW-011: `npm ci --cache /tmp/swl-npm-cache && npm run typecheck && npm test && npm run frontend:build && git diff --check` — PASS; 8 frontend test files/56 tests; build and diff check pass. Vite reports a non-failing ineffective dynamic import warning.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` — PASS.
- `cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --no-deps --format-version 1` — PASS.
- `python -m unittest discover -s tests -v` — PASS (11 archive QA harness tests).
- `python scripts/verify_handoff.py` — PASS (handoff/schema/fixture checks, not runtime verification).
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib` — BLOCKED before project compile: missing GLib >=2.70 development files.
- QA reviewed SW-011 integrated head `b7fe9b6`: no blocker. Caveat: replaying a formerly valid explicit-target request after that target is deactivated returns NotFound; it does not launch again.
- Performance targets and 10k-resource measurements remain unverified. No Windows acceptance, packaging, DPI/native, startup, memory, or Shell behavior has been confirmed.

## Next executable tasks

1. Await review and merge SW-014 PR #38, then continue SW-015..SW-025 in dependency order while performing cross-platform work available here.
2. Run SW-012 acceptance and portable packaging only on real Windows 11 x64. Until then V1 stays BLOCKED and no READY notice may be sent.

Canonical agent state: [`AGENT_STATUS.json`](AGENT_STATUS.json). Resumption notes: [`CODEX_RESUME.md`](../CODEX_RESUME.md).
