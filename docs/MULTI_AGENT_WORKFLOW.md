# GitHub-first multi-agent Codex workflow

## Mission
Implement and ship an independently Windows-tested V1.0, notify the user with the real downloadable GitHub Release, then continue toward V1.1, V1.2 and V2.0 advanced goals. V1 is a milestone, not the project endpoint. Do not claim agents work while their execution session is stopped.

## Agent configuration
Read `.codex/config.toml` and the five role files. Root coordinator GPT-6 Luna / Medium. Backend Core, React UI, QA and Windows Release Luna / Medium. Narrow read-only research Luna / Low. Three simultaneously active subagents maximum in addition to coordinator. Spawn Core + UI when type contracts are frozen, and QA for independent test/review. Release replaces an idle slot at the native Windows gate; research replaces an idle slot for time-limited investigation.

If runtime does not recognize the configured model names/role settings, report this truthfully and surface the needed change before executing under a different model. Codex must explicitly spawn/delegate tasks; role config alone does not start tasks.

## Ownership
- Root: architecture and contract freeze, dependency scheduling, integration, merges, project status and notifications.
- Core: `apps/desktop/src-tauri/**`, migrations and IPC/domain types when assigned.
- UI: `apps/desktop/src/**`, UI components and frontend tests after IPC freeze.
- QA: acceptance fixtures, independent verification, review of PRs, regressions/security.
- Release: Windows packaging, checksums and executable smoke tests on real Windows.
Never allow simultaneous writers for shared files like `package.json`, `Cargo.lock`, contracts, migrations or status files.

## Workflow
0. Preflight repository permissions, existing work, approved Windows runner and availability of actual Codex execution environment. Check `backlog/STATUS.md`, `backlog/AGENT_STATUS.json`, PRs and open issues.
1. Freeze typed IPC/event contracts and common UUID identifiers with one accountable owner.
2. Work on feature branches/worktrees, one GitHub task per PR when practical. Root assigns exact paths, dependencies, acceptance, tests and expected artifacts.
3. Core and UI build independently against typed contracts. QA owns tests/reviews and verifies not only assertions but executed commands. Root merges in dependency order after review.
4. After each merge root updates durable state with SHA, PR, tests, blockers and next task; incomplete verification is explicitly NOT_RUN/BLOCKED.
5. On a verified Windows 11 V1 release, publish tagged release, executable ZIP, SHA256, test evidence and a dedicated notice. Do not stop; continue advanced milestones.
6. V1.1 dynamic resources and workflows; V1.2 import/discovery/widgets/device mappings; V2 permissioned extensions, optional sync/remote/AI, hardening and native releases.

## GitHub rules
No GitHub Actions; do not create `.github/workflows/` or automate changes without permission. Use GitHub issues, PR template, code review and local reproducible scripts. Protect existing work; no force pushes. Public repo: no employer files, network hostnames, real user data, tokens or sensitive logs.

## Resilience
Status and ownership in `backlog/AGENT_STATUS.json`; human readable progress in `backlog/STATUS.md`. In interrupted sessions, begin with `CODEX_RESUME.md`. Never claim background continuation after runtime shutdown. If Windows validation unavailable, finish feasible cross-platform work but mark release BLOCKED.
