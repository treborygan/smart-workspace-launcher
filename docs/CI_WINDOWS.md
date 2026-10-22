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

1. Merge CI and validate successful Windows-hosted runs.
2. Build and download preview Windows ZIP; inspect logs and checksum.
3. Fix all blocking build/test issues using focused PRs and review.
4. On a real approved Windows 11 x64 machine, run W-001..W-014 as a **standard user** and retain evidence.
5. Only after full acceptance, publish `v1.0.0` with release asset, SHA256, notes and V1-ready notification.
6. Continue V1.1, V1.2 and V2.0 using the same CI plus Windows release gates.
