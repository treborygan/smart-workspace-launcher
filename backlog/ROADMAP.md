# GitHub-first implementation roadmap

**V1 release is mandatory but not the final goal. Continue through V1.1, V1.2 and V2.0.** No issue is complete based solely on a document or mock.

## V1.0 dependencies and releases
- SW-001 repo/toolchain -> SW-002 SQLite and domain IDs.
- SW-002 -> SW-003 core resource CRUD/launch; SW-004 profiles/layout.
- SW-003 -> SW-005 search/hotkey; SW-007 metadata/analytics.
- SW-004 -> SW-006 editable dashboard; SW-008 profile import/export.
- SW-003+SW-004 -> SW-009 Dock (also requires SW-005).
- SW-006 -> SW-010 Explorer/Inspector/theme.
- SW-007/008/009/010 -> SW-011 integration/hardening.
- SW-011 -> SW-012 real Windows tests, portable release and V1-ready notice.

## Advanced roadmap
- **V1.1:** SW-013 dynamic shortcuts/smart collections; SW-014 workspace action sequences; SW-015 usage export/retention; SW-016 relationship graph/snapshots (after 013/014).
- **V1.2:** SW-017 duplicate/move assistant (after 013); SW-018 Start Menu/directory/bookmark importer (after 013); SW-019 widget templates (after 016); SW-020 device mappings (after 017/018).
- **V2.0:** SW-021 permissioned extension SDK (after 019/020); SW-022 optional selective sync (after 020/021); SW-023 optional authenticated remote companion (after 021); SW-024 optional AI organization (after 021); SW-025 integration/native regression/final release (after 022/023/024).

Root coordinator owns contract freeze, issue dependency choice, integrations/PR merges, accurate status and milestone notifications. Core and UI develop in parallel only after stable types; QA independently reviews and verifies. Windows release has separate native gate. See docs/MULTI_AGENT_WORKFLOW.md.
