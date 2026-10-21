# Traceability matrix

| Requirement | Primary implementation | Tests | Issues |
|---|---|---|---|
| Default Shell file/URL opening | Rust LaunchService | W-001..W-005 | SW-003 |
| Global search, hotkey | SearchService + Quick Launch | W-006, search performance | SW-005 |
| Drag/resize grid | Dashboard + layouts | W-010, frontend tests | SW-006 |
| Profiles/themes | ProfileService and UI | W-011, UI tests | SW-004, SW-010 |
| Floating Dock | Separate Tauri window | W-007..W-009 | SW-009 |
| File metadata/usage analytics | Async metadata + immutable events | W-012, event idempotency | SW-007 |
| Portable profile safety | Preview + transactional import | W-011, malicious fixture tests | SW-008 |
| Windows distributable | Native build + QA | W-001..W-014 | SW-011, SW-012 |
| Dynamic shortcuts/actions | Resource resolver/workspace runner | Regression | SW-013, SW-014 |
| Extensions and opt-in integrations | Capability-restricted SDK | Security/regression | SW-021..SW-025 |
