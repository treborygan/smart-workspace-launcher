# Resume Smart Workspace Launcher implementation

## Checkpoint — 2026-10-22 02:48 UTC

Repository `treborygan/smart-workspace-launcher`, coordinator `/workspace/sw020-core`, branch `feature/sw-021-permissioned-extensions`, current commit `b47e916`. Canonical live progress is in `backlog/AGENT_STATUS.json` and `backlog/STATUS.md`. Host is Debian 13 x86_64; no real Windows 11 x64 or PowerShell verification is available. V1/SW-012 remains BLOCKED.

## Active SW-021 work

SW-021 issue #21 and draft PR #47 are incomplete. The earlier integrated code is registry/policy only (schema v12) and must not be represented as a working runtime. A separately reviewed sandbox contract was frozen and pushed in `2ca5ea4`; contract QA passed. It defines `.swext` package bounds and exact entries; content-hash-bound preview/install; duplicate ID removal policy; Wasm ABI, no imports/no start section, explicit run only, fuel/memory/recursion/stack/input/output bounds; typed capability/data pairing; bounded profile metadata and suggestions; and output restrictions. Detailed contract: `docs/EXTENSION_SDK.md`, `contracts/extension_invocation.schema.json`, `contracts/extension_result.schema.json`.

The UI implementation is integrated and independently QA-approved at `4041757` (typecheck, 137 tests, production build and diff check pass). Core implementation is active in `/workspace/sw021-runtime-core` on `feature/sw-021-runtime-core`; it owns only `apps/desktop/src-tauri/**`. UI implementation is active in `/workspace/sw021-runtime-ui` on `feature/sw-021-runtime-ui`; it owns only `apps/desktop/src/**` and tests. Do not modify shared contracts, package manifests, status files, or integration branch from those agents. Root owns integration and canonical state. After agents finish: inspect both diffs, independent QA, cherry-pick in dependency order, run frontend checks, Rust formatting, focused tests if possible, handoff/release checks, update PR #47 and status. Core runtime is integrated and independently QA-approved at `71eded8`. Opaque one-time five-minute preview tokens are path+digest bound; package/output/pointer/profile and count/byte truncation tests are authored. Frontend, formatting, metadata, handoff and isolated Wasmi harness checks pass. Latest extended security QA PASS on Core d23d008: Wasm structural limits, single compilation on explicit run, and no DB mutex during package parsing or guest execution; previous token/ZIP/output/truncation fixes remain. PR #47 remains draft and SW-021 in progress because app Cargo tests are BLOCKED before crate compilation by missing GLib/GObject >=2.70 pkg-config files. Keep SW-022..SW-024 gated until app tests run. Rust application compilation currently cannot run here because GLib/GObject development files are absent; do not claim Rust tests passed if not executed.

Latest `python scripts/verify_handoff.py` and `git diff --check` passed on contract commit. Current UI typecheck is expected to fail until UI/Core catch up: old `extension_registry_register` command and an old test fixture omitting `moduleSha256` remain.

## Review and release tracking

At 2026-10-22 02:47 UTC, GitHub review and inline-thread APIs returned empty for both PR #38 (SW-014) and #41 (SW-015). Independent read-only QA previously found no actionable blocker; #41 has a non-blocking shared database lock duration concern during large CSV export. Do not merge stacked changes out of dependency order.

PR #47 stays draft and stacked on PR #46/SW-020; issue #21 stays open until the sandbox runtime and permission behavior are implemented and verified. No Windows runner, PowerShell, MinGW, WebView2/packaging, or Windows acceptance is available. Never claim a working EXE or V1 release. No GitHub Actions.

## Continue

Implement and QA the SW-021 sandbox runtime, integrate Core and UI, update PR/status/checkpoints, then continue later backlog tasks in dependency order. Windows V1 remains BLOCKED until real Windows 11 x64 acceptance.
