# Security, privacy and policy controls

- Local-first, no external telemetry. Do not read document contents for metadata; never scan whole drives or network shares by default. Per-resource user choice for monitoring and retention.
- Windows corporate restrictions must be respected. Portable executable still may be blocked by WDAC/AppLocker/Defender. Do not bypass security controls.
- Public repository contains synthetic only fixtures and redacted logs. No employer documents, drive credentials, local personal paths, tokens, secrets or session cookies.
- Reject imported untrusted ZIPs violating size, ratio, decompressed limits, path traversal, absolute path/drive extraction or JSON schema. No import-triggered action execution. Preview merge/replace, back up and atomically commit; separate executable scripts require explicit approval.
- Launch URLs allow approved schemes; default HTTPS. No unbounded shell commands. EXE args must be structured and escaped. Native IPC enforces authorization independent of React.
- Remote companion, cloud sync, extension SDK and AI integrations are opt-in advanced features, disabled by default with explicit permission and secure secret handling. Never auto-connect work data to private clouds.
- Export excludes credentials, tokens and launch history unless expressly requested.
- User can disable history, exclude resources, purge records, or keep Work/Personal in isolated databases if needed.
- Security tests: zip-slip, decompression bomb, invalid URL protocols, injection args, symlink/out-of-scope folder traversal, changed path after check, concurrent imports and crash recovery.
- Workspace sequences store only bounded references to resources already authorized in the selected profile. Every run step is revalidated by the regular launch path; sequences cannot carry shell commands, scripts, or arbitrary arguments. Import may create inert sequence definitions but never invokes them. Sequence run request IDs are idempotent so replay cannot repeat completed launch side effects.
