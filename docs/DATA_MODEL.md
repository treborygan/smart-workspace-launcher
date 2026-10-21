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

Usage history (SW-015) is profile-local. `usage_settings` controls recording, retention (30/90/365 days or indefinite), and excluded resource IDs. `launch_request_guard` is an independent replay ledger containing only request/resource/target/profile IDs, source, UTC time, outcome, error code, duration, and a hashed intent; it has no path, arguments, or free-form message and survives analytics retention/purge. `launch_events` contains only events the user has chosen to retain. Stop-recording suppresses new event rows without clearing existing rows; explicit purge and retention remove event rows while retaining replay guards. Terminal sequence run records are compacted when history is disabled, purged, or expired; in-progress sequence snapshots remain only for crash-safe resume.

Resource relationships (SW-016) are profile-local undirected edges between two distinct resources already assigned to that profile. Each edge has a stable UUID and a short user label; reversed endpoint pairs are duplicates. They are descriptive links only and never execute an action. Workspace snapshots are named checkpoints of dashboard and dock layouts plus the resource IDs present at capture time. Restoring updates layouts for resources that still belong to the profile, reports missing references, and does not delete resources, memberships, or cards. Snapshots are local profile data and do not launch resources.

Usage queries and summaries share profile, inclusive UTC range, source, status, and resource filters. Pagination orders by UTC time then request ID for stable boundaries. CSV usage export includes request/resource IDs, UTC time, source, status, and duration only; it excludes paths, arguments, user-controlled resource names, and free-form failure text. Launch history represents requests recorded by this application on this device, not files opened in other applications.
