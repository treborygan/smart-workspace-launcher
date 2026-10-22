# Codex operating contract — Smart Workspace Launcher

## Mission
Deliver a **working, tested Windows 11 x64 desktop launcher**, then continue through V1.1, V1.2 and V2 advanced features. V1 is an intermediate release, not project completion. The spec is locked: hybrid Dashboard/Quick Launch/optional Dock; resizable drag-and-drop grid; local manually exportable/importable profiles; fully supported light/dark themes; balanced V1 scope.

## Non-negotiable rules
1. Read `README.md`, `CODEX_KICKOFF.md`, the requirements, architecture, data model, security and test documents before coding. Treat JSON schemas and fixtures as implementation contracts. If requirements conflict, use precedence: latest user instruction > `docs/DECISIONS.md` > `docs/PRODUCT_REQUIREMENTS.md` > other docs > code comments; record ambiguity in an ADR.
2. Do not replace the requested native desktop app with a browser-only demo, mock UI, Python script, or instructions-only result. Create real source code, builds and tests.
3. Work in **GitHub**: feature branches, focused commits, pull requests, issue cross-links. Never directly overwrite unrelated work. A PR requires a clear test-evidence section.
4. **GitHub Actions exception approved for this repository (2026-10-22):** `.github/workflows/windows-ci.yml` and `windows-build.yml` may run hosted Windows x64 automated tests, build preview binaries, and upload non-sensitive logs/artifacts. Keep permissions minimal and respect untrusted PRs; no automatic release publication, unrelated third-party CI or other deployments without additional approval. Real Windows 11 standard-user acceptance remains mandatory for V1 readiness. Preserve local verification scripts.
5. Never invent tests, performance measurements, an executable, screenshots, user approvals, trademark registration, or platform verification. Mark unrun checks BLOCKED or NOT RUN.
6. No credential collection, telemetry, cloud sync, network monitoring of arbitrary shares, privilege escalation or unrestricted script execution in V1.
7. Use Windows Shell handlers for opening files and HTTPS URLs. Treat Windows Shell handoff success separately from actual document load. All actions must log an event exactly once at the application boundary.
8. Keep shortcuts logical: immutable UUID IDs, profiles as memberships, multiple targets and per-device overrides. Do not use file paths as IDs.
9. File metadata retrieval must be cancellable, bounded and nonblocking. Never scan entire drives or network shares by default.
10. Treat imported profiles as untrusted ZIP/JSON: size limits, decompression limits, zip-slip checks, strict schemas, no executable contents/auto-runs, and transactional preview-and-commit.
11. Do not make work/private files or telemetry appear in public GitHub fixtures/logs. Use generated dummy data.
12. Before declaring V1 complete, attach evidence from an actual Windows 11 machine for Shell launch, tray, global hotkey, Dock, DPI/multimonitor, portable build and target-system permissions.

## Mandatory multi-agent coordination (explicit user authorization)
Follow `docs/MULTI_AGENT_WORKFLOW.md`, `docs/CODEX_RUNTIME_SETUP.md`, `.codex/config.toml`, and `.codex/agents/*.toml`. Spawn Core/UI/QA specialized Codex subagents explicitly where independent work exists; cap active subagents at 3 plus coordinator. Default family is **GPT-6 Luna**, coordinator **Medium**, Core/UI/QA/Release **Medium**, narrow read-only Research **Low**. Role instructions and non-overlapping worktrees/branches are binding. The coordinator alone merges, edits canonical status and declares releases; agent completion must include tested code and evidence. If runtime cannot spawn subagents, disclose the fallback and use independent branches/tasks with equivalent review; do not pretend parallel work is active. No runtime or Windows execution environment? Persist status, identify blocker, continue what is feasible, and never promise unseen background execution.

### V1 and full-program completion
Once `docs/NOTIFICATIONS_AND_RELEASE_GATES.md` conditions actually pass, publish/download-link the Windows V1 artifact and **send user a distinct V1-ready notification**. Then proceed through SW-013..SW-025 until the full advanced roadmap is verified. Do not stop at V1, do not send a false success notification, and do not claim scripts themselves schedule Codex sessions.

## Execution protocol
- Begin each milestone by reading issue acceptance, existing source/tests, and dependencies.
- Write a failing focused test when practical; implement; run test; record outputs.
- Commit incrementally and update `backlog/STATUS.md` with actual evidence and unverified blockers.
- A milestone is complete only after acceptance criteria pass; do not convert missing Windows evidence into simulated success.
- Keep user-visible UI accessible: keyboard navigation, contrast, focus rings, reduced motion, 100–200% DPI, light/dark contrast.
- Explicitly test on a restricted **non-admin Windows account**, where software policy permits execution.

## Commands after scaffolding
Prefer exact version-locked package manager scripts in the created `package.json` and `Cargo.lock`. Expected commands after Codex implements them:
`npm ci`, `npm run lint`, `npm run typecheck`, `npm run test`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`, `npm run tauri build` (actual names may differ; document exact commands and script equivalents). Current GitHub Actions Windows CI verifies these project commands successfully as of main commit `98b1432` (2026-10-22); re-run on the current head and distinguish hosted-runner automated checks from actual Windows 11 GUI acceptance.