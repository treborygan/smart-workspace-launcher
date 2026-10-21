# Smart Workspace Launcher — GitHub/Codex multi-agent full-roadmap handoff

**Status: specification + contracts + backlog only. This repository is NOT an implemented, runnable launcher.**

A Windows 11 desktop app: (1) resizable drag-and-drop dashboard, (2) global-hotkey quick launcher, (3) optional floating Dock. These share local resources, profiles, layouts, a metadata cache, and launch history. Supports files in Windows default applications, folders, apps and HTTPS URLs in their associated handlers.

**Naming is deliberately provisional.** See [`docs/NAME_SHORTLIST.md`](docs/NAME_SHORTLIST.md). Keep neutral internal identifiers (`smart-workspace-launcher`, `com.smartworkspace.launcher`) until the product name is approved and verified; changing the display name must not change saved resource IDs.

## Multi-agent full-plan execution

This version explicitly configures **GPT-6 Luna** Codex roles under `.codex/`. Root runs on **Medium**, coding and release subagents on **Medium**, read-only research on **Low**. Use **up to three simultaneous subagents** on separate worktrees and PRs. Release tested **V1**, alert user, then keep implementing **V1.1, V1.2 and V2**. See `docs/MULTI_AGENT_WORKFLOW.md`, `CODEX_RESUME.md` and `docs/NOTIFICATIONS_AND_RELEASE_GATES.md`. The package does not start any remote tasks by itself.

**Repository:** https://github.com/treborygan/smart-workspace-launcher (confirmed existing, public, and empty when handoff revised). Upload this handoff at repo root before starting Codex. Do not push confidential corporate fixtures into public GitHub.

## Start here (Codex)

1. Read [`AGENTS.md`](AGENTS.md) and [`CODEX_KICKOFF.md`](CODEX_KICKOFF.md) in full.
2. Read [`docs/PRODUCT_REQUIREMENTS.md`](docs/PRODUCT_REQUIREMENTS.md), [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`docs/DATA_MODEL.md`](docs/DATA_MODEL.md), and [`docs/TEST_AND_ACCEPTANCE.md`](docs/TEST_AND_ACCEPTANCE.md).
3. Execute [`backlog/ROADMAP.md`](backlog/ROADMAP.md) by dependency, not as independent prototypes.
4. Execute `python scripts/verify_handoff.py` to validate handoff contents and fixtures before changing anything. It does not test an application.
5. Scaffold actual Tauri 2/React/TypeScript/Rust app using current official supported versions on the implementation machine. Record chosen dependency versions and reproducible lock files, then implement the source and tests.

## GitHub-native workflow, without GitHub Actions

- Put the **contents of this folder at the root** of a new/private GitHub repository.
- Keep `main` protected in practice: use feature branches, issues and pull requests; avoid force-push.
- Codex should create a branch per milestone, implement code + tests, run local verification, write a PR summary using `.github/pull_request_template.md` and commit results.
- GitHub Actions and `.github/workflows/` are **not permitted unless user explicitly approves them**. Run verification locally or in an approved Windows development environment.
- Cloud/Linux development may validate frontend, cross-platform Rust logic, schemas, and documentation. **Windows packaging, Shell integration, global hotkeys, tray, DPI/monitor behavior, and executable launch require a real Windows 11 validation machine**.
- Optional GitHub Issues import: `scripts/create_github_issues.ps1` previews by default; `-Apply` requires GitHub CLI access and explicit user approval.

## Locked choices

| Subject | Decision |
|---|---|
| OS | Windows 11 x64 |
| Delivery | Portable distribution first; optional per-user installer |
| Framework | Tauri 2, React, TypeScript, Rust |
| Storage | SQLite local, with schema migrations; no live DB file sync |
| UI | Resize/reorder **grid**; light/dark custom themes |
| Interfaces | Dashboard, Quick Launch, optional separate Dock, Explorer/Inspector |
| Profiles | Local-first; manual validated `.swprofile` ZIP export/import |
| Usage | App-observed launch handoffs; no claim of third-party usage tracking |
| Power | Non-admin operation; no background network required |
| Automation | No GitHub Actions; approved local scripts |

## Handoff file map

- `AGENTS.md`: global instructions for Codex and other coding agents.
- `.codex/`: real Codex config and specialized role files (core/ui/qa/research/release).
- `CODEX_RESUME.md`: restartable context after interrupted sessions.
- `docs/MULTI_AGENT_WORKFLOW.md`: subagent scheduling, worktrees, PR merges, recovery.
- `docs/NOTIFICATIONS_AND_RELEASE_GATES.md`: V1 readiness criteria, GitHub release notice, and mandated continued development.
- `CODEX_KICKOFF.md`: pasteable starting prompt.
- `docs/`: product/UX, architecture, data model, security, testing, release, naming decisions.
- `contracts/`: example JSON Schema contracts for imported profiles and resources.
- `fixtures/`: safe dummy test input including malformed import.
- `backlog/`: ordered milestones and structured GitHub issue catalog.
- `.github/`: issue templates and PR checklist, **no Actions workflows**.
- `scripts/`: contract validation and opt-in issue publisher.

## No implementation yet

This archive contains no runnable EXE and no fabricated passing app tests. Codex must build and verify them. `scripts/verify_handoff.py` checks only the handoff artifact.

## Important deployment notes

Unsigned portable programs can still be blocked by corporate AppLocker/WDAC/Defender policy. WebView2 existence and permitted execution must be checked on the target Windows device; do not assume portability bypasses enterprise policies. Do not upload employer data, actual credentials, or confidential corporate filenames to a public repository.
## GitHub setup state — 2026-10-21

- Repository initialized on `main` with 53 version-controlled handoff files, including five configured Luna subagent roles.
- [25 GitHub implementation issues](https://github.com/treborygan/smart-workspace-launcher/issues) created for SW-001 through SW-025.
- No GitHub Actions, live app code, downloadable EXE or release assertions exist yet.
- No active Codex Tasks environment was registered at verification. **Codex must be connected to this repository and invoked using `CODEX_KICKOFF.md` to begin actual development.**
- V1 availability is independently checked by the release watch only after an actual Windows-tested GitHub release.
