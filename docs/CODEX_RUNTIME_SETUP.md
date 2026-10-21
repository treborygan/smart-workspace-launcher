# Codex runtime initialization

1. Open repository `treborygan/smart-workspace-launcher` in a connected Codex execution environment with GitHub write permissions.
2. Confirm the Codex client's actual supported model identifiers. Prefer GPT-6 Luna Medium for coordinator, Core/UI/QA/Release; Luna Low for research. Do not silently substitute models.
3. Trust/load `.codex/config.toml`, confirm agent configs and concurrency capacity (three active subagents plus coordinator), and **explicitly spawn** agents for bounded assignments.
4. Confirm working Git executable, Node.js, Rust, local project build/test toolchain, and separately a real approved Windows 11 x64 runtime with WebView2 and MSVC build tooling.
5. Supply `CODEX_KICKOFF.md` to coordinator. It should read the full repository docs and initiate implementation rather than provide another plan.
6. In each pause or external blocker, commit state and use `CODEX_RESUME.md` to restart.
7. A repository and configuration do not keep Codex sessions running. Persistent autonomous scheduling must be configured in an actual supported runtime separately. This handoff cannot do that on its own.
8. Deliver V1-ready notice only upon verified release asset and Windows tests. GitHub notifications also depend on the user's Watch settings; this must not be presented as guaranteed from a spec.
