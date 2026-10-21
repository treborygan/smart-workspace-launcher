# GitHub repository setup

Repository: https://github.com/treborygan/smart-workspace-launcher
Default branch: main. GitHub is the source of truth for branches, PRs, design documents and issue tracking. The supplied source handoff is planning content only, not a runnable application.

- Preserve AGENTS.md, CODEX_KICKOFF.md, CODEX_RESUME.md, .codex, .github, docs, contracts, fixtures and backlog.
- Use feature branches and pull requests for implementation, no GitHub Actions unless user explicitly approves. Test locally or in an approved Windows/other real runtime. No force-push to main.
- Assign and execute GitHub issues in dependency order, see backlog/ROADMAP.md.
- Subagents work in independent branches/worktrees, share typed contract snapshot, and report exact commits and results.
- Do not upload corporate data or actual credentials to this public repository. Use generated dummy fixtures.
- Separate cross-platform build validation from native Windows acceptance. The latter is necessary for V1 release readiness.
- A configured Codex client does not automatically imply an active remote task; confirm a connected environment before claiming workers run.
