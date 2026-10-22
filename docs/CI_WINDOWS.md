# GitHub Actions Windows CI and preview builds

## Approval and scope

The repository owner explicitly approved GitHub Actions on 2026-10-22 as an exception to the previous no-Actions rule. Approval covers **automated tests, Windows x64 compilation, preview artifact packaging, and limited evidence uploads**. It does **not** authorize release certification without real Windows 11 testing, automatic V1 publication, unrelated automation, or transferring confidential user/company files.

## Workflows

| Workflow | Triggers | Host | Results |
|---|---|---|---|
| [Windows CI](../.github/workflows/windows-ci.yml) | PRs to `main`, pushes to `main`, manual | `windows-2025` x64 | Node/TypeScript/React, Python/contract, Rust/MSVC, frontend production build; verification logs |
| [Windows Build](../.github/workflows/windows-build.yml) | pushes to `main`, manual | `windows-2025` x64 | Mandatory tests followed by Tauri native Windows build, preview portable ZIP, installer (if supported), SHA256 and provenance |

Both use `GITHUB_TOKEN` with only `contents: read`; no external secrets, deployment, repository mutation or elevated permission. Builds run on the actual checked-out commit. Each job uses a timeout, dependency caches, and a bounded artifact retention period. Outputs for previews must remain clearly labeled **UNVERIFIED**.

### Running

1. Open [Actions](https://github.com/treborygan/smart-workspace-launcher/actions).
2. Select **Windows CI** or **Windows Build (unverified preview)**.
3. For a manual run, select `main` or an appropriate branch and press **Run workflow**.
4. Inspect the job summary, step logs and **Artifacts** section when complete.
5. For preview Windows installation, download the *portable preview* ZIP and accompanying checksum, extract, and verify SHA256 locally before running.

A pull request to `main` automatically triggers Windows CI. Merge only after CI output is reviewed and relevant failures fixed. The first merge to `main` also triggers the Windows preview build.

## First verified preview artifacts (2026-10-22)

The approved workflows are implemented on `main` from [merged PR #40](https://github.com/treborygan/smart-workspace-launcher/pull/40) at commit `98b1432a6596152e1e6885e9ab569266994f1cd6`.

| Recorded result | Evidence |
|---|---|
| Windows CI **PASS** — 73 frontend tests / 11 files; 63 Rust tests; Python/contract checks; typecheck and production frontend build | [Run 39723525925](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525925) |
| Windows preview build **PASS** — native Tauri EXE, NSIS, ZIP, checksum, package validation | [Run 39723525976](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976) |
| Portable Windows x64 ZIP + SHA256 | [Artifact 12402740774](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402740774) |
| NSIS installer | [Artifact 12402700842](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402700842) |
| Build evidence logs | [Artifact 12402561192](https://github.com/treborygan/smart-workspace-launcher/actions/runs/39723525976/artifacts/12402561192) |

**Portable inner ZIP filename:** `smart-workspace-launcher-v0.1.0-preview.1-win-x64-portable.zip`.

**Portable inner ZIP SHA256:**
```text
c209f2c27382f1c77215b0a4344515114124258117598373da78f8d727197f4c
```

These are **unsigned, pre-release GitHub Actions artifacts**, not permanent release downloads. Each linked artifact is downloaded as an **outer ZIP**; extract it to obtain the inner portable ZIP and `.sha256`, or the NSIS installer. Downloads may require GitHub sign-in. Artifact retention is configured to **14 days**; links will expire, after which use a fresh successful run or a properly certified GitHub Release.

Check the SHA256 of the **inner portable ZIP** after extraction, for example in PowerShell:
```powershell
Get-FileHash -Algorithm SHA256 .\smart-workspace-launcher-v0.1.0-preview.1-win-x64-portable.zip
```
Compare the returned hash with the value above or the bundled `.sha256` file. A matching hash confirms file integrity, **not** successful Windows 11 runtime behavior or publisher identity.

**Release status:** No W-001–W-014 Windows 11 standard-user interactive acceptance evidence has been submitted. `releases/v1/readiness.json` must remain `BLOCKED` and there must be no V1 READY announcement.

## Required evidence

- Exact commit SHA and runner OS/architecture/tool versions
- Commands executed and exit codes for frontend, Python, Rust and production build
- ZIP archive integrity, EXE presence, and SHA256 digest
- Artifact download URLs and GitHub Actions run/job links
- Unresolved warnings and tests that could not run on the hosted environment
- No private/confidential user data in publicly downloadable artifacts or logs

CI logs are useful evidence that tests **ran on GitHub-hosted Windows Server**, not evidence of a Windows 11 standard-user session.

## Test-to-environment coverage

| Test scope | Hosted Windows Server CI | Real Windows 11 gate |
|---|---|---|
| TypeScript/Rust/Python unit and integration tests | Yes | Optional rerun |
| SQLite migrations, schema and import security tests | Yes | Optional rerun |
| Production frontend and Tauri EXE build | Yes | Fresh install/download smoke |
| Executable/package structure and checksum | Yes | Fresh downloaded package proof |
| Shell/default Excel/PDF handlers and web browser | Limited; do not infer end-to-end | W-001..W-004 |
| Unavailable UNC resource and focus/hotkey/tray | Not reliably representative | W-005..W-007 |
| Dock and true multi-monitor/DPI behavior | No reliable equivalent | W-008..W-009 |
| Dashboard persistence, clean-user profile round trip | Unit coverage only unless interactive harness added | W-010..W-011 |
| Search/usage events offline and clean portable installation | Automated subcomponents | W-012..W-014 |

**Hard gate:** `releases/v1/readiness.json` remains `BLOCKED` until actual W-001..W-014 evidence is attached. A green workflow or successful ZIP cannot trigger the final V1 READY notice by itself.

## Troubleshooting

- **MSVC missing:** verify GitHub runner image, Rust toolchain target `x86_64-pc-windows-msvc` and C++ Build Tools installation. Do not silently substitute the GNU toolchain.
- **PowerShell parameter-binding error:** invoke the existing `scripts/verify.ps1` and use explicit `-Arguments` arrays inside its checked command helper.
- **Rust test failure:** preserve Cargo diagnostics, fix source/toolchain mismatch and rerun. Never use `-SkipCargo` for qualifying CI.
- **Tauri/NSIS failure:** inspect the exact `tauri.conf.json`, CLI invocation, required icons and Windows packaging tools. Repair the build, do not upload a partial preview under a success label.
- **Artifact ZIP mismatch:** compare SHA256 against adjacent `.sha256` file and fail rather than publish.
- **GUI check blocked:** record `BLOCKED` or `NOT_RUN`. Hosted runners do not stand in for full interactive Windows 11 acceptance.
- **Hosted disk limit:** reduce caches/retention and remove unused files; retain reproducibility and mandatory tests.

## Release progression

1. **Done (2026-10-22):** Merge CI (#40) and verify a successful hosted-Windows CI run.
2. **Done:** Build portable preview ZIP/installer and verify ZIP format and checksum in the hosted runner. A fresh-download test on Windows 11 is still outstanding.
3. Fix any remaining application issues uncovered by native testing using focused PRs and review.
4. On a real approved Windows 11 x64 machine, run W-001..W-014 as a **standard user**, validate the freshly downloaded preview and retain redacted evidence.
5. Only after full acceptance, publish `v1.0.0` with release asset, SHA256, notes and V1-ready notification.
6. Continue V1.1, V1.2 and V2.0 using the same CI plus Windows release gates.
