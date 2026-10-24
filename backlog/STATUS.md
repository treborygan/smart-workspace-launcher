# Current implementation status

**Verified 2026-10-22 against GitHub `main`:** `98b1432a6596152e1e6885e9ab569266994f1cd6`. This is a **repository snapshot**, not a claim that any Codex agent is currently running.

## Release status

| Gate | Result | Evidence |
|---|---|---|
| V1 application implementation | Integrated in `main`; native behavior still awaiting interactive validation | [SW-001–SW-011](../docs/TRACEABILITY.md) |
| Windows-hosted CI | **PASS** — Windows Server 2025 x64, 73 frontend tests (11 files), 63 Rust tests, Python/contract checks, frontend production build | [CI run 39723525925](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525925) |
| Windows x64 preview packaging | **PASS** — Tauri EXE, NSIS installer, portable ZIP, package structure and SHA256 | [Build run 39723525976](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976) |
| Preview artifacts | **AVAILABLE TEMPORARILY** — unsigned and unqualified | [Portable](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402740774) · [Installer](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402700842) |
| W-001–W-014 on actual Windows 11 x64 | **NOT_RUN / BLOCKED** — no evidence of completed standard-user GUI acceptance | [Test matrix](../docs/TEST_AND_ACCEPTANCE.md) |
| Final GitHub Release / V1 READY notice | **BLOCKED** — no certified V1 release asset | [Readiness JSON](../releases/v1/readiness.json) |

**Portable preview:** `smart-workspace-launcher-v0.1.0-preview.1-win-x64-portable.zip` inside the GitHub Actions artifact, SHA256 `c209f2c27382f1c77215b0a4344515114124258117598373da78f8d727197f4c`. Do **not** mark `readiness.json` PASS based on this build.

Artifacts are subject to the workflow's **14-day retention** and may require GitHub sign-in. There is no permanent V1 download under [Releases](https://github.com/treborygan/smart-workspace-launcher/releases) yet.

## Mainline implementation

- SW-001–SW-004 foundation, SQLite, resources, profiles and layouts; SW-005 keyboard launcher; SW-006 Dashboard; SW-007 metadata/analytics; SW-008 import/export; SW-009 independent Dock; SW-010 Explorer/Inspector/themes; SW-011 security/hardening.
- **SW-013** dynamic shortcuts and smart collections: [PR #37](https://github.com/treborygan/smart-workspace-launcher/pull/37) **merged** as `77b3b1b`.
- **Windows CI and preview packaging:** [PR #40](https://github.com/treborygan/smart-workspace-launcher/pull/40) **merged** as `98b1432`. The work fixed an absent Windows icon, native Rust compile errors, synthetic Windows test fixtures, regression assertions, and PowerShell verification argument binding.

## Active development proposals (not part of `main`)

The following PRs were open at the verified snapshot. An open PR does not mean a feature has passed QA or is released:

| Work | PR |
|---|---|
| SW-014 workspace action sequences | [#38](https://github.com/treborygan/smart-workspace-launcher/pull/38) |
| SW-015 usage dashboard, export and retention | [#41](https://github.com/treborygan/smart-workspace-launcher/pull/41) |
| SW-016 resource relationships and snapshots | [#42](https://github.com/treborygan/smart-workspace-launcher/pull/42) |
| SW-017 duplicate/relocation assistant | [#43](https://github.com/treborygan/smart-workspace-launcher/pull/43) |
| SW-018 discovery importer | [#44](https://github.com/treborygan/smart-workspace-launcher/pull/44) |
| SW-019 widgets | [#45](https://github.com/treborygan/smart-workspace-launcher/pull/45) |
| SW-020 per-device mappings | [#46](https://github.com/treborygan/smart-workspace-launcher/pull/46) |
| SW-021 permissioned extensions | [#47](https://github.com/treborygan/smart-workspace-launcher/pull/47) |

## Next executable steps

1. Execute the **actual Windows 11 x64 interactive acceptance suite W-001–W-014** using the GitHub Actions preview package on a standard-user installation. Capture reproducible, redacted evidence of Shell default handlers, global hotkeys, tray, Dock, multi-monitor/DPI, local profiles, offline use and a freshly extracted executable.
2. If failures arise, repair them in focused GitHub PRs and rerun Windows CI/build plus affected Windows 11 tests.
3. Only after all V1 criteria pass, publish a permanent tagged GitHub release with verified checksum, release notes and the user-facing V1 READY notice.
4. Continue reviewing/merging advanced SW-014–SW-025 changes in dependency order; do not confuse V1 readiness with V1.1–V2 completion.

Useful references: [Windows CI and preview instructions](../docs/CI_WINDOWS.md) · [Release gate](../docs/NOTIFICATIONS_AND_RELEASE_GATES.md) · [Codex resume](../CODEX_RESUME.md) · [Roadmap](ROADMAP.md).

For activity/agent session state, see [AGENT_STATUS.json](AGENT_STATUS.json), which separates historical agent task records from observed GitHub facts.

## 2026-10-24 local Windows acceptance update

Reviewed the current repository documents, issue #11, existing source and acceptance matrix. The local Windows 11 Pro x64 pass on `main` recorded in local-only acceptance report is **PARTIAL; V1 remains BLOCKED**. The user confirmed no dedicated standard account is available and only one display is available. W-001 and W-003 passed; W-002, W-004, W-006, W-009 and W-014 were partial; W-005, W-010, W-012 and W-013 were not run; W-008 and W-011 are blocked. W-007 failed the tested preview's read-only tray/lifecycle source audit. Computer Use later refused further input after repeated out-of-window coordinates; no alternate UI driver was used.

To address W-007's source finding, branch `feature/sw-012-tray-window-lifecycle` adds a native tray menu, single-instance dashboard focus, and a local app-wide close preference (`Hide to tray` or `Exit`), with SQLite persistence and accessible Settings controls. Final local automated verification on this branch passed: `scripts/verify.ps1` (frontend 82/82, Python 11/11, Rust 67/67, plus lint/typecheck, handoff and formatting); `npm run build` produced the x64 executable and NSIS installer. The rebuilt EXE SHA256 is `EB35D23A9B9181DD21A8A3953AF312A3AF5BB4FC13E92B0A184FD4D84B78615F`. Build and source tests do not exercise tray callbacks, close/hide behavior or single-instance behavior interactively; W-007 therefore remains unverified on this branch. No new packaged GUI acceptance was run after this source change.

The current host remains a filtered administrator account, not a standard account; no second monitor is attached. DPI beyond 100%, fresh portable extraction under standard-user permissions, alternate hotkey registration, missing-path behavior, local-profile restart persistence, search/tag/analytics UI, and offline behavior remain unverified. Do not publish V1 or mark readiness PASS. Continue with a real Windows 11 standard-user account, second monitor, and interactive W-001–W-014 completion.

The PR head's hosted Windows CI check passed on Windows Server 2025 x64: [run 40096989462](https://github.com/treborygan/smart-workspace-launcher/actions/runs/40096989462). This is automated verification only; it is not Windows 11 interactive acceptance.

A fresh local portable ZIP was also built from PR #50 and extracted successfully: eleases/local/smart-workspace-launcher-v0.1.0-sw012-pr50-win-x64-portable.zip, SHA256 58ec7ee229c448b2009ec5572c2b99cd88c97a1e153781407e499c1308d417b4; extracted EXE SHA256 90F966A2CE41A3EE1A6711B1DB5DED59DDDD01F34AE522DBB30EDBCD54E47DA4. It is a local, unverified artifact, not a release. No fresh-extracted launch or tray interaction was run because the standard-user and native UI acceptance gates remain unavailable.
