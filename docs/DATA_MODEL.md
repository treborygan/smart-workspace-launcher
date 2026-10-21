# Data model and contracts

SQLite canonical entities:
- resources(id UUID PK, type, display_name, description, icon, default_action, created_utc, updated_utc)
- resource_targets(id UUID PK, resource_id FK, kind, canonical_uri/path, device_selector, priority, arguments_json, working_directory, active)
- profiles(id UUID PK, name, theme_id, settings_json, created_utc, updated_utc)
- profile_resources(profile_id FK, resource_id FK, group_id, favorite, sort_position, overrides_json)
- groups(id UUID PK, profile_id FK, parent_id, name, ordering)
- tags(id UUID PK, name), resource_tags(resource_id, tag_id)
- layouts(id UUID PK, profile_id, surface, layout_json, version)
- dock_items(profile_id, resource_id, position, settings_json)
- launch_events(id UUID PK, request_id UNIQUE, resource_id FK, target_id, profile_id, device_id, source, launched_at_utc, status, error_code, duration_ms)
- metadata_cache(resource_id, target_id, inspected_utc, file_size_bytes, created_utc, modified_utc, availability, provenance, error_code)
- device_mappings(id, device_id, source_prefix, target_prefix, approved)
- schema_migrations(version PK, checksum, applied_utc)

All timestamps UTC in storage, localized for display. One request ID leads to one persistent launch event, with updates to that event if lifecycle has multiple outcomes. Missing resource/target is still a failed launch attempt if valid resource ID supplied.

Resource IDs never derive from paths, file size or name. A resource can belong to many profiles, multiple targets and groups; no per-profile re-creation required. Cascades and deletion/retention are explicit. Launch statistics must not overstate Windows Shell handoff as document load success.

Manual .swprofile export contains manifest with schemaVersion, application identity, exportedAt, selected profile and resources/layout/pins, optionally history (default OFF) and icon assets. Credentials/tokens excluded. Use contracts/*.schema.json and fixtures/ for detailed validation. Do not silently import executable actions or external connector secrets. 

Workspace action sequences (SW-014) are profile-scoped named records with 1–20 ordered steps. Each step stores a stable ID and references an existing resource in that profile plus an optional active target owned by the same resource. No command text, executable path, arbitrary arguments, conditions, or scripts are stored. Runs are explicitly requested by the user, use the standard validated launcher once per step, persist normal launch events, and stop after the first failed Shell handoff. A unique run request ID stores progress/results so retries return the prior result without launching again. Imported sequence definitions remain inert until a user explicitly runs them.
