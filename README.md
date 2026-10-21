# Smart Workspace Launcher

**Status: specification and Codex multi-agent handoff — NOT a built application.**

A Windows 11 x64 desktop workspace manager with a resizable drag-and-drop dashboard, a keyboard Quick Launcher, an optional floating Dock, local resource metadata and launch analytics, editable themes, and portable profile export/import.

## Selected architecture
- Tauri 2 + React + TypeScript + Rust + SQLite
- Local-first, no admin installation requirement; Windows 11 verification mandatory
- GPT-6 Luna coordinator on Medium; specialized core, UI and QA subagents on Medium; research on Low
- GitHub-first feature branches, scoped pull requests, independent QA evidence
- V1 is a tested-release milestone; continue through V1.1, V1.2 and V2.0
- No GitHub Actions without explicit user approval

## Codex entrypoint
Read [AGENTS.md](AGENTS.md), [CODEX_KICKOFF.md](CODEX_KICKOFF.md), and [CODEX_RESUME.md](CODEX_RESUME.md).
Read the detailed multi-agent workflow at [docs/MULTI_AGENT_WORKFLOW.md](docs/MULTI_AGENT_WORKFLOW.md).
Do not claim a downloadable Windows V1 release before real Windows tests and a GitHub release asset with SHA256.

## Repository implementation state
No application code, built EXE, passing Windows tests or currently running Codex agent is evidenced by this bootstrap.
