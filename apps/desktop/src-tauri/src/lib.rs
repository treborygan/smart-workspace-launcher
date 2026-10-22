use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::AtomicBool,
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};
use thiserror::Error;
use url::Url;
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("resource not found")]
    NotFound,
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("profile package error: {0}")]
    Package(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTarget {
    pub id: String,
    pub kind: String,
    pub canonical_uri: String,
    pub priority: i64,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: String,
    pub resource_type: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_action: String,
    pub created_utc: String,
    pub updated_utc: String,
    #[serde(default)]
    pub targets: Vec<ResourceTarget>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub category: Option<String>,
    pub favorite: bool,
    pub launch_count: i64,
    pub last_launched_utc: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTargetInput {
    pub id: Option<String>,
    pub kind: String,
    pub canonical_uri: String,
    pub priority: i64,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInput {
    pub resource_type: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_action: Option<String>,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<ResourceTargetInput>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub category: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePatch {
    pub resource_type: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_action: Option<String>,
    pub targets: Option<Vec<ResourceTargetInput>>,
    pub tags: Option<Vec<String>>,
    pub aliases: Option<Vec<String>>,
    pub profile_id: Option<String>,
    pub category: Option<String>,
    pub favorite: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceDuplicateRequest {
    pub profile_id: String,
    pub resource_id: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRelocationPreviewRequest {
    pub profile_id: String,
    pub source_prefix: String,
    pub destination_prefix: String,
    #[serde(default)]
    pub resource_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRelocationChange {
    pub resource_id: String,
    pub target_id: String,
    pub from_uri: String,
    pub to_uri: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRelocationPreview {
    pub profile_id: String,
    pub source_prefix: String,
    pub destination_prefix: String,
    #[serde(default)]
    pub resource_ids: Option<Vec<String>>,
    pub preview_token: String,
    pub changes: Vec<ResourceRelocationChange>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRelocationResult {
    pub updated_target_count: usize,
    pub updated_resource_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub request_id: String,
    pub resource_id: String,
    pub target_id: Option<String>,
    pub profile_id: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub theme_id: String,
    pub settings_json: String,
    pub created_utc: String,
    pub updated_utc: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileThemeInput {
    pub mode: String,
    pub accent: String,
    pub density: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartCollectionFilters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favorite_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SmartCollection {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    #[serde(default)]
    pub filters: SmartCollectionFilters,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicShortcut {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

const MAX_SMART_COLLECTIONS_PER_PROFILE: i64 = 100;
const MAX_DYNAMIC_SHORTCUTS_PER_PROFILE: i64 = 100;
const MAX_WORKSPACE_SEQUENCES_PER_PROFILE: i64 = 100;
const MAX_WORKSPACE_SEQUENCE_STEPS: usize = 20;
const MAX_RELOCATION_RESOURCE_IDS: usize = 5_000;
const MAX_RELOCATION_CHANGES: usize = 10_000;
const MAX_RESOURCE_RELATIONSHIPS_PER_PROFILE: i64 = 500;
const MAX_WORKSPACE_SNAPSHOTS_PER_PROFILE: i64 = 100;
const MAX_WORKSPACE_SNAPSHOT_RESOURCE_IDS: usize = 5_000;
const MAX_WORKSPACE_SNAPSHOT_BYTES: usize = 1_048_576;
const PROFILE_ARCHIVE_MAX_COMPRESSED: u64 = 32 * 1024 * 1024;
const PROFILE_ARCHIVE_MAX_TOTAL: u64 = 128 * 1024 * 1024;
const PROFILE_ARCHIVE_MAX_ENTRY: u64 = 32 * 1024 * 1024;
const PROFILE_ARCHIVE_MAX_RATIO: u64 = 100;
const PROFILE_ARCHIVE_MAX_ENTRIES: usize = 2503;
const PROFILE_REQUIRED_MEMBERS: [&str; 4] = [
    "manifest.json",
    "profile.json",
    "resources.json",
    "layouts.json",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageManifest {
    format: String,
    schema_version: u32,
    exported_at_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_version: Option<String>,
    profile_file: String,
    resources_file: String,
    layouts_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    history_file: Option<String>,
    #[serde(default)]
    include_history: bool,
    #[serde(default)]
    assets: Vec<ProfilePackageAsset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageAsset {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageDocument {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    theme: ProfilePackageTheme,
    settings: serde_json::Value,
    memberships: Vec<String>,
    #[serde(default)]
    membership_details: Vec<ProfilePackageMembership>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    smart_collections: Vec<ProfilePackageSmartCollection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dynamic_shortcuts: Vec<ProfilePackageDynamicShortcut>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspace_sequences: Vec<ProfilePackageWorkspaceSequence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageSmartCollection {
    id: String,
    name: String,
    filters: SmartCollectionFilters,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageDynamicShortcut {
    id: String,
    name: String,
    resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageWorkspaceSequence {
    id: String,
    name: String,
    steps: Vec<ProfilePackageWorkspaceSequenceStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageWorkspaceSequenceStep {
    id: String,
    resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSequenceStep {
    pub id: String,
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSequence {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub steps: Vec<WorkspaceSequenceStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSequenceStepResult {
    pub step_id: String,
    pub resource_id: String,
    pub target_id: Option<String>,
    pub result: LaunchResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSequenceRunResult {
    pub sequence_id: String,
    pub status: String,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub stopped_at_step_id: Option<String>,
    pub steps: Vec<WorkspaceSequenceStepResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageTheme {
    mode: String,
    accent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    density: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageMembership {
    resource_id: String,
    favorite: bool,
    category: Option<String>,
    sort_position: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileResourcesDocument {
    schema_version: u32,
    resources: Vec<ProfilePackageResource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageResource {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(rename = "type")]
    resource_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    targets: Vec<ProfilePackageTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageTarget {
    id: String,
    kind: String,
    value: String,
    priority: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_label: Option<String>,
    #[serde(default)]
    argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_directory: Option<String>,
    #[serde(default = "default_active")]
    active: bool,
}

fn default_active() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileLayoutsDocument {
    schema_version: u32,
    layouts: Vec<ProfilePackageLayout>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageLayout {
    #[serde(default)]
    surface: Option<String>,
    breakpoint: String,
    cards: Vec<ProfilePackageLayoutCard>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageLayoutCard {
    resource_id: String,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    #[serde(rename = "type")]
    card_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileHistoryDocument {
    schema_version: u32,
    events: Vec<ProfilePackageHistoryEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilePackageHistoryEvent {
    request_id: String,
    resource_id: String,
    target_id: Option<String>,
    source: String,
    status: String,
    launched_at_utc: String,
    error_code: Option<String>,
    message: Option<String>,
    duration_ms: Option<i64>,
}

struct ValidatedProfilePackage {
    manifest: ProfilePackageManifest,
    profile: ProfilePackageDocument,
    resources: ProfileResourcesDocument,
    layouts: ProfileLayoutsDocument,
    history: Option<ProfileHistoryDocument>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileImportPreview {
    pub package_sha256: String,
    pub preview_token: String,
    pub profile_id: String,
    pub profile_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_profile_id: Option<String>,
    pub resource_count: usize,
    pub layout_count: usize,
    pub history_event_count: usize,
    pub smart_collection_count: usize,
    pub dynamic_shortcut_count: usize,
    pub workspace_sequence_count: usize,
    pub profile_id_collision: bool,
    pub profile_name_collision: bool,
    pub resource_id_collisions: Vec<String>,
    pub target_id_collisions: Vec<String>,
    pub resource_conflicts: Vec<ProfileResourceConflictPreview>,
    pub smart_collection_id_collisions: Vec<String>,
    pub smart_collection_name_collisions: Vec<String>,
    pub dynamic_shortcut_id_collisions: Vec<String>,
    pub dynamic_shortcut_name_collisions: Vec<String>,
    pub workspace_sequence_id_collisions: Vec<String>,
    pub workspace_sequence_name_collisions: Vec<String>,
    pub asset_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResourceConflictPreview {
    pub resolution: String,
    pub imported: ProfileResourceSnapshot,
    pub existing: ProfileResourceSnapshot,
    pub proposed: ProfileResourceSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResourceSnapshot {
    pub id: String,
    pub name: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub targets: Vec<ProfileTargetSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTargetSnapshot {
    pub id: String,
    pub kind: String,
    pub value: String,
    pub priority: i64,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileImportResult {
    pub profile_id: String,
    pub profile_name: String,
    pub imported_resources: usize,
    pub reused_resources: usize,
    pub imported_layouts: usize,
    pub imported_history_events: usize,
    pub imported_workspace_sequences: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExportResult {
    pub path: String,
    pub profile_name: String,
    pub resource_count: usize,
    pub history_event_count: usize,
}

fn package_error(message: impl Into<String>) -> AppError {
    AppError::Package(message.into())
}

fn safe_archive_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.starts_with('/')
        && !name.starts_with('~')
        && !name.contains(':')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn read_zip_member<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, AppError> {
    let mut member = archive
        .by_name(name)
        .map_err(|_| package_error(format!("missing or unreadable ZIP member: {name}")))?;
    if member.size() > PROFILE_ARCHIVE_MAX_ENTRY {
        return Err(package_error(format!(
            "ZIP member exceeds size limit: {name}"
        )));
    }
    let mut bytes = Vec::with_capacity(member.size() as usize);
    member
        .take(PROFILE_ARCHIVE_MAX_ENTRY + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| package_error(format!("could not read ZIP member: {name}")))?;
    if bytes.len() as u64 > PROFILE_ARCHIVE_MAX_ENTRY {
        return Err(package_error(format!(
            "ZIP member exceeds size limit: {name}"
        )));
    }
    Ok(bytes)
}

fn parse_package_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    name: &str,
) -> Result<T, AppError> {
    serde_json::from_slice(bytes).map_err(|error| {
        package_error(format!("{name} does not match the package schema: {error}"))
    })
}

fn valid_uuid(value: &str, field: &str) -> Result<(), AppError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| package_error(format!("{field} must be a UUID")))
}

fn valid_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_package_fields(package: &ValidatedProfilePackage) -> Result<(), AppError> {
    let manifest = &package.manifest;
    if manifest.format != "swprofile"
        || manifest.schema_version != 1
        || manifest.profile_file != "profile.json"
        || manifest.resources_file != "resources.json"
        || manifest.layouts_file != "layouts.json"
        || manifest
            .history_file
            .as_deref()
            .is_some_and(|name| name != "history.json")
        || manifest.include_history != package.history.is_some()
    {
        return Err(package_error("unsupported profile package manifest"));
    }
    if chrono::DateTime::parse_from_rfc3339(&manifest.exported_at_utc).is_err() {
        return Err(package_error("invalid package export timestamp"));
    }
    if package.profile.schema_version != 1
        || package.resources.schema_version != 1
        || package.layouts.schema_version != 1
        || package
            .history
            .as_ref()
            .is_some_and(|history| history.schema_version != 1)
    {
        return Err(package_error("unsupported profile package schema version"));
    }
    valid_uuid(&package.profile.id, "profile id")?;
    if package.profile.name.trim().is_empty() || package.profile.name.chars().count() > 80 {
        return Err(package_error(
            "profile name must contain 1 to 80 characters",
        ));
    }
    if package
        .profile
        .description
        .as_ref()
        .is_some_and(|value| value.chars().count() > 1024)
    {
        return Err(package_error("profile description exceeds 1024 characters"));
    }
    if !["light", "dark", "system"].contains(&package.profile.theme.mode.as_str())
        || !valid_hex_color(&package.profile.theme.accent)
        || package
            .profile
            .theme
            .density
            .as_ref()
            .is_some_and(|density| {
                !["compact", "balanced", "comfortable"].contains(&density.as_str())
            })
    {
        return Err(package_error("invalid profile theme"));
    }
    if !package.profile.settings.is_object()
        || package
            .profile
            .settings
            .as_object()
            .is_some_and(|map| map.len() > 50)
    {
        return Err(package_error(
            "profile settings must be an object with at most 50 properties",
        ));
    }
    if package.resources.resources.len() > 10_000
        || package.profile.memberships.len() > 10_000
        || package.layouts.layouts.len() > 25
        || package.profile.smart_collections.len() > MAX_SMART_COLLECTIONS_PER_PROFILE as usize
        || package.profile.dynamic_shortcuts.len() > MAX_DYNAMIC_SHORTCUTS_PER_PROFILE as usize
        || package.profile.workspace_sequences.len() > MAX_WORKSPACE_SEQUENCES_PER_PROFILE as usize
    {
        return Err(package_error("package exceeds item count limit"));
    }
    let mut resource_ids = std::collections::HashSet::new();
    let mut target_ids = std::collections::HashSet::new();
    for resource in &package.resources.resources {
        valid_uuid(&resource.id, "resource id")?;
        if !resource_ids.insert(resource.id.clone()) {
            return Err(package_error("resource IDs must be unique"));
        }
        if resource.schema_version != 1
            || resource.name.trim().is_empty()
            || resource.name.chars().count() > 256
            || resource
                .description
                .as_ref()
                .is_some_and(|value| value.chars().count() > 4096)
            || !["file", "folder", "app", "shortcut", "url"]
                .contains(&resource.resource_type.as_str())
            || resource.targets.is_empty()
            || resource.targets.len() > 8
            || resource.tags.len() > 64
            || resource.aliases.len() > 32
        {
            return Err(package_error("invalid resource fields or limits"));
        }
        for (field, values, limit) in [
            ("tag", &resource.tags, 80usize),
            ("alias", &resource.aliases, 80usize),
        ] {
            let mut unique = std::collections::HashSet::new();
            if values
                .iter()
                .any(|value| value.chars().count() > limit || !unique.insert(value.to_lowercase()))
            {
                return Err(package_error(format!(
                    "invalid or duplicate resource {field}"
                )));
            }
        }
        for target in &resource.targets {
            valid_uuid(&target.id, "target id")?;
            if !target_ids.insert(target.id.clone()) {
                return Err(package_error("target IDs must be unique"));
            }
            if !["file", "folder", "app", "shortcut", "url"].contains(&target.kind.as_str())
                || target.value.trim().is_empty()
                || target.value.chars().count() > 4096
                || !(0..=1000).contains(&target.priority)
                || target.argv.len() > 64
                || target.argv.iter().any(|arg| arg.chars().count() > 4096)
                || target
                    .working_directory
                    .as_ref()
                    .is_some_and(|value| value.chars().count() > 4096)
                || target
                    .device_label
                    .as_ref()
                    .is_some_and(|value| value.chars().count() > 128)
            {
                return Err(package_error("invalid target fields or limits"));
            }
            if target.kind == "url" {
                let parsed =
                    Url::parse(&target.value).map_err(|_| package_error("invalid URL target"))?;
                if parsed.scheme() != "https" || parsed.host_str().is_none() {
                    return Err(package_error("only HTTPS URL targets are allowed"));
                }
            }
        }
    }
    if resource_ids.iter().any(|id| target_ids.contains(id)) {
        return Err(package_error(
            "resource and target IDs must be globally unique",
        ));
    }
    let known_resources: std::collections::HashSet<String> = resource_ids.clone();
    let mut membership_ids = std::collections::HashSet::new();
    for id in &package.profile.memberships {
        valid_uuid(id, "membership resource id")?;
        if !known_resources.contains(id) || !membership_ids.insert(id.clone()) {
            return Err(package_error(
                "profile memberships must be unique known resources",
            ));
        }
    }
    let mut smart_collection_ids = std::collections::HashSet::new();
    let mut smart_collection_names = std::collections::HashSet::new();
    for collection in &package.profile.smart_collections {
        valid_uuid(&collection.id, "smart collection id")?;
        if !smart_collection_ids.insert(collection.id.as_str())
            || collection.id == package.profile.id
            || resource_ids.contains(&collection.id)
            || target_ids.contains(&collection.id)
        {
            return Err(package_error(
                "smart collection IDs must be unique across package entities",
            ));
        }
        validate_smart_collection_fields(&collection.id, &collection.name, &collection.filters)
            .map_err(|error| package_error(error.to_string()))?;
        if !smart_collection_names.insert(collection.name.trim().to_lowercase()) {
            return Err(package_error(
                "smart collection names must be unique within the profile",
            ));
        }
    }
    let mut dynamic_shortcut_ids = std::collections::HashSet::new();
    let mut dynamic_shortcut_names = std::collections::HashSet::new();
    let package_targets: std::collections::HashMap<&str, (&str, bool)> = package
        .resources
        .resources
        .iter()
        .flat_map(|resource| {
            resource
                .targets
                .iter()
                .map(move |target| (target.id.as_str(), (resource.id.as_str(), target.active)))
        })
        .collect();
    for shortcut in &package.profile.dynamic_shortcuts {
        valid_uuid(&shortcut.id, "dynamic shortcut id")?;
        valid_uuid(&shortcut.resource_id, "dynamic shortcut resource id")?;
        if !dynamic_shortcut_ids.insert(shortcut.id.as_str())
            || shortcut.id == package.profile.id
            || resource_ids.contains(&shortcut.id)
            || target_ids.contains(&shortcut.id)
            || smart_collection_ids.contains(shortcut.id.as_str())
        {
            return Err(package_error(
                "dynamic shortcut IDs must be unique across package entities",
            ));
        }
        if shortcut.name.trim().is_empty()
            || shortcut.name.trim().chars().count() > 100
            || shortcut.name.chars().any(char::is_control)
            || !membership_ids.contains(&shortcut.resource_id)
        {
            return Err(package_error(
                "invalid dynamic shortcut name or resource membership",
            ));
        }
        if let Some(target_id) = shortcut.target_id.as_deref() {
            valid_uuid(target_id, "dynamic shortcut target id")?;
            if !package_targets
                .get(target_id)
                .is_some_and(|(resource_id, active)| {
                    *resource_id == shortcut.resource_id && *active
                })
            {
                return Err(package_error(
                    "dynamic shortcut target must be active and owned by its resource",
                ));
            }
        }
        if !dynamic_shortcut_names.insert(shortcut.name.trim().to_lowercase()) {
            return Err(package_error(
                "dynamic shortcut names must be unique within the profile",
            ));
        }
    }
    let mut sequence_ids = std::collections::HashSet::new();
    let mut sequence_names = std::collections::HashSet::new();
    let mut sequence_step_ids = std::collections::HashSet::new();
    let all_sequence_ids = package
        .profile
        .workspace_sequences
        .iter()
        .map(|sequence| sequence.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let all_sequence_step_ids = package
        .profile
        .workspace_sequences
        .iter()
        .flat_map(|sequence| sequence.steps.iter().map(|step| step.id.as_str()))
        .collect::<std::collections::HashSet<_>>();
    if all_sequence_ids.len() != package.profile.workspace_sequences.len()
        || all_sequence_step_ids.len()
            != package
                .profile
                .workspace_sequences
                .iter()
                .map(|sequence| sequence.steps.len())
                .sum::<usize>()
        || all_sequence_ids
            .iter()
            .any(|id| all_sequence_step_ids.contains(id))
    {
        return Err(package_error(
            "workspace sequence and step IDs must be unique",
        ));
    }
    for sequence in &package.profile.workspace_sequences {
        valid_uuid(&sequence.id, "workspace sequence id")?;
        if !sequence_ids.insert(sequence.id.as_str())
            || sequence.id == package.profile.id
            || resource_ids.contains(&sequence.id)
            || target_ids.contains(&sequence.id)
            || smart_collection_ids.contains(sequence.id.as_str())
            || dynamic_shortcut_ids.contains(sequence.id.as_str())
            || all_sequence_step_ids.contains(sequence.id.as_str())
        {
            return Err(package_error(
                "workspace sequence IDs must be unique across package entities",
            ));
        }
        let validated_steps = sequence
            .steps
            .iter()
            .map(|step| WorkspaceSequenceStep {
                id: step.id.clone(),
                resource_id: step.resource_id.clone(),
                target_id: step.target_id.clone(),
            })
            .collect::<Vec<_>>();
        validate_workspace_sequence_fields(&sequence.id, &sequence.name, &validated_steps)
            .map_err(|error| package_error(error.to_string()))?;
        if !sequence_names.insert(sequence.name.trim().to_lowercase()) {
            return Err(package_error(
                "workspace sequence names must be unique within the profile",
            ));
        }
        for step in &sequence.steps {
            if !sequence_step_ids.insert(step.id.as_str())
                || sequence_ids.contains(step.id.as_str())
                || step.id == package.profile.id
                || resource_ids.contains(&step.id)
                || target_ids.contains(&step.id)
                || smart_collection_ids.contains(step.id.as_str())
                || dynamic_shortcut_ids.contains(step.id.as_str())
                || all_sequence_ids.contains(step.id.as_str())
            {
                return Err(package_error(
                    "workspace sequence step IDs must be unique across package entities",
                ));
            }
            if !membership_ids.contains(&step.resource_id) {
                return Err(package_error(
                    "workspace sequence resource must be a profile member",
                ));
            }
            if let Some(target_id) = step.target_id.as_deref() {
                let target_is_active_and_owned =
                    package_targets
                        .get(target_id)
                        .is_some_and(|(resource_id, active)| {
                            *resource_id == step.resource_id && *active
                        });
                if !target_is_active_and_owned {
                    return Err(package_error(
                        "workspace sequence target must be active and owned by its resource",
                    ));
                }
            }
        }
    }
    let mut details = std::collections::HashSet::new();
    for membership in &package.profile.membership_details {
        valid_uuid(&membership.resource_id, "membership resource id")?;
        if !known_resources.contains(&membership.resource_id)
            || !membership_ids.contains(&membership.resource_id)
            || !details.insert(membership.resource_id.clone())
            || !(0..=1_000_000).contains(&membership.sort_position)
            || membership
                .category
                .as_ref()
                .is_some_and(|value| value.chars().count() > 80)
        {
            return Err(package_error("invalid membership metadata"));
        }
    }
    for layout in &package.layouts.layouts {
        if !["desktop", "tablet", "compact"].contains(&layout.breakpoint.as_str())
            || layout
                .surface
                .as_ref()
                .is_some_and(|surface| !["dashboard", "dock"].contains(&surface.as_str()))
            || layout.cards.len() > 10_000
        {
            return Err(package_error("invalid layout fields or limits"));
        }
        for card in &layout.cards {
            valid_uuid(&card.resource_id, "layout resource id")?;
            if !known_resources.contains(&card.resource_id)
                || !membership_ids.contains(&card.resource_id)
                || !(0..=11).contains(&card.x)
                || !(0..=9999).contains(&card.y)
                || !(1..=12).contains(&card.w)
                || card.x + card.w > 12
                || !(1..=100).contains(&card.h)
                || !["icon", "compact", "info", "group"].contains(&card.card_type.as_str())
            {
                return Err(package_error("invalid layout card"));
            }
        }
    }
    if let Some(history) = &package.history {
        if history.events.len() > 100_000 {
            return Err(package_error("history exceeds item count limit"));
        }
        let mut request_ids = std::collections::HashSet::new();
        for event in &history.events {
            valid_uuid(&event.request_id, "history request id")?;
            valid_uuid(&event.resource_id, "history resource id")?;
            if let Some(target_id) = event.target_id.as_deref() {
                valid_uuid(target_id, "history target id")?;
            }
            if chrono::DateTime::parse_from_rfc3339(&event.launched_at_utc).is_err() {
                return Err(package_error("invalid history event timestamp"));
            }
            if !known_resources.contains(&event.resource_id)
                || !request_ids.insert(event.request_id.clone())
                || event
                    .target_id
                    .as_deref()
                    .is_some_and(|id| !target_ids.contains(id))
                || ![
                    "dashboard",
                    "quick_launch",
                    "dock",
                    "explorer",
                    "sequence",
                    "other",
                ]
                .contains(&event.source.as_str())
                || !["requested", "handed_off", "failed"].contains(&event.status.as_str())
                || event
                    .error_code
                    .as_ref()
                    .is_some_and(|value| value.len() > 128)
                || event
                    .message
                    .as_ref()
                    .is_some_and(|value| value.chars().count() > 1024)
                || event
                    .duration_ms
                    .is_some_and(|value| !(0..=3_600_000).contains(&value))
            {
                return Err(package_error("invalid history event"));
            }
        }
    }
    Ok(())
}

fn read_profile_package(path: &Path) -> Result<ValidatedProfilePackage, AppError> {
    read_profile_package_with_digest(path).map(|(package, _)| package)
}

fn read_profile_package_with_digest(
    path: &Path,
) -> Result<(ValidatedProfilePackage, String), AppError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| package_error(format!("cannot inspect package: {error}")))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > PROFILE_ARCHIVE_MAX_COMPRESSED
    {
        return Err(package_error(
            "package must be a regular file under the compressed size limit",
        ));
    }
    let file =
        File::open(path).map_err(|error| package_error(format!("cannot open package: {error}")))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(PROFILE_ARCHIVE_MAX_COMPRESSED + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| package_error(format!("cannot read package: {error}")))?;
    if bytes.len() as u64 > PROFILE_ARCHIVE_MAX_COMPRESSED {
        return Err(package_error("package exceeds the compressed size limit"));
    }
    let package_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| package_error("not a valid ZIP profile package"))?;
    if archive.len() > PROFILE_ARCHIVE_MAX_ENTRIES {
        return Err(package_error("archive contains too many entries"));
    }
    let mut names = std::collections::HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| package_error("invalid ZIP directory"))?;
        let name = entry.name().to_owned();
        if !safe_archive_path(&name) || !names.insert(name) {
            return Err(package_error(
                "archive contains an unsafe or duplicate member path",
            ));
        }
        if entry.encrypted()
            || !matches!(
                entry.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err(package_error(
                "encrypted or unsupported compression ZIP member",
            ));
        }
        if [
            ".bat", ".cmd", ".com", ".dll", ".exe", ".js", ".msi", ".ps1", ".py", ".scr", ".sh",
            ".vbs", ".wsf",
        ]
        .iter()
        .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
        {
            return Err(package_error("executable archive members are not allowed"));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(package_error(
                "symbolic link archive members are not allowed",
            ));
        }
        if entry.size() > PROFILE_ARCHIVE_MAX_ENTRY {
            return Err(package_error(
                "archive member exceeds the uncompressed size limit",
            ));
        }
        if entry.size() > 0 && entry.compressed_size() == 0 {
            return Err(package_error("archive member has invalid compressed size"));
        }
        if entry.size()
            > entry
                .compressed_size()
                .saturating_mul(PROFILE_ARCHIVE_MAX_RATIO)
        {
            return Err(package_error("archive compression ratio exceeds the limit"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| package_error("archive size overflow"))?;
        if total > PROFILE_ARCHIVE_MAX_TOTAL {
            return Err(package_error(
                "archive exceeds total uncompressed size limit",
            ));
        }
    }
    for required in PROFILE_REQUIRED_MEMBERS {
        if !names.contains(required) {
            return Err(package_error("archive is missing a required JSON document"));
        }
    }
    let manifest: ProfilePackageManifest = parse_package_json(
        &read_zip_member(&mut archive, "manifest.json")?,
        "manifest.json",
    )?;
    if manifest.include_history != manifest.history_file.is_some() {
        return Err(package_error(
            "history flag and history file declaration do not match",
        ));
    }
    let mut expected_names: std::collections::HashSet<String> = PROFILE_REQUIRED_MEMBERS
        .iter()
        .map(|value| value.to_string())
        .collect();
    if manifest.include_history {
        expected_names.insert("history.json".into());
    }
    if manifest.assets.len() > 2500 {
        return Err(package_error("too many package assets"));
    }
    let mut asset_paths = std::collections::HashSet::new();
    for asset in &manifest.assets {
        if !valid_asset_name(&asset.path)
            || !asset_paths.insert(asset.path.as_str())
            || asset.sha256.len() != 64
            || !asset
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(package_error("invalid package asset declaration"));
        }
        expected_names.insert(asset.path.clone());
    }
    if names != expected_names {
        return Err(package_error("archive members do not match the manifest"));
    }
    for asset in &manifest.assets {
        let bytes = read_zip_member(&mut archive, &asset.path)?;
        if format!("{:x}", Sha256::digest(bytes)) != asset.sha256 {
            return Err(package_error(format!(
                "asset checksum mismatch: {}",
                asset.path
            )));
        }
    }
    let profile = parse_package_json(
        &read_zip_member(&mut archive, "profile.json")?,
        "profile.json",
    )?;
    let resources = parse_package_json(
        &read_zip_member(&mut archive, "resources.json")?,
        "resources.json",
    )?;
    let layouts = parse_package_json(
        &read_zip_member(&mut archive, "layouts.json")?,
        "layouts.json",
    )?;
    let history = if manifest.include_history {
        Some(parse_package_json(
            &read_zip_member(&mut archive, "history.json")?,
            "history.json",
        )?)
    } else {
        None
    };
    let package = ValidatedProfilePackage {
        manifest,
        profile,
        resources,
        layouts,
        history,
    };
    validate_package_fields(&package)?;
    Ok((package, package_sha256))
}

fn valid_asset_name(name: &str) -> bool {
    safe_archive_path(name)
        && (name.starts_with("assets/icons/") && (name.ends_with(".png") || name.ends_with(".ico")))
        && name.rsplit('/').next().is_some_and(|file| {
            file.len() == 68
                && file.as_bytes().get(64) == Some(&b'.')
                && file[..64]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

fn scrub_secret_settings(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let filtered = object
                .into_iter()
                .filter_map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                    let sensitive = [
                        "token",
                        "secret",
                        "password",
                        "credential",
                        "apikey",
                        "authorization",
                        "privatekey",
                    ]
                    .iter()
                    .any(|part| normalized.contains(part));
                    (!sensitive).then(|| (key, scrub_secret_settings(value)))
                })
                .collect();
            serde_json::Value::Object(filtered)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(scrub_secret_settings).collect())
        }
        other => other,
    }
}

fn profile_package_from_db(
    conn: &Connection,
    profile_id: &str,
    include_history: bool,
) -> Result<ValidatedProfilePackage, AppError> {
    let (name, theme_id, settings_json): (String, String, String) = conn
        .query_row(
            "SELECT name,theme_id,settings_json FROM profiles WHERE id=?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    let raw_settings: serde_json::Value = serde_json::from_str(&settings_json)?;
    let settings = scrub_secret_settings(raw_settings);
    let settings_map = settings.as_object();
    let accent = settings_map
        .and_then(|map| map.get("accent").or_else(|| map.get("accentColor")))
        .and_then(serde_json::Value::as_str)
        .filter(|value| valid_hex_color(value))
        .unwrap_or("#6858CE")
        .to_owned();
    let density = settings_map
        .and_then(|map| map.get("density"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| ["compact", "balanced", "comfortable"].contains(value))
        .map(str::to_owned);
    let theme = ProfilePackageTheme {
        mode: if ["light", "dark", "system"].contains(&theme_id.as_str()) {
            theme_id
        } else {
            "system".into()
        },
        accent,
        density,
    };

    let mut membership_stmt = conn.prepare("SELECT resource_id,favorite,category,sort_position FROM profile_resources WHERE profile_id=?1 ORDER BY sort_position,resource_id")?;
    let memberships = membership_stmt
        .query_map([profile_id], |row| {
            Ok(ProfilePackageMembership {
                resource_id: row.get(0)?,
                favorite: row.get::<_, i64>(1)? != 0,
                category: row.get(2)?,
                sort_position: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let membership_ids = memberships
        .iter()
        .map(|membership| membership.resource_id.clone())
        .collect::<Vec<_>>();
    let membership_set: std::collections::HashSet<String> =
        membership_ids.iter().cloned().collect();
    let mut resource_stmt = conn.prepare("SELECT r.id,r.resource_type,r.display_name,r.description,r.icon FROM resources r JOIN profile_resources pr ON pr.resource_id=r.id WHERE pr.profile_id=?1 ORDER BY pr.sort_position,r.display_name COLLATE NOCASE")?;
    let mut resources = Vec::new();
    for row in resource_stmt.query_map([profile_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })? {
        let (id, resource_type, name, description, icon) = row?;
        let mut resource = Resource {
            id,
            resource_type,
            display_name: name,
            description,
            icon,
            default_action: "open".into(),
            created_utc: String::new(),
            updated_utc: String::new(),
            targets: Vec::new(),
            tags: Vec::new(),
            aliases: Vec::new(),
            category: None,
            favorite: false,
            launch_count: 0,
            last_launched_utc: None,
        };
        load_resource_details(conn, &mut resource, Some(profile_id))?;
        let targets = resource
            .targets
            .into_iter()
            .map(|target| ProfilePackageTarget {
                id: target.id,
                kind: target.kind,
                value: target.canonical_uri,
                priority: target.priority,
                device_label: None,
                argv: target.arguments,
                working_directory: target.working_directory,
                active: target.active,
            })
            .collect();
        resources.push(ProfilePackageResource {
            schema_version: 1,
            id: resource.id,
            name: resource.display_name,
            resource_type: resource.resource_type,
            description: resource.description,
            tags: resource.tags,
            aliases: resource.aliases,
            icon: resource.icon,
            targets,
        });
    }
    let mut layouts = Vec::new();
    let mut layout_stmt = conn
        .prepare("SELECT surface,layout_json FROM layouts WHERE profile_id=?1 ORDER BY surface")?;
    for row in layout_stmt.query_map([profile_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (surface, json) = row?;
        let layout: DashboardLayout = serde_json::from_str(&json)?;
        let cards = layout
            .cards
            .into_iter()
            .filter(|card| membership_set.contains(&card.resource_id))
            .map(|card| ProfilePackageLayoutCard {
                resource_id: card.resource_id,
                x: card.x,
                y: card.y,
                w: card.w,
                h: card.h,
                card_type: card.card_type,
            })
            .collect();
        layouts.push(ProfilePackageLayout {
            surface: Some(surface),
            breakpoint: "desktop".into(),
            cards,
        });
    }
    let history = if include_history {
        let mut stmt = conn.prepare("SELECT le.request_id,le.resource_id,le.target_id,le.source,le.status,le.launched_at_utc,le.error_code,NULL,le.duration_ms FROM launch_events le JOIN profile_resources pr ON pr.resource_id=le.resource_id AND pr.profile_id=le.profile_id WHERE le.profile_id=?1 ORDER BY le.launched_at_utc DESC LIMIT 100000")?;
        let events = stmt
            .query_map([profile_id], |row| {
                Ok(ProfilePackageHistoryEvent {
                    request_id: row.get(0)?,
                    resource_id: row.get(1)?,
                    target_id: row.get(2)?,
                    source: row.get(3)?,
                    status: row.get(4)?,
                    launched_at_utc: row.get(5)?,
                    error_code: row.get(6)?,
                    message: row.get(7)?,
                    duration_ms: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Some(ProfileHistoryDocument {
            schema_version: 1,
            events,
        })
    } else {
        None
    };
    let mut smart_collection_stmt = conn.prepare(
        "SELECT id,name,filters_json FROM smart_collections WHERE profile_id=?1 ORDER BY name COLLATE NOCASE,id",
    )?;
    let smart_collections = smart_collection_stmt
        .query_map([profile_id], |row| {
            let filters_json: String = row.get(2)?;
            let filters = serde_json::from_str(&filters_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    filters_json.len(),
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(ProfilePackageSmartCollection {
                id: row.get(0)?,
                name: row.get(1)?,
                filters,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut dynamic_shortcut_stmt = conn.prepare(
        "SELECT id,name,resource_id,target_id FROM dynamic_shortcuts WHERE profile_id=?1 ORDER BY name COLLATE NOCASE,id",
    )?;
    let dynamic_shortcuts = dynamic_shortcut_stmt
        .query_map([profile_id], |row| {
            Ok(ProfilePackageDynamicShortcut {
                id: row.get(0)?,
                name: row.get(1)?,
                resource_id: row.get(2)?,
                target_id: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let workspace_sequences = workspace_sequence_list_on_conn(conn, profile_id)?
        .into_iter()
        .map(|sequence| ProfilePackageWorkspaceSequence {
            id: sequence.id,
            name: sequence.name,
            steps: sequence
                .steps
                .into_iter()
                .map(|step| ProfilePackageWorkspaceSequenceStep {
                    id: step.id,
                    resource_id: step.resource_id,
                    target_id: step.target_id,
                })
                .collect(),
        })
        .collect();
    let manifest = ProfilePackageManifest {
        format: "swprofile".into(),
        schema_version: 1,
        exported_at_utc: now_utc(),
        app_version: Some(env!("CARGO_PKG_VERSION").into()),
        profile_file: "profile.json".into(),
        resources_file: "resources.json".into(),
        layouts_file: "layouts.json".into(),
        history_file: include_history.then(|| "history.json".into()),
        include_history,
        assets: Vec::new(),
    };
    let package = ValidatedProfilePackage {
        manifest,
        profile: ProfilePackageDocument {
            schema_version: 1,
            id: profile_id.into(),
            name,
            description: None,
            theme,
            settings,
            memberships: membership_ids,
            membership_details: memberships,
            smart_collections,
            dynamic_shortcuts,
            workspace_sequences,
        },
        resources: ProfileResourcesDocument {
            schema_version: 1,
            resources,
        },
        layouts: ProfileLayoutsDocument {
            schema_version: 1,
            layouts,
        },
        history,
    };
    validate_package_fields(&package)?;
    Ok(package)
}

fn write_zip_file<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
) -> Result<(), AppError> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file(name, options)
        .map_err(|error| package_error(format!("cannot write package member: {error}")))?;
    writer
        .write_all(bytes)
        .map_err(|error| package_error(format!("cannot write package member: {error}")))?;
    Ok(())
}

fn finalize_export_no_clobber(temporary: &Path, destination: &Path) -> Result<(), AppError> {
    // A same-directory hard link is atomic and fails if the destination already exists.
    // Unlike rename, it never replaces a file created after the earlier existence check.
    std::fs::hard_link(temporary, destination)
        .map_err(|error| package_error(format!("cannot finalize export archive: {error}")))?;
    std::fs::remove_file(temporary)
        .map_err(|error| package_error(format!("cannot remove export temporary file: {error}")))?;
    Ok(())
}

fn export_profile_to_path(
    conn: &Connection,
    profile_id: &str,
    path: &Path,
    include_history: bool,
) -> Result<(), AppError> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("swprofile"))
    {
        return Err(AppError::Invalid(
            "export path must end in .swprofile".into(),
        ));
    }
    if path.exists() {
        return Err(AppError::Invalid(
            "export destination already exists".into(),
        ));
    }
    let package = profile_package_from_db(conn, profile_id, include_history)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(AppError::Invalid(
            "export destination folder does not exist".into(),
        ));
    }
    let temp_path = parent.join(format!(".swprofile-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                package_error(format!("cannot create export temporary file: {error}"))
            })?;
        let mut archive = ZipWriter::new(file);
        let manifest = serde_json::to_vec_pretty(&package.manifest)?;
        let profile = serde_json::to_vec_pretty(&package.profile)?;
        let resources = serde_json::to_vec_pretty(&package.resources)?;
        let layouts = serde_json::to_vec_pretty(&package.layouts)?;
        for (name, bytes) in [
            ("manifest.json", &manifest),
            ("profile.json", &profile),
            ("resources.json", &resources),
            ("layouts.json", &layouts),
        ] {
            if bytes.len() as u64 > PROFILE_ARCHIVE_MAX_ENTRY {
                return Err(package_error(format!(
                    "export member exceeds size limit: {name}"
                )));
            }
        }
        let mut uncompressed_total = manifest.len() as u64
            + profile.len() as u64
            + resources.len() as u64
            + layouts.len() as u64;
        write_zip_file(&mut archive, "manifest.json", &manifest)?;
        write_zip_file(&mut archive, "profile.json", &profile)?;
        write_zip_file(&mut archive, "resources.json", &resources)?;
        write_zip_file(&mut archive, "layouts.json", &layouts)?;
        if let Some(history) = &package.history {
            let bytes = serde_json::to_vec_pretty(history)?;
            if bytes.len() as u64 > PROFILE_ARCHIVE_MAX_ENTRY {
                return Err(package_error("export history exceeds member size limit"));
            }
            uncompressed_total = uncompressed_total.saturating_add(bytes.len() as u64);
            write_zip_file(&mut archive, "history.json", &bytes)?;
        }
        if uncompressed_total > PROFILE_ARCHIVE_MAX_TOTAL {
            return Err(package_error(
                "export exceeds total uncompressed size limit",
            ));
        }
        let file = archive
            .finish()
            .map_err(|error| package_error(format!("cannot finish export archive: {error}")))?;
        file.sync_all()
            .map_err(|error| package_error(format!("cannot sync export archive: {error}")))?;
        if file.metadata()?.len() > PROFILE_ARCHIVE_MAX_COMPRESSED {
            return Err(package_error("export exceeds compressed size limit"));
        }
        finalize_export_no_clobber(&temp_path, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn profile_import_preview_from_conn(
    conn: &Connection,
    package: &ValidatedProfilePackage,
    package_sha256: &str,
    target_profile_id: Option<&str>,
) -> Result<ProfileImportPreview, AppError> {
    if let Some(target_profile_id) = target_profile_id {
        valid_uuid(target_profile_id, "target profile id")?;
        let target_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
            [target_profile_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !target_exists {
            return Err(AppError::NotFound);
        }
    }
    let profile_id_collision = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&package.profile.id],
        |row| row.get::<_, i64>(0),
    )? != 0;
    let profile_name_collision = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE name=?1 COLLATE NOCASE)",
        [&package.profile.name],
        |row| row.get::<_, i64>(0),
    )? != 0;
    let mut resource_id_collisions = Vec::new();
    let mut target_id_collisions = Vec::new();
    let mut resource_conflicts = Vec::new();
    for resource in &package.resources.resources {
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1)",
            [&resource.id],
            |row| row.get::<_, i64>(0),
        )? != 0
        {
            resource_id_collisions.push(resource.id.clone());
            let imported = imported_resource_snapshot(resource);
            if let Some(existing) = existing_resource_snapshot(conn, &resource.id)? {
                let matches_existing = resources_equivalent(&imported, &existing);
                let proposed = if matches_existing {
                    existing.clone()
                } else {
                    proposed_import_snapshot(imported.clone())
                };
                resource_conflicts.push(ProfileResourceConflictPreview {
                    resolution: if matches_existing {
                        "reuse_existing".into()
                    } else {
                        "import_as_new".into()
                    },
                    imported,
                    existing,
                    proposed,
                });
            }
        }
        for target in &resource.targets {
            if conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1)",
                [&target.id],
                |row| row.get::<_, i64>(0),
            )? != 0
            {
                target_id_collisions.push(target.id.clone());
            }
        }
    }
    let mut smart_collection_id_collisions = Vec::new();
    let mut smart_collection_name_collisions = Vec::new();
    for collection in &package.profile.smart_collections {
        let id_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM smart_collections WHERE id=?1)",
            [&collection.id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if id_exists {
            smart_collection_id_collisions.push(collection.id.clone());
        }
        let name_exists: bool = if let Some(target_profile_id) = target_profile_id {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM smart_collections WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
                params![target_profile_id, collection.name], |row| row.get::<_, i64>(0),
            )? != 0
        } else {
            false
        };
        if name_exists {
            smart_collection_name_collisions.push(collection.name.clone());
        }
    }
    let mut dynamic_shortcut_id_collisions = Vec::new();
    let mut dynamic_shortcut_name_collisions = Vec::new();
    for shortcut in &package.profile.dynamic_shortcuts {
        let id_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE id=?1)",
            [&shortcut.id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if id_exists {
            dynamic_shortcut_id_collisions.push(shortcut.id.clone());
        }
        let name_exists: bool = if let Some(target_profile_id) = target_profile_id {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
                params![target_profile_id, shortcut.name], |row| row.get::<_, i64>(0),
            )? != 0
        } else {
            false
        };
        if name_exists {
            dynamic_shortcut_name_collisions.push(shortcut.name.clone());
        }
    }
    let mut workspace_sequence_id_collisions = Vec::new();
    let mut workspace_sequence_name_collisions = Vec::new();
    for sequence in &package.profile.workspace_sequences {
        let id_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_sequences WHERE id=?1)",
            [&sequence.id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if id_exists {
            workspace_sequence_id_collisions.push(sequence.id.clone());
        }
        let name_exists: bool = if let Some(target_profile_id) = target_profile_id {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM workspace_sequences WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
                params![target_profile_id, sequence.name],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )?
        } else {
            false
        };
        if name_exists {
            workspace_sequence_name_collisions.push(sequence.name.clone());
        }
    }
    let mut preview = ProfileImportPreview {
        package_sha256: package_sha256.into(),
        preview_token: String::new(),
        profile_id: package.profile.id.clone(),
        profile_name: package.profile.name.clone(),
        target_profile_id: target_profile_id.map(str::to_string),
        resource_count: package.resources.resources.len(),
        layout_count: package.layouts.layouts.len(),
        history_event_count: package
            .history
            .as_ref()
            .map_or(0, |history| history.events.len()),
        smart_collection_count: package.profile.smart_collections.len(),
        dynamic_shortcut_count: package.profile.dynamic_shortcuts.len(),
        workspace_sequence_count: package.profile.workspace_sequences.len(),
        profile_id_collision,
        profile_name_collision,
        resource_id_collisions,
        target_id_collisions,
        resource_conflicts,
        smart_collection_id_collisions,
        smart_collection_name_collisions,
        dynamic_shortcut_id_collisions,
        dynamic_shortcut_name_collisions,
        workspace_sequence_id_collisions,
        workspace_sequence_name_collisions,
        asset_count: package.manifest.assets.len(),
    };
    preview.preview_token = profile_preview_token(&preview)?;
    Ok(preview)
}

fn imported_resource_snapshot(resource: &ProfilePackageResource) -> ProfileResourceSnapshot {
    ProfileResourceSnapshot {
        id: resource.id.clone(),
        name: resource.name.clone(),
        resource_type: resource.resource_type.clone(),
        description: resource.description.clone(),
        icon: resource.icon.clone(),
        tags: resource.tags.clone(),
        aliases: resource.aliases.clone(),
        targets: resource
            .targets
            .iter()
            .map(|target| ProfileTargetSnapshot {
                id: target.id.clone(),
                kind: target.kind.clone(),
                value: target.value.clone(),
                priority: target.priority,
                arguments: target.argv.clone(),
                working_directory: target.working_directory.clone(),
                active: target.active,
            })
            .collect(),
    }
}

fn proposed_import_snapshot(mut imported: ProfileResourceSnapshot) -> ProfileResourceSnapshot {
    for target in &mut imported.targets {
        if ["app", "shortcut"].contains(&target.kind.as_str()) {
            target.active = false;
        }
    }
    imported
}

fn existing_resource_snapshot(
    conn: &Connection,
    resource_id: &str,
) -> Result<Option<ProfileResourceSnapshot>, AppError> {
    let mut resource: Option<Resource> = conn.query_row(
        "SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",
        [resource_id], resource_from_row,
    ).optional()?;
    let Some(mut resource) = resource.take() else {
        return Ok(None);
    };
    load_resource_details(conn, &mut resource, None)?;
    Ok(Some(ProfileResourceSnapshot {
        id: resource.id,
        name: resource.display_name,
        resource_type: resource.resource_type,
        description: resource.description,
        icon: resource.icon,
        tags: resource.tags,
        aliases: resource.aliases,
        targets: resource
            .targets
            .into_iter()
            .map(|target| ProfileTargetSnapshot {
                id: target.id,
                kind: target.kind,
                value: target.canonical_uri,
                priority: target.priority,
                arguments: target.arguments,
                working_directory: target.working_directory,
                active: target.active,
            })
            .collect(),
    }))
}

fn resources_equivalent(
    imported: &ProfileResourceSnapshot,
    existing: &ProfileResourceSnapshot,
) -> bool {
    let mut imported_tags = imported.tags.clone();
    let mut existing_tags = existing.tags.clone();
    let mut imported_aliases = imported.aliases.clone();
    let mut existing_aliases = existing.aliases.clone();
    imported_tags.sort();
    existing_tags.sort();
    imported_aliases.sort();
    existing_aliases.sort();
    let mut unmatched = existing.targets.iter().collect::<Vec<_>>();
    imported.name == existing.name
        && imported.resource_type == existing.resource_type
        && imported.description == existing.description
        && imported.icon == existing.icon
        && imported_tags == existing_tags
        && imported_aliases == existing_aliases
        && imported.targets.len() == existing.targets.len()
        && imported.targets.iter().all(|target| {
            let Some(index) = unmatched
                .iter()
                .position(|other| targets_equivalent(target, other))
            else {
                return false;
            };
            unmatched.remove(index);
            true
        })
}

fn targets_equivalent(imported: &ProfileTargetSnapshot, existing: &ProfileTargetSnapshot) -> bool {
    let expected_active = imported.active && !["app", "shortcut"].contains(&imported.kind.as_str());
    imported.kind == existing.kind
        && imported.value == existing.value
        && imported.priority == existing.priority
        && imported.arguments == existing.arguments
        && imported.working_directory == existing.working_directory
        && expected_active == existing.active
}

fn profile_preview_token(preview: &ProfileImportPreview) -> Result<String, AppError> {
    let mut stable_preview = preview.clone();
    stable_preview.preview_token.clear();
    let serialized = serde_json::to_vec(&stable_preview)?;
    let mut hasher = Sha256::new();
    hasher.update(preview.package_sha256.as_bytes());
    hasher.update([0]);
    hasher.update(serialized);
    Ok(format!("{:x}", hasher.finalize()))
}

fn profile_import_name(conn: &Connection, preferred: &str) -> Result<String, AppError> {
    let base = format!(
        "{} (Imported)",
        preferred.trim().chars().take(65).collect::<String>()
    );
    for suffix in 0..1000 {
        let candidate = if suffix == 0 {
            base.chars().take(80).collect::<String>()
        } else {
            let ending = format!(" {suffix}");
            format!(
                "{}{}",
                base.chars()
                    .take(80 - ending.chars().count())
                    .collect::<String>(),
                ending
            )
        };
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE name=?1 COLLATE NOCASE)",
            [&candidate],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(package_error("could not allocate a unique profile name"))
}

fn safe_import_settings(value: serde_json::Value) -> serde_json::Value {
    let filtered = scrub_secret_settings(value);
    let mut object = filtered.as_object().cloned().unwrap_or_default();
    object.remove("quickLaunchShortcut");
    serde_json::Value::Object(object)
}

fn unique_smart_collection_name(
    tx: &Transaction<'_>,
    profile_id: &str,
    requested: &str,
) -> Result<String, AppError> {
    let requested = requested.trim();
    for suffix in 1..=1000 {
        let candidate = if suffix == 1 {
            requested.to_string()
        } else {
            let ending = format!(" (imported {suffix})");
            let base: String = requested
                .chars()
                .take(100usize.saturating_sub(ending.chars().count()))
                .collect();
            format!("{}{}", base.trim_end(), ending)
        };
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM smart_collections WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
            params![profile_id, candidate],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(package_error(
        "could not allocate a unique smart collection name",
    ))
}

fn fresh_smart_collection_id(tx: &Transaction<'_>) -> Result<String, AppError> {
    for _ in 0..100 {
        let id = Uuid::new_v4().to_string();
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM smart_collections WHERE id=?1)",
            [&id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !exists {
            return Ok(id);
        }
    }
    Err(package_error(
        "could not allocate a unique smart collection ID",
    ))
}

fn import_profile_smart_collections(
    tx: &Transaction<'_>,
    package: &ValidatedProfilePackage,
    target_profile_id: &str,
    strategy: &str,
) -> Result<(), AppError> {
    for collection in &package.profile.smart_collections {
        let existing_by_name_or_id: Option<(String, String)> = tx
            .query_row(
                "SELECT name,filters_json FROM smart_collections WHERE profile_id=?1 AND (id=?2 OR name=?3 COLLATE NOCASE) LIMIT 1",
                params![target_profile_id, collection.id, collection.name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if strategy == "merge" {
            if let Some((existing_name, existing_filters_json)) = &existing_by_name_or_id {
                let existing_filters: SmartCollectionFilters =
                    serde_json::from_str(existing_filters_json)?;
                if existing_name.eq_ignore_ascii_case(&collection.name)
                    && existing_filters == collection.filters
                {
                    continue;
                }
            }
        }
        let id_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM smart_collections WHERE id=?1)",
            [&collection.id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        let id = if strategy == "duplicate" || id_exists {
            fresh_smart_collection_id(tx)?
        } else {
            collection.id.clone()
        };
        let name = if existing_by_name_or_id.is_some() {
            unique_smart_collection_name(tx, target_profile_id, &collection.name)?
        } else {
            collection.name.clone()
        };
        let existing_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM smart_collections WHERE profile_id=?1",
            [target_profile_id],
            |row| row.get(0),
        )?;
        if existing_count >= MAX_SMART_COLLECTIONS_PER_PROFILE {
            return Err(package_error(
                "profile would exceed the smart collection limit",
            ));
        }
        let filters_json = serde_json::to_string(&collection.filters)?;
        let timestamp = now_utc();
        tx.execute(
            "INSERT INTO smart_collections(id,profile_id,name,filters_json,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?5)",
            params![id, target_profile_id, name, filters_json, timestamp],
        )?;
    }
    Ok(())
}

fn unique_dynamic_shortcut_name(
    tx: &Transaction<'_>,
    profile_id: &str,
    requested: &str,
) -> Result<String, AppError> {
    let requested = requested.trim();
    for suffix in 1..=1000 {
        let candidate = if suffix == 1 {
            requested.to_string()
        } else {
            let ending = format!(" (imported {suffix})");
            let base: String = requested
                .chars()
                .take(100usize.saturating_sub(ending.chars().count()))
                .collect();
            format!("{}{}", base.trim_end(), ending)
        };
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
            params![profile_id, candidate],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(package_error(
        "could not allocate a unique dynamic shortcut name",
    ))
}

fn fresh_dynamic_shortcut_id(tx: &Transaction<'_>) -> Result<String, AppError> {
    for _ in 0..100 {
        let id = Uuid::new_v4().to_string();
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE id=?1)",
            [&id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !exists {
            return Ok(id);
        }
    }
    Err(package_error(
        "could not allocate a unique dynamic shortcut ID",
    ))
}

fn import_profile_dynamic_shortcuts(
    tx: &Transaction<'_>,
    package: &ValidatedProfilePackage,
    target_profile_id: &str,
    strategy: &str,
    resource_map: &std::collections::HashMap<String, String>,
    target_map: &std::collections::HashMap<String, String>,
) -> Result<(), AppError> {
    for shortcut in &package.profile.dynamic_shortcuts {
        let resource_id = resource_map
            .get(&shortcut.resource_id)
            .ok_or_else(|| package_error("dynamic shortcut resource mapping missing"))?;
        let target_id = if let Some(source_target_id) = shortcut.target_id.as_ref() {
            if let Some(mapped_id) = target_map.get(source_target_id) {
                let active_and_owned: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)",
                    params![mapped_id, resource_id],
                    |row| row.get::<_, i64>(0).map(|value| value != 0),
                )?;
                active_and_owned.then(|| mapped_id.clone())
            } else {
                None
            }
        } else {
            None
        };
        let existing: Option<(String, String, Option<String>, String)> = tx
            .query_row(
                "SELECT id,resource_id,target_id,name FROM dynamic_shortcuts WHERE profile_id=?1 AND (id=?2 OR name=?3 COLLATE NOCASE) LIMIT 1",
                params![target_profile_id, shortcut.id, shortcut.name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if strategy == "merge" {
            if let Some((_existing_id, existing_resource, existing_target, existing_name)) =
                &existing
            {
                if existing_name.eq_ignore_ascii_case(&shortcut.name)
                    && existing_resource == resource_id
                    && existing_target == &target_id
                {
                    continue;
                }
            }
        }
        let id_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE id=?1)",
            [&shortcut.id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        let id = if strategy == "duplicate" || id_exists {
            fresh_dynamic_shortcut_id(tx)?
        } else {
            shortcut.id.clone()
        };
        let name = if existing.is_some() {
            unique_dynamic_shortcut_name(tx, target_profile_id, &shortcut.name)?
        } else {
            shortcut.name.clone()
        };
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM dynamic_shortcuts WHERE profile_id=?1",
            [target_profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_DYNAMIC_SHORTCUTS_PER_PROFILE {
            return Err(package_error(
                "profile would exceed the dynamic shortcut limit",
            ));
        }
        let timestamp = now_utc();
        tx.execute(
            "INSERT INTO dynamic_shortcuts(id,profile_id,name,resource_id,target_id,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?6,?6)",
            params![id, target_profile_id, name, resource_id, target_id, timestamp],
        )?;
    }
    Ok(())
}

fn unique_workspace_sequence_name(
    tx: &Transaction<'_>,
    profile_id: &str,
    requested: &str,
) -> Result<String, AppError> {
    let requested = requested.trim();
    for suffix in 1..=1000 {
        let ending = if suffix == 1 {
            String::new()
        } else {
            format!(" (imported {suffix})")
        };
        let base: String = requested
            .chars()
            .take(100usize.saturating_sub(ending.chars().count()))
            .collect();
        let candidate = format!("{}{}", base.trim_end(), ending);
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM workspace_sequences WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)", params![profile_id,candidate], |row| row.get::<_,i64>(0).map(|value| value != 0))?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(package_error(
        "could not allocate a unique workspace sequence name",
    ))
}

fn fresh_workspace_sequence_id(tx: &Transaction<'_>) -> Result<String, AppError> {
    for _ in 0..100 {
        let id = Uuid::new_v4().to_string();
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1)", [&id], |row| row.get::<_,i64>(0).map(|value| value != 0))?;
        if !exists {
            return Ok(id);
        }
    }
    Err(package_error(
        "could not allocate a unique workspace sequence ID",
    ))
}

fn fresh_workspace_sequence_step_id(tx: &Transaction<'_>) -> Result<String, AppError> {
    for _ in 0..100 {
        let id = Uuid::new_v4().to_string();
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1)", [&id], |row| row.get::<_,i64>(0).map(|value| value != 0))?;
        if !exists {
            return Ok(id);
        }
    }
    Err(package_error(
        "could not allocate a unique workspace sequence step ID",
    ))
}

fn import_profile_workspace_sequences(
    tx: &Transaction<'_>,
    package: &ValidatedProfilePackage,
    target_profile_id: &str,
    strategy: &str,
    resource_map: &std::collections::HashMap<String, String>,
    target_map: &std::collections::HashMap<String, String>,
) -> Result<usize, AppError> {
    let mut imported = 0;
    for source in &package.profile.workspace_sequences {
        let mut steps = Vec::with_capacity(source.steps.len());
        for source_step in &source.steps {
            let resource_id = resource_map
                .get(&source_step.resource_id)
                .ok_or_else(|| package_error("workspace sequence resource mapping missing"))?
                .clone();
            let target_id = if let Some(source_target_id) = source_step.target_id.as_deref() {
                let mapped = target_map
                    .get(source_target_id)
                    .ok_or_else(|| package_error("workspace sequence target mapping missing"))?;
                let active_owned: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)", params![mapped,resource_id], |row| row.get::<_,i64>(0).map(|value| value != 0))?;
                if !active_owned {
                    return Err(package_error(
                        "workspace sequence target is no longer active and owned",
                    ));
                }
                Some(mapped.clone())
            } else {
                None
            };
            steps.push(WorkspaceSequenceStep {
                id: source_step.id.clone(),
                resource_id,
                target_id,
            });
        }
        let existing = workspace_sequence_list_on_conn(tx, target_profile_id)?
            .into_iter()
            .find(|sequence| {
                sequence.id == source.id || sequence.name.eq_ignore_ascii_case(source.name.trim())
            });
        if strategy == "merge" {
            if let Some(existing) = &existing {
                let equivalent = existing.name.eq_ignore_ascii_case(source.name.trim())
                    && existing.steps.len() == steps.len()
                    && existing.steps.iter().zip(&steps).all(|(left, right)| {
                        left.resource_id == right.resource_id && left.target_id == right.target_id
                    });
                if equivalent {
                    continue;
                }
            }
        }
        let id_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1)",
            [&source.id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        let id = if strategy == "duplicate" || id_exists {
            fresh_workspace_sequence_id(tx)?
        } else {
            source.id.clone()
        };
        let name = if existing.is_some() {
            unique_workspace_sequence_name(tx, target_profile_id, &source.name)?
        } else {
            source.name.trim().to_string()
        };
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM workspace_sequences WHERE profile_id=?1",
            [target_profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_WORKSPACE_SEQUENCES_PER_PROFILE {
            return Err(package_error(
                "profile would exceed the workspace sequence limit",
            ));
        }
        for step in &mut steps {
            let step_id_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1)",
                [&step.id],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )?;
            if strategy == "duplicate" || step_id_exists {
                step.id = fresh_workspace_sequence_step_id(tx)?;
            }
        }
        let sequence = WorkspaceSequence {
            id: id.clone(),
            profile_id: target_profile_id.to_string(),
            name,
            steps,
        };
        validate_workspace_sequence_fields(&sequence.id, &sequence.name, &sequence.steps)
            .map_err(|error| package_error(error.to_string()))?;
        let timestamp = now_utc();
        tx.execute("INSERT INTO workspace_sequences(id,profile_id,name,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?4)", params![id,target_profile_id,sequence.name,timestamp])?;
        for (position, step) in sequence.steps.iter().enumerate() {
            tx.execute("INSERT INTO workspace_sequence_steps(sequence_id,profile_id,id,position,resource_id,target_id) VALUES (?1,?2,?3,?4,?5,?6)", params![id,target_profile_id,step.id,position as i64,step.resource_id,step.target_id])?;
        }
        imported += 1;
    }
    Ok(imported)
}

fn import_profile_package(
    conn: &mut Connection,
    package: &ValidatedProfilePackage,
    strategy: &str,
    target_profile_id: Option<&str>,
) -> Result<ProfileImportResult, AppError> {
    if !["duplicate", "merge", "replace"].contains(&strategy) {
        return Err(AppError::Invalid(
            "import strategy must be duplicate, merge or replace".into(),
        ));
    }
    let tx = conn.transaction()?;
    let target_id = match strategy {
        "duplicate" => Uuid::new_v4().to_string(),
        _ => {
            let requested = target_profile_id.unwrap_or(&package.profile.id);
            valid_uuid(requested, "target profile id")?;
            let exists = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [requested],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if !exists {
                return Err(AppError::NotFound);
            }
            requested.to_owned()
        }
    };
    let existing_target_name: Option<String> = tx
        .query_row(
            "SELECT name FROM profiles WHERE id=?1",
            [&target_id],
            |row| row.get(0),
        )
        .optional()?;
    let profile_name = match strategy {
        "duplicate" => profile_import_name(&tx, &package.profile.name)?,
        "merge" => existing_target_name
            .clone()
            .unwrap_or_else(|| package.profile.name.clone()),
        "replace" => {
            let collides_elsewhere = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE name=?1 COLLATE NOCASE AND id<>?2)",
                params![package.profile.name, target_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if collides_elsewhere {
                existing_target_name
                    .clone()
                    .unwrap_or_else(|| package.profile.name.clone())
            } else {
                package.profile.name.clone()
            }
        }
        _ => unreachable!(),
    };
    let mut settings = safe_import_settings(package.profile.settings.clone());
    if let Some(object) = settings.as_object_mut() {
        object.insert(
            "accent".into(),
            serde_json::Value::String(package.profile.theme.accent.clone()),
        );
        if let Some(density) = &package.profile.theme.density {
            object.insert("density".into(), serde_json::Value::String(density.clone()));
        }
    }
    let settings_json = settings.to_string();
    let now = now_utc();
    let theme = package.profile.theme.mode.as_str();
    if strategy == "duplicate" {
        tx.execute("INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?5)", params![target_id, profile_name, theme, settings_json, now])?;
    } else if strategy == "replace" {
        tx.execute(
            "DELETE FROM profile_resources WHERE profile_id=?1",
            [&target_id],
        )?;
        tx.execute("DELETE FROM layouts WHERE profile_id=?1", [&target_id])?;
        tx.execute(
            "DELETE FROM smart_collections WHERE profile_id=?1",
            [&target_id],
        )?;
        tx.execute(
            "DELETE FROM dynamic_shortcuts WHERE profile_id=?1",
            [&target_id],
        )?;
        tx.execute(
            "DELETE FROM workspace_sequences WHERE profile_id=?1",
            [&target_id],
        )?;
        tx.execute(
            "UPDATE profiles SET name=?2,theme_id=?3,settings_json=?4,updated_utc=?5 WHERE id=?1",
            params![target_id, profile_name, theme, settings_json, now],
        )?;
    } else {
        let mut current: serde_json::Value = tx
            .query_row(
                "SELECT settings_json FROM profiles WHERE id=?1",
                [&target_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|raw| serde_json::from_str(&raw))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        if let (Some(current), Some(imported)) = (current.as_object_mut(), settings.as_object()) {
            for (key, value) in imported {
                current.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        tx.execute(
            "UPDATE profiles SET theme_id=?2,settings_json=?3,updated_utc=?4 WHERE id=?1",
            params![target_id, theme, current.to_string(), now],
        )?;
    }
    let duplicate_all = strategy != "merge";
    let mut resource_map = std::collections::HashMap::new();
    let mut target_map = std::collections::HashMap::new();
    let mut imported_resources = 0usize;
    let mut reused_resources = 0usize;
    for resource in &package.resources.resources {
        let existing_resource = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1)",
            [&resource.id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if strategy == "merge" && existing_resource {
            let existing = existing_resource_snapshot(&tx, &resource.id)?
                .ok_or_else(|| package_error("resource collision disappeared during merge"))?;
            let imported = imported_resource_snapshot(resource);
            if resources_equivalent(&imported, &existing) {
                resource_map.insert(resource.id.clone(), resource.id.clone());
                let mut unmatched_targets = existing.targets.iter().collect::<Vec<_>>();
                for source_target in &imported.targets {
                    let target_index = unmatched_targets
                        .iter()
                        .position(|candidate| targets_equivalent(source_target, candidate))
                        .ok_or_else(|| package_error("matching target disappeared during merge"))?;
                    let existing_target = unmatched_targets.remove(target_index);
                    target_map.insert(source_target.id.clone(), existing_target.id.clone());
                }
                reused_resources += 1;
                continue;
            }
        }
        let new_id = if duplicate_all || existing_resource {
            Uuid::new_v4().to_string()
        } else {
            resource.id.clone()
        };
        resource_map.insert(resource.id.clone(), new_id.clone());
        let mut targets = Vec::with_capacity(resource.targets.len());
        for target in &resource.targets {
            let target_exists = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1)",
                [&target.id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            let target_id = if duplicate_all || target_exists {
                Uuid::new_v4().to_string()
            } else {
                target.id.clone()
            };
            target_map.insert(target.id.clone(), target_id.clone());
            targets.push(ResourceTarget {
                id: target_id,
                kind: target.kind.clone(),
                canonical_uri: target.value.clone(),
                priority: target.priority,
                arguments: target.argv.clone(),
                working_directory: target.working_directory.clone(),
                active: target.active && !["app", "shortcut"].contains(&target.kind.as_str()),
            });
        }
        let imported = Resource {
            id: new_id,
            resource_type: resource.resource_type.clone(),
            display_name: resource.name.clone(),
            description: resource.description.clone(),
            icon: resource.icon.clone(),
            default_action: "open".into(),
            created_utc: now.clone(),
            updated_utc: now.clone(),
            targets,
            tags: resource.tags.clone(),
            aliases: resource.aliases.clone(),
            category: None,
            favorite: false,
            launch_count: 0,
            last_launched_utc: None,
        };
        persist_resource_in_transaction(&tx, &imported, true, None)?;
        imported_resources += 1;
    }
    let membership_by_id: std::collections::HashMap<&str, &ProfilePackageMembership> = package
        .profile
        .membership_details
        .iter()
        .map(|entry| (entry.resource_id.as_str(), entry))
        .collect();
    for source_id in &package.profile.memberships {
        let imported_id = resource_map
            .get(source_id)
            .ok_or_else(|| package_error("membership mapping missing"))?;
        let detail = membership_by_id.get(source_id.as_str());
        tx.execute("INSERT INTO profile_resources(profile_id,resource_id,favorite,category,sort_position) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(profile_id,resource_id) DO UPDATE SET favorite=CASE WHEN ?6 THEN excluded.favorite ELSE profile_resources.favorite END, category=COALESCE(excluded.category,profile_resources.category), sort_position=CASE WHEN ?6 THEN excluded.sort_position ELSE profile_resources.sort_position END",
            params![target_id, imported_id, detail.is_some_and(|entry| entry.favorite) as i64, detail.and_then(|entry| entry.category.as_deref()), detail.map_or(0, |entry| entry.sort_position), strategy != "merge"])?;
    }
    let mut imported_layouts = 0;
    for layout in &package.layouts.layouts {
        let surface = layout.surface.as_deref().unwrap_or("dashboard");
        let cards = layout
            .cards
            .iter()
            .filter_map(|card| {
                resource_map
                    .get(&card.resource_id)
                    .map(|resource_id| LayoutCard {
                        resource_id: resource_id.clone(),
                        x: card.x,
                        y: card.y,
                        w: card.w,
                        h: card.h,
                        card_type: card.card_type.clone(),
                    })
            })
            .collect::<Vec<_>>();
        let value = DashboardLayout {
            profile_id: target_id.clone(),
            surface: surface.into(),
            version: 1,
            cards,
        };
        let json = serde_json::to_string(&value)?;
        if strategy == "merge" {
            tx.execute("INSERT OR IGNORE INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,?2,?3,?4,?5)", params![target_id, surface, value.version, json, now])?;
        } else {
            tx.execute("INSERT INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(profile_id,surface) DO UPDATE SET version=excluded.version,layout_json=excluded.layout_json,updated_utc=excluded.updated_utc", params![target_id, surface, value.version, json, now])?;
        }
        imported_layouts += 1;
    }
    import_profile_smart_collections(&tx, package, &target_id, strategy)?;
    import_profile_dynamic_shortcuts(
        &tx,
        package,
        &target_id,
        strategy,
        &resource_map,
        &target_map,
    )?;
    let imported_workspace_sequences = import_profile_workspace_sequences(
        &tx,
        package,
        &target_id,
        strategy,
        &resource_map,
        &target_map,
    )?;
    let mut imported_history_events = 0;
    if let Some(history) = &package.history {
        for event in &history.events {
            let Some(resource_id) = resource_map.get(&event.resource_id) else {
                continue;
            };
            if !should_record_usage(&tx, Some(&target_id), resource_id)? {
                continue;
            }
            let target_id_mapped = event
                .target_id
                .as_ref()
                .and_then(|target_id| target_map.get(target_id));
            let imported_utc = normalize_usage_timestamp(Some(event.launched_at_utc.clone()))?
                .unwrap_or_else(now_utc);
            tx.execute("INSERT INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status,error_code,message,duration_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), resource_id, target_id_mapped, target_id, event.source, imported_utc, event.status, event.error_code, event.message, event.duration_ms])?;
            imported_history_events += 1;
        }
    }
    tx.commit()?;
    Ok(ProfileImportResult {
        profile_id: target_id,
        profile_name,
        imported_resources,
        reused_resources,
        imported_layouts,
        imported_history_events,
        imported_workspace_sequences,
    })
}

fn import_profile_package_from_path(
    conn: &mut Connection,
    package_path: &Path,
    strategy: &str,
    target_profile_id: Option<&str>,
    expected_package_sha256: &str,
    expected_preview_token: &str,
) -> Result<ProfileImportResult, AppError> {
    if strategy == "duplicate" && target_profile_id.is_some() {
        return Err(AppError::Invalid(
            "duplicate imports do not use a target profile".into(),
        ));
    }
    if strategy != "duplicate" && target_profile_id.is_none() {
        return Err(AppError::Invalid(
            "merge and replace imports require a target profile".into(),
        ));
    }
    let (package, package_sha256) = read_profile_package_with_digest(package_path)?;
    if package_sha256 != expected_package_sha256 {
        return Err(package_error(
            "profile package changed after preview; preview it again",
        ));
    }
    let preview =
        profile_import_preview_from_conn(conn, &package, &package_sha256, target_profile_id)?;
    if preview.preview_token != expected_preview_token {
        return Err(package_error(
            "workspace changed after preview; review the updated preview",
        ));
    }
    import_profile_package(conn, &package, strategy, target_profile_id)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub request_id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub launched_at_utc: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvent {
    pub request_id: String,
    pub resource_id: String,
    pub target_id: Option<String>,
    pub profile_id: Option<String>,
    pub source: String,
    pub status: String,
    pub launched_at_utc: String,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    pub resource_id: String,
    pub target_id: String,
    pub inspected_utc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_utc: Option<String>,
    pub availability: String,
    pub provenance: String,
    pub cache_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub total_launches: u64,
    pub by_source: BTreeMap<String, u64>,
    pub by_status: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_event_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_utc: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageSettings {
    pub profile_id: String,
    pub recording_enabled: bool,
    pub retention_days: Option<u16>,
    pub excluded_resource_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageFilter {
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub resource_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageQuery {
    pub profile_id: String,
    pub from_utc: Option<String>,
    pub to_utc: Option<String>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub resource_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageExportResult {
    pub path: String,
    pub event_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutCard {
    pub resource_id: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    #[serde(rename = "type")]
    pub card_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DashboardLayout {
    pub profile_id: String,
    pub surface: String,
    pub version: i64,
    pub cards: Vec<LayoutCard>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRelationship {
    pub id: String,
    pub profile_id: String,
    pub source_resource_id: String,
    pub target_resource_id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub profile_id: String,
    pub name: String,
    pub created_utc: String,
    pub resource_ids: Vec<String>,
    pub dashboard: DashboardLayout,
    pub dock: DashboardLayout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshotRestoreResult {
    pub snapshot_id: String,
    pub restored_layouts: usize,
    pub missing_resource_ids: Vec<String>,
}

pub struct AppState(pub Arc<Mutex<Connection>>);

pub struct HotkeyStatusState(pub Mutex<QuickLaunchHotkeyStatus>);
pub struct ActiveProfileState(pub Mutex<String>);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickLaunchHotkeyStatus {
    pub registered: bool,
    pub shortcut: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_shortcut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

const QUICK_LAUNCH_HOTKEY_SPEC: &str = "ALT+SPACE";
const QUICK_LAUNCH_SETTING_KEY: &str = "quickLaunchShortcut";

fn hotkey_error_code(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();
    if [
        "already registered",
        "already in use",
        "conflict",
        "occupied",
        "busy",
    ]
    .iter()
    .any(|needle| message.contains(needle))
    {
        "hotkey_conflict"
    } else {
        "hotkey_registration_failed"
    }
}

fn quick_launch_toggle_pressed(_shortcut: &Shortcut, event: &ShortcutEvent) -> bool {
    event.state() == ShortcutState::Pressed
}

fn normalize_shortcut(value: &str) -> Result<String, AppError> {
    if value.trim().is_empty() || value.len() > 80 {
        return Err(AppError::Invalid(
            "shortcut must be a nonempty value under 80 bytes".into(),
        ));
    }
    let parsed: Shortcut = value
        .parse()
        .map_err(|_| AppError::Invalid("invalid global shortcut syntax".into()))?;
    Ok(parsed.into_string())
}

fn switch_hotkey<F, G>(
    requested: &str,
    active: Option<&str>,
    mut register: F,
    mut unregister: G,
) -> QuickLaunchHotkeyStatus
where
    F: FnMut(&str) -> Result<(), String>,
    G: FnMut(&str) -> Result<(), String>,
{
    let requested = requested.to_owned();
    if active == Some(requested.as_str()) {
        return QuickLaunchHotkeyStatus {
            registered: true,
            shortcut: requested.clone(),
            active_shortcut: Some(requested),
            error_code: None,
        };
    }
    if let Err(error) = register(&requested) {
        return QuickLaunchHotkeyStatus {
            registered: false,
            shortcut: requested,
            active_shortcut: active.map(str::to_owned),
            error_code: Some(hotkey_error_code(&error).into()),
        };
    }
    if let Some(active) = active {
        if let Err(error) = unregister(active) {
            let rollback = unregister(&requested);
            let code = if rollback.is_err() {
                "hotkey_registration_failed"
            } else {
                hotkey_error_code(&error)
            };
            return QuickLaunchHotkeyStatus {
                registered: false,
                shortcut: requested,
                active_shortcut: Some(active.to_owned()),
                error_code: Some(code.into()),
            };
        }
    }
    QuickLaunchHotkeyStatus {
        registered: true,
        shortcut: requested.clone(),
        active_shortcut: Some(requested),
        error_code: None,
    }
}

fn toggle_quick_launch(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("quick-launch") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if window.show().is_ok() {
        let _ = window.set_focus();
        let _ = window.emit("quick-launch-toggle", ());
    }
}

fn profile_settings(
    conn: &Connection,
    profile_id: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let raw: String = conn
        .query_row(
            "SELECT settings_json FROM profiles WHERE id=?1",
            [profile_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::Invalid("profile settings must be a JSON object".into()))
}

fn profile_shortcut(conn: &Connection, profile_id: &str) -> Result<String, AppError> {
    let settings = profile_settings(conn, profile_id)?;
    match settings.get(QUICK_LAUNCH_SETTING_KEY) {
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| AppError::Invalid("quickLaunchShortcut must be a string".into())),
        None => Ok(QUICK_LAUNCH_HOTKEY_SPEC.into()),
    }
}

fn persist_profile_shortcut(
    conn: &Connection,
    profile_id: &str,
    shortcut: &str,
) -> Result<(), AppError> {
    let mut settings = profile_settings(conn, profile_id)?;
    settings.insert(
        QUICK_LAUNCH_SETTING_KEY.into(),
        serde_json::Value::String(shortcut.into()),
    );
    conn.execute(
        "UPDATE profiles SET settings_json=?2,updated_utc=?3 WHERE id=?1",
        params![
            profile_id,
            serde_json::Value::Object(settings).to_string(),
            now_utc()
        ],
    )?;
    Ok(())
}

fn set_profile_shortcut_with<F, G>(
    conn: &mut Connection,
    profile_id: &str,
    requested: &str,
    active: Option<&str>,
    mut register: F,
    mut unregister: G,
) -> Result<QuickLaunchHotkeyStatus, AppError>
where
    F: FnMut(&str) -> Result<(), String>,
    G: FnMut(&str) -> Result<(), String>,
{
    let mut settings = profile_settings(conn, profile_id)?;
    let tx = conn.transaction()?;
    settings.insert(
        QUICK_LAUNCH_SETTING_KEY.into(),
        serde_json::Value::String(requested.into()),
    );
    tx.execute(
        "UPDATE profiles SET settings_json=?2,updated_utc=?3 WHERE id=?1",
        params![
            profile_id,
            serde_json::Value::Object(settings).to_string(),
            now_utc()
        ],
    )?;
    let status = switch_hotkey(requested, active, &mut register, &mut unregister);
    if !status.registered {
        drop(tx);
        return Ok(status);
    }
    if let Err(error) = tx.commit() {
        if let Some(active) = active {
            let _ = switch_hotkey(active, Some(requested), &mut register, &mut unregister);
        } else {
            let _ = unregister(requested);
        }
        return Err(AppError::Database(error));
    }
    Ok(status)
}

fn record_active_profile(state: &ActiveProfileState, profile_id: &str) {
    *state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = profile_id.to_owned();
}

fn inactive_hotkey_status(requested: &str, active: Option<String>) -> QuickLaunchHotkeyStatus {
    QuickLaunchHotkeyStatus {
        registered: false,
        shortcut: requested.into(),
        active_shortcut: active,
        error_code: None,
    }
}

fn save_inactive_profile_shortcut(
    conn: &Connection,
    profile_id: &str,
    requested: &str,
    active: Option<String>,
) -> Result<QuickLaunchHotkeyStatus, AppError> {
    persist_profile_shortcut(conn, profile_id, requested)?;
    Ok(inactive_hotkey_status(requested, active))
}

fn apply_profile_shortcut(
    app: &tauri::AppHandle,
    conn: &Connection,
    profile_id: &str,
    status_state: &HotkeyStatusState,
) -> Result<QuickLaunchHotkeyStatus, AppError> {
    let raw = profile_shortcut(conn, profile_id)?;
    let mut stored = status_state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = stored.active_shortcut.clone();
    let status = match normalize_shortcut(&raw) {
        Ok(requested) => switch_hotkey(
            &requested,
            previous.as_deref(),
            |shortcut| {
                app.global_shortcut()
                    .register(shortcut)
                    .map_err(|error| error.to_string())
            },
            |shortcut| {
                app.global_shortcut()
                    .unregister(shortcut)
                    .map_err(|error| error.to_string())
            },
        ),
        Err(_) => QuickLaunchHotkeyStatus {
            registered: false,
            shortcut: raw,
            active_shortcut: previous,
            error_code: Some("invalid_shortcut".into()),
        },
    };
    *stored = status.clone();
    Ok(status)
}

const MIGRATION: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS resources (
 id TEXT PRIMARY KEY, resource_type TEXT NOT NULL, display_name TEXT NOT NULL,
 description TEXT, icon TEXT, default_action TEXT NOT NULL,
 created_utc TEXT NOT NULL, updated_utc TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_utc TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS resource_targets (
 id TEXT PRIMARY KEY, resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
 kind TEXT NOT NULL, canonical_uri TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0,
 arguments_json TEXT NOT NULL DEFAULT '[]', working_directory TEXT, active INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_resources_name ON resources(display_name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_targets_resource ON resource_targets(resource_id, active, priority);
CREATE TABLE IF NOT EXISTS profiles (
 id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, theme_id TEXT NOT NULL DEFAULT 'system',
 settings_json TEXT NOT NULL DEFAULT '{}', created_utc TEXT NOT NULL DEFAULT '', updated_utc TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS profile_resources (
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
 resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
 favorite INTEGER NOT NULL DEFAULT 0, category TEXT, sort_position INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(profile_id, resource_id)
);
CREATE TABLE IF NOT EXISTS resource_terms (
 resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN ('tag','alias')), value TEXT NOT NULL COLLATE NOCASE,
 PRIMARY KEY(resource_id,kind,value)
);
CREATE TABLE IF NOT EXISTS launch_events (
 id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, resource_id TEXT NOT NULL,
 target_id TEXT, profile_id TEXT, source TEXT NOT NULL, launched_at_utc TEXT NOT NULL,
 status TEXT NOT NULL, error_code TEXT, message TEXT, duration_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_launch_resource_time ON launch_events(resource_id, launched_at_utc DESC);
CREATE TABLE IF NOT EXISTS layouts (
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
 surface TEXT NOT NULL CHECK(surface IN ('dashboard','dock')),
 version INTEGER NOT NULL,
 layout_json TEXT NOT NULL,
 updated_utc TEXT NOT NULL,
 PRIMARY KEY(profile_id,surface)
);
CREATE TABLE IF NOT EXISTS smart_collections (
 id TEXT PRIMARY KEY,
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100),
 filters_json TEXT NOT NULL CHECK(length(filters_json) <= 4096),
 created_utc TEXT NOT NULL,
 updated_utc TEXT NOT NULL,
 UNIQUE(profile_id,name COLLATE NOCASE)
);
CREATE INDEX IF NOT EXISTS idx_smart_collections_profile_name
 ON smart_collections(profile_id,name COLLATE NOCASE);
CREATE UNIQUE INDEX IF NOT EXISTS idx_targets_resource_id
 ON resource_targets(resource_id,id);
CREATE TABLE IF NOT EXISTS dynamic_shortcuts (
 id TEXT PRIMARY KEY,
 profile_id TEXT NOT NULL,
 name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100),
 resource_id TEXT NOT NULL,
 target_id TEXT,
 created_utc TEXT NOT NULL,
 updated_utc TEXT NOT NULL,
 UNIQUE(profile_id,name COLLATE NOCASE),
 FOREIGN KEY(profile_id,resource_id)
   REFERENCES profile_resources(profile_id,resource_id) ON DELETE CASCADE,
 FOREIGN KEY(resource_id,target_id)
   REFERENCES resource_targets(resource_id,id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_dynamic_shortcuts_profile_name
 ON dynamic_shortcuts(profile_id,name COLLATE NOCASE);
CREATE TRIGGER IF NOT EXISTS prevent_deactivating_dynamic_shortcut_target
BEFORE UPDATE OF active ON resource_targets
WHEN OLD.active=1 AND NEW.active=0
 AND EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE resource_id=OLD.resource_id AND target_id=OLD.id)
BEGIN
 SELECT RAISE(ABORT,'target is referenced by a dynamic shortcut');
END;
INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (1,'sw001-core-schema-v1',CURRENT_TIMESTAMP);
"#;

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn open_database(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Invalid(e.to_string()))?;
    }
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA trusted_schema=OFF;")?;
    migrate_database(&conn)?;
    Ok(conn)
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), AppError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.as_deref().ok() == Some(column));
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

fn migrate_database(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(MIGRATION)?;
    // Additive, restartable migration for databases created by the first core build.
    ensure_column(conn, "profiles", "created_utc", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "profiles", "updated_utc", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "profile_resources", "category", "TEXT")?;
    ensure_column(conn, "launch_events", "message", "TEXT")?;
    ensure_column(conn, "launch_events", "duration_ms", "INTEGER")?;
    conn.execute("UPDATE profiles SET created_utc=CASE WHEN created_utc='' THEN CURRENT_TIMESTAMP ELSE created_utc END, updated_utc=CASE WHEN updated_utc='' THEN CURRENT_TIMESTAMP ELSE updated_utc END WHERE created_utc='' OR updated_utc=''",[])?;
    let has_default: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE name='Default' COLLATE NOCASE)",
        [],
        |row| row.get(0),
    )?;
    if has_default == 0 {
        let ts = now_utc();
        conn.execute("INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,'Default','system','{}',?2,?2)",params![Uuid::new_v4().to_string(),ts])?;
    }
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (2,'sw003-profile-launch-metadata-v2',?1)",[now_utc()])?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (3,'sw004-profile-surface-layouts-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS metadata_cache (resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE, target_id TEXT NOT NULL REFERENCES resource_targets(id) ON DELETE CASCADE, inspected_utc TEXT NOT NULL, file_size_bytes INTEGER, created_utc TEXT, modified_utc TEXT, availability TEXT NOT NULL, provenance TEXT NOT NULL, cache_status TEXT NOT NULL DEFAULT 'fresh', error_code TEXT, PRIMARY KEY(resource_id,target_id)); CREATE INDEX IF NOT EXISTS idx_metadata_cache_inspected ON metadata_cache(inspected_utc);")?;
    ensure_column(
        conn,
        "metadata_cache",
        "cache_status",
        "TEXT NOT NULL DEFAULT 'fresh'",
    )?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (4,'sw007-metadata-cache-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS smart_collections (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100), filters_json TEXT NOT NULL CHECK(length(filters_json) <= 4096), created_utc TEXT NOT NULL, updated_utc TEXT NOT NULL, UNIQUE(profile_id,name COLLATE NOCASE)); CREATE INDEX IF NOT EXISTS idx_smart_collections_profile_name ON smart_collections(profile_id,name COLLATE NOCASE);")?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (5,'sw013-profile-smart-collections-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_targets_resource_id ON resource_targets(resource_id,id); CREATE TABLE IF NOT EXISTS dynamic_shortcuts (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100), resource_id TEXT NOT NULL, target_id TEXT, created_utc TEXT NOT NULL, updated_utc TEXT NOT NULL, UNIQUE(profile_id,name COLLATE NOCASE), FOREIGN KEY(profile_id,resource_id) REFERENCES profile_resources(profile_id,resource_id) ON DELETE CASCADE, FOREIGN KEY(resource_id,target_id) REFERENCES resource_targets(resource_id,id) ON DELETE CASCADE); CREATE INDEX IF NOT EXISTS idx_dynamic_shortcuts_profile_name ON dynamic_shortcuts(profile_id,name COLLATE NOCASE); CREATE TRIGGER IF NOT EXISTS prevent_deactivating_dynamic_shortcut_target BEFORE UPDATE OF active ON resource_targets WHEN OLD.active=1 AND NEW.active=0 AND EXISTS(SELECT 1 FROM dynamic_shortcuts WHERE resource_id=OLD.resource_id AND target_id=OLD.id) BEGIN SELECT RAISE(ABORT,'target is referenced by a dynamic shortcut'); END;")?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (6,'sw013-dynamic-shortcuts-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS workspace_sequences (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100), created_utc TEXT NOT NULL, updated_utc TEXT NOT NULL, UNIQUE(profile_id,id), UNIQUE(profile_id,name COLLATE NOCASE)); CREATE TABLE IF NOT EXISTS workspace_sequence_steps (sequence_id TEXT NOT NULL, profile_id TEXT NOT NULL, id TEXT NOT NULL UNIQUE, position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 19), resource_id TEXT NOT NULL, target_id TEXT, PRIMARY KEY(sequence_id,position), UNIQUE(sequence_id,id), FOREIGN KEY(profile_id,sequence_id) REFERENCES workspace_sequences(profile_id,id) ON DELETE CASCADE); CREATE INDEX IF NOT EXISTS idx_workspace_sequences_profile_name ON workspace_sequences(profile_id,name COLLATE NOCASE);")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS workspace_sequence_runs (request_id TEXT PRIMARY KEY, sequence_id TEXT NOT NULL, profile_id TEXT NOT NULL, sequence_json TEXT NOT NULL, result_json TEXT NOT NULL, created_utc TEXT NOT NULL, updated_utc TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_workspace_sequence_runs_profile_time ON workspace_sequence_runs(profile_id,created_utc DESC);")?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (7,'sw014-workspace-sequences-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS usage_settings (profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE, recording_enabled INTEGER NOT NULL DEFAULT 1 CHECK(recording_enabled IN (0,1)), retention_days INTEGER CHECK(retention_days IN (30,90,365)), excluded_resource_ids_json TEXT NOT NULL DEFAULT '[]'); CREATE TABLE IF NOT EXISTS launch_request_guard (request_id TEXT PRIMARY KEY, intent_hash TEXT NOT NULL, resource_id TEXT NOT NULL, target_id TEXT, profile_id TEXT, source TEXT NOT NULL, status TEXT NOT NULL, launched_at_utc TEXT NOT NULL, error_code TEXT, duration_ms INTEGER); CREATE INDEX IF NOT EXISTS idx_launch_guard_profile_time ON launch_request_guard(profile_id,launched_at_utc DESC);")?;
    let guard_has_message: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('launch_request_guard') WHERE name='message')", [], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
    if guard_has_message {
        conn.execute_batch("DROP INDEX IF EXISTS idx_launch_guard_profile_time; ALTER TABLE launch_request_guard RENAME TO launch_request_guard_with_message; CREATE TABLE launch_request_guard (request_id TEXT PRIMARY KEY, intent_hash TEXT NOT NULL, resource_id TEXT NOT NULL, target_id TEXT, profile_id TEXT, source TEXT NOT NULL, status TEXT NOT NULL, launched_at_utc TEXT NOT NULL, error_code TEXT, duration_ms INTEGER); INSERT INTO launch_request_guard(request_id,intent_hash,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,duration_ms) SELECT request_id,intent_hash,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,duration_ms FROM launch_request_guard_with_message; DROP TABLE launch_request_guard_with_message; CREATE INDEX idx_launch_guard_profile_time ON launch_request_guard(profile_id,launched_at_utc DESC);")?;
    }
    conn.execute("INSERT OR IGNORE INTO launch_request_guard(request_id,intent_hash,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,duration_ms) SELECT request_id,'legacy:'||request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,duration_ms FROM launch_events",[])?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (8,'sw015-usage-settings-request-guard-v1',?1)",[now_utc()])?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (9,'sw015-request-guard-path-free-v1',?1)",[now_utc()])?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS resource_relationships (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, source_resource_id TEXT NOT NULL, target_resource_id TEXT NOT NULL, label TEXT NOT NULL CHECK(length(label) BETWEEN 1 AND 80), CHECK(source_resource_id < target_resource_id), UNIQUE(profile_id,source_resource_id,target_resource_id), FOREIGN KEY(profile_id,source_resource_id) REFERENCES profile_resources(profile_id,resource_id) ON DELETE CASCADE, FOREIGN KEY(profile_id,target_resource_id) REFERENCES profile_resources(profile_id,resource_id) ON DELETE CASCADE); CREATE INDEX IF NOT EXISTS idx_resource_relationships_profile ON resource_relationships(profile_id,source_resource_id,target_resource_id); CREATE TABLE IF NOT EXISTS workspace_snapshots (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 100), created_utc TEXT NOT NULL, snapshot_json TEXT NOT NULL CHECK(length(snapshot_json) <= 1048576), UNIQUE(profile_id,name COLLATE NOCASE)); CREATE INDEX IF NOT EXISTS idx_workspace_snapshots_profile_time ON workspace_snapshots(profile_id,created_utc DESC,id); CREATE TRIGGER IF NOT EXISTS enforce_workspace_snapshot_limit BEFORE INSERT ON workspace_snapshots WHEN (SELECT COUNT(*) FROM workspace_snapshots WHERE profile_id=NEW.profile_id) >= 100 BEGIN SELECT RAISE(ABORT,'workspace snapshot limit reached'); END;")?;
    conn.execute("INSERT OR IGNORE INTO schema_migrations(version,checksum,applied_utc) VALUES (10,'sw016-resource-graph-workspace-snapshots-v1',?1)",[now_utc()])?;
    Ok(())
}

fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
    state
        .0
        .lock()
        .map_err(|_| AppError::Invalid("database lock poisoned".into()))
}

fn resource_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Resource> {
    Ok(Resource {
        id: row.get(0)?,
        resource_type: row.get(1)?,
        display_name: row.get(2)?,
        description: row.get(3)?,
        icon: row.get(4)?,
        default_action: row.get(5)?,
        created_utc: row.get(6)?,
        updated_utc: row.get(7)?,
        targets: Vec::new(),
        tags: Vec::new(),
        aliases: Vec::new(),
        category: None,
        favorite: false,
        launch_count: 0,
        last_launched_utc: None,
    })
}

fn load_targets(conn: &Connection, resource: &mut Resource) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT id,kind,canonical_uri,priority,arguments_json,working_directory,active FROM resource_targets WHERE resource_id=?1 ORDER BY priority DESC")?;
    let rows = stmt.query_map([&resource.id], |r| {
        Ok(ResourceTarget {
            id: r.get(0)?,
            kind: r.get(1)?,
            canonical_uri: r.get(2)?,
            priority: r.get(3)?,
            arguments: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
            working_directory: r.get(5)?,
            active: r.get::<_, i64>(6)? != 0,
        })
    })?;
    resource.targets = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn load_resource_details(
    conn: &Connection,
    resource: &mut Resource,
    profile_id: Option<&str>,
) -> Result<(), AppError> {
    load_resource_details_with_limits(conn, resource, profile_id, false)
}

fn load_resource_details_bounded(
    conn: &Connection,
    resource: &mut Resource,
    profile_id: Option<&str>,
) -> Result<(), AppError> {
    load_resource_details_with_limits(conn, resource, profile_id, true)
}

fn load_resource_details_with_limits(
    conn: &Connection,
    resource: &mut Resource,
    profile_id: Option<&str>,
    bounded: bool,
) -> Result<(), AppError> {
    load_targets(conn, resource)?;
    let query = if bounded {
        "SELECT kind,value FROM (SELECT kind,value FROM (SELECT kind,value FROM resource_terms WHERE resource_id=?1 AND kind='tag' ORDER BY value COLLATE NOCASE LIMIT 32) UNION ALL SELECT kind,value FROM (SELECT kind,value FROM resource_terms WHERE resource_id=?1 AND kind='alias' ORDER BY value COLLATE NOCASE LIMIT 32))"
    } else {
        "SELECT kind,value FROM resource_terms WHERE resource_id=?1 ORDER BY kind,value COLLATE NOCASE"
    };
    let mut stmt = conn.prepare(query)?;
    for term in stmt.query_map([&resource.id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (kind, value) = term?;
        if kind == "tag" {
            resource.tags.push(value)
        } else {
            resource.aliases.push(value)
        }
    }
    let (favorite, category) = if let Some(pid) = profile_id {
        conn.query_row("SELECT favorite,category FROM profile_resources WHERE resource_id=?1 AND profile_id=?2",params![resource.id,pid],|r|Ok((r.get::<_,i64>(0)?!=0,r.get::<_,Option<String>>(1)?))).optional()?.unwrap_or((false,None))
    } else {
        let any_favorite = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE resource_id=?1 AND favorite=1)",
            [&resource.id],
            |r| r.get::<_, i64>(0),
        )? != 0;
        (any_favorite, None)
    };
    resource.favorite = favorite;
    resource.category = category;
    let stats = if let Some(pid) = profile_id {
        conn.query_row("SELECT COUNT(*),MAX(launched_at_utc) FROM launch_events WHERE resource_id=?1 AND profile_id=?2",params![resource.id,pid],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?)))?
    } else {
        conn.query_row(
            "SELECT COUNT(*),MAX(launched_at_utc) FROM launch_events WHERE resource_id=?1",
            [&resource.id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )?
    };
    resource.launch_count = stats.0;
    resource.last_launched_utc = stats.1;
    Ok(())
}

#[tauri::command]
fn resource_list(
    state: State<'_, AppState>,
    profile_id: Option<String>,
) -> Result<Vec<Resource>, AppError> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT r.id,r.resource_type,r.display_name,r.description,r.icon,r.default_action,r.created_utc,r.updated_utc FROM resources r WHERE (?1 IS NULL OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND pr.profile_id=?1)) ORDER BY r.display_name COLLATE NOCASE")?;
    let mut rows = stmt.query_map([profile_id.as_deref()], resource_from_row)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next() {
        let mut resource = row?;
        load_resource_details(&conn, &mut resource, profile_id.as_deref())?;
        result.push(resource);
    }
    Ok(result)
}

fn validate_resource(resource: &Resource) -> Result<(), AppError> {
    Uuid::parse_str(&resource.id)
        .map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    if resource.display_name.trim().is_empty() || resource.display_name.chars().count() > 256 {
        return Err(AppError::Invalid(
            "display name must contain 1 to 256 characters".into(),
        ));
    }
    if resource
        .description
        .as_ref()
        .is_some_and(|value| value.chars().count() > 4096)
        || resource
            .icon
            .as_ref()
            .is_some_and(|value| value.chars().count() > 256)
        || resource.targets.is_empty()
        || resource.targets.len() > 8
        || resource.tags.len() > 64
        || resource.aliases.len() > 32
    {
        return Err(AppError::Invalid("resource exceeds field limits".into()));
    }
    for (name, values) in [("tag", &resource.tags), ("alias", &resource.aliases)] {
        let mut unique = std::collections::HashSet::new();
        if values.iter().any(|value| {
            value.trim().is_empty()
                || value.chars().count() > 80
                || !unique.insert(value.to_lowercase())
        }) {
            return Err(AppError::Invalid(format!(
                "invalid or duplicate resource {name}"
            )));
        }
    }
    if !["file", "folder", "app", "shortcut", "url"].contains(&resource.resource_type.as_str()) {
        return Err(AppError::Invalid("unsupported resource type".into()));
    }
    for target in &resource.targets {
        Uuid::parse_str(&target.id)
            .map_err(|_| AppError::Invalid("target id must be a UUID".into()))?;
        validate_target(target)?;
    }
    Ok(())
}

fn validate_target(target: &ResourceTarget) -> Result<(), AppError> {
    if target.canonical_uri.trim().is_empty()
        || target.canonical_uri.chars().count() > 4096
        || target.canonical_uri.chars().any(char::is_control)
        || !(0..=1000).contains(&target.priority)
        || target.arguments.len() > 64
        || target
            .arguments
            .iter()
            .any(|arg| arg.chars().count() > 4096 || arg.contains('\0'))
        || target.working_directory.as_ref().is_some_and(|value| {
            value.chars().count() > 4096
                || value.chars().any(char::is_control)
                || !valid_windows_absolute_path(value)
        })
    {
        return Err(AppError::Invalid(
            "target exceeds field limits or contains invalid characters".into(),
        ));
    }
    match target.kind.as_str() {
        "url" => {
            let parsed = Url::parse(&target.canonical_uri)
                .map_err(|_| AppError::Invalid("URL is invalid".into()))?;
            if parsed.scheme() != "https" || parsed.host_str().is_none() {
                return Err(AppError::Invalid("only HTTPS URLs may be opened".into()));
            }
            if !parsed.username().is_empty() || parsed.password().is_some() {
                return Err(AppError::Invalid(
                    "URL targets cannot include credentials".into(),
                ));
            }
            if !target.arguments.is_empty() {
                return Err(AppError::Invalid(
                    "arguments are only allowed for app targets".into(),
                ));
            }
        }
        "file" | "folder" | "app" | "shortcut" => {
            let path = target.canonical_uri.trim();
            if !valid_windows_absolute_path(path) {
                return Err(AppError::Invalid(
                    "target must be an absolute Windows path or UNC path".into(),
                ));
            }
            if target.kind != "app" && !target.arguments.is_empty() {
                return Err(AppError::Invalid(
                    "arguments are only allowed for app targets".into(),
                ));
            }
        }
        _ => return Err(AppError::Invalid("unsupported target kind".into())),
    }
    Ok(())
}

fn valid_windows_absolute_path(path: &str) -> bool {
    if path.is_empty()
        || path.chars().any(char::is_control)
        || path.starts_with("\\\\?\\")
        || path.starts_with("\\\\.\\")
    {
        return false;
    }
    let normalized = path.replace('/', "\\");
    let parts: Vec<&str>;
    let rest: &str;
    if normalized.as_bytes().get(1) == Some(&b':')
        && normalized
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && normalized.as_bytes().get(2) == Some(&b'\\')
    {
        rest = &normalized[3..];
        parts = rest.split('\\').filter(|part| !part.is_empty()).collect();
    } else if normalized.starts_with("\\\\") {
        let unc_parts: Vec<&str> = normalized[2..].split('\\').collect();
        if unc_parts.len() < 2 || unc_parts[0].is_empty() || unc_parts[1].is_empty() {
            return false;
        }
        parts = unc_parts;
    } else {
        return false;
    }
    parts.iter().all(|part| {
        *part != "."
            && *part != ".."
            && !part.ends_with([' ', '.'])
            && !part
                .chars()
                .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
            && !is_reserved_windows_device_name(part)
    })
}

fn is_reserved_windows_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix)
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
    })
}

fn persist_resource(
    conn: &mut Connection,
    resource: &Resource,
    creating: bool,
    profile_id: Option<&str>,
) -> Result<(), AppError> {
    validate_resource(resource)?;
    let tx = conn.transaction()?;
    persist_resource_in_transaction(&tx, resource, creating, profile_id)?;
    tx.commit()?;
    Ok(())
}

fn persist_resource_in_transaction(
    tx: &Transaction<'_>,
    resource: &Resource,
    creating: bool,
    profile_id: Option<&str>,
) -> Result<(), AppError> {
    validate_resource(resource)?;
    if creating {
        tx.execute(
            "INSERT INTO resources VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                resource.id,
                resource.resource_type,
                resource.display_name,
                resource.description,
                resource.icon,
                resource.default_action,
                resource.created_utc,
                resource.updated_utc
            ],
        )?;
    } else {
        let changed = tx.execute("UPDATE resources SET resource_type=?2,display_name=?3,description=?4,icon=?5,default_action=?6,updated_utc=?7 WHERE id=?1", params![resource.id,resource.resource_type,resource.display_name,resource.description,resource.icon,resource.default_action,resource.updated_utc])?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        tx.execute(
            "DELETE FROM resource_targets WHERE resource_id=?1",
            [&resource.id],
        )?;
    }
    for target in &resource.targets {
        tx.execute(
            "INSERT INTO resource_targets VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                target.id,
                resource.id,
                target.kind,
                target.canonical_uri,
                target.priority,
                serde_json::to_string(&target.arguments)?,
                target.working_directory,
                if target.active { 1i64 } else { 0i64 }
            ],
        )?;
    }
    tx.execute(
        "DELETE FROM resource_terms WHERE resource_id=?1",
        [&resource.id],
    )?;
    for (kind, values) in [("tag", &resource.tags), ("alias", &resource.aliases)] {
        for value in values {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO resource_terms(resource_id,kind,value) VALUES (?1,?2,?3)",
                params![resource.id, kind, value],
            )?;
        }
    }
    if creating {
        if let Some(profile_id) = profile_id {
            Uuid::parse_str(profile_id)
                .map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
            tx.execute(
                "INSERT INTO profile_resources(profile_id,resource_id,category) VALUES (?1,?2,?3)",
                params![profile_id, resource.id, resource.category],
            )?;
        }
    }
    Ok(())
}

#[tauri::command]
fn resource_create(
    state: State<'_, AppState>,
    resource: ResourceInput,
) -> Result<Resource, AppError> {
    let ts = now_utc();
    let profile_id = resource.profile_id;
    let targets = resource
        .targets
        .into_iter()
        .map(|t| ResourceTarget {
            id: t.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            kind: t.kind,
            canonical_uri: t.canonical_uri,
            priority: t.priority,
            arguments: t.arguments,
            working_directory: t.working_directory,
            active: t.active,
        })
        .collect();
    let resource = Resource {
        id: Uuid::new_v4().to_string(),
        resource_type: resource.resource_type,
        display_name: resource.display_name,
        description: resource.description,
        icon: resource.icon,
        default_action: resource.default_action.unwrap_or_else(|| "open".into()),
        created_utc: ts.clone(),
        updated_utc: ts,
        targets,
        tags: resource.tags,
        aliases: resource.aliases,
        category: resource.category,
        favorite: false,
        launch_count: 0,
        last_launched_utc: None,
    };
    let mut conn = lock(&state)?;
    let profile_id = Some(profile_id.unwrap_or(default_profile_id(&conn)?));
    persist_resource(&mut conn, &resource, true, profile_id.as_deref())?;
    Ok(resource)
}

#[tauri::command]
fn resource_update(
    state: State<'_, AppState>,
    id: String,
    patch: ResourcePatch,
) -> Result<Resource, AppError> {
    let mut conn = lock(&state)?;
    resource_update_in_conn(&mut conn, &id, patch)
}

fn resource_update_in_conn(
    conn: &mut Connection,
    id: &str,
    patch: ResourcePatch,
) -> Result<Resource, AppError> {
    Uuid::parse_str(id).map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    let mut resource=conn.query_row("SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",[id],resource_from_row).optional()?.ok_or(AppError::NotFound)?;
    load_resource_details(conn, &mut resource, patch.profile_id.as_deref())?;
    if let Some(v) = patch.resource_type {
        resource.resource_type = v;
    }
    if let Some(v) = patch.display_name {
        resource.display_name = v;
    }
    if patch.description.is_some() {
        resource.description = patch.description;
    }
    if patch.icon.is_some() {
        resource.icon = patch.icon;
    }
    if let Some(v) = patch.default_action {
        resource.default_action = v;
    }
    if let Some(v) = patch.targets {
        resource.targets = v
            .into_iter()
            .map(|t| ResourceTarget {
                id: t.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
                kind: t.kind,
                canonical_uri: t.canonical_uri,
                priority: t.priority,
                arguments: t.arguments,
                working_directory: t.working_directory,
                active: t.active,
            })
            .collect();
    }
    if let Some(v) = patch.tags {
        resource.tags = v;
    }
    if let Some(v) = patch.aliases {
        resource.aliases = v;
    }
    let profile_id = patch.profile_id.clone();
    if profile_id.is_some() {
        resource.category = patch.category.clone().or(resource.category);
        resource.favorite = patch.favorite.unwrap_or(resource.favorite);
    }
    resource.updated_utc = now_utc();
    validate_resource(&resource)?;
    if let Some(pid) = profile_id.as_deref() {
        Uuid::parse_str(pid).map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    }
    let tx = conn.transaction()?;
    if let Some(pid) = profile_id.as_deref() {
        let profile_exists: i64 = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
            [pid],
            |row| row.get(0),
        )?;
        if profile_exists == 0 {
            return Err(AppError::NotFound);
        }
    }
    persist_resource_in_transaction(&tx, &resource, false, None)?;
    if let Some(pid) = profile_id {
        tx.execute("INSERT INTO profile_resources(profile_id,resource_id,favorite,category) VALUES (?1,?2,?3,?4) ON CONFLICT(profile_id,resource_id) DO UPDATE SET favorite=excluded.favorite,category=excluded.category",params![pid,resource.id,if resource.favorite {1i64}else{0i64},resource.category])?;
    }
    tx.commit()?;
    Ok(resource)
}

#[tauri::command]
fn resource_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    Uuid::parse_str(&id).map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    let changed = lock(&state)?.execute("DELETE FROM resources WHERE id=?1", [id])?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn resource_duplicate_on_conn(
    conn: &mut Connection,
    request: &ResourceDuplicateRequest,
) -> Result<Resource, AppError> {
    valid_uuid(&request.profile_id, "profile id")?;
    valid_uuid(&request.resource_id, "resource id")?;
    if let Some(name) = request.display_name.as_deref() {
        let trimmed = name.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > 256
            || trimmed.chars().any(char::is_control)
        {
            return Err(AppError::Invalid(
                "duplicate display name must contain 1 to 256 printable characters".into(),
            ));
        }
    }

    let tx = conn.transaction()?;
    let member: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
        params![request.profile_id, request.resource_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !member {
        return Err(AppError::NotFound);
    }
    let mut source = tx
        .query_row(
            "SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",
            [&request.resource_id],
            resource_from_row,
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    load_resource_details(&tx, &mut source, Some(&request.profile_id))?;

    let now = now_utc();
    let favorite = source.favorite;
    let mut duplicate = source;
    duplicate.id = Uuid::new_v4().to_string();
    duplicate.display_name = request
        .display_name
        .as_deref()
        .map(str::trim)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            duplicate
                .display_name
                .chars()
                .chain(" (copy)".chars())
                .take(256)
                .collect()
        });
    duplicate.created_utc = now.clone();
    duplicate.updated_utc = now;
    duplicate.launch_count = 0;
    duplicate.last_launched_utc = None;
    for target in &mut duplicate.targets {
        target.id = Uuid::new_v4().to_string();
    }
    validate_resource(&duplicate)?;
    persist_resource_in_transaction(&tx, &duplicate, true, Some(&request.profile_id))?;
    tx.execute(
        "UPDATE profile_resources SET favorite=?3 WHERE profile_id=?1 AND resource_id=?2",
        params![
            request.profile_id,
            duplicate.id,
            if favorite { 1i64 } else { 0i64 }
        ],
    )?;
    tx.commit()?;
    Ok(duplicate)
}

#[tauri::command]
fn resource_duplicate(
    state: State<'_, AppState>,
    request: ResourceDuplicateRequest,
) -> Result<Resource, AppError> {
    let mut conn = lock(&state)?;
    resource_duplicate_on_conn(&mut conn, &request)
}

fn normalize_relocation_prefix(path: &str, field: &str) -> Result<String, AppError> {
    let path = path.trim();
    if path.is_empty() || path.chars().count() > 4096 || !valid_windows_absolute_path(path) {
        return Err(AppError::Invalid(format!(
            "{field} must be an absolute Windows path"
        )));
    }
    let mut normalized = path.replace('/', "\\");
    let root_len = if normalized.as_bytes().get(1) == Some(&b':') {
        3
    } else {
        2
    };
    while normalized.len() > root_len && normalized.ends_with('\\') {
        normalized.pop();
    }
    Ok(normalized)
}

fn path_has_prefix(path: &str, prefix: &str) -> bool {
    let Some(head) = path.get(..prefix.len()) else {
        return false;
    };
    head.eq_ignore_ascii_case(prefix)
        && (path.len() == prefix.len()
            || prefix.ends_with('\\')
            || path.as_bytes().get(prefix.len()) == Some(&b'\\'))
}

fn replace_relocation_prefix(path: &str, source: &str, destination: &str) -> Option<String> {
    let path = path.replace('/', "\\");
    if !path_has_prefix(&path, source) {
        return None;
    }
    let suffix = path.get(source.len()..)?.trim_start_matches('\\');
    let replacement = if suffix.is_empty() {
        destination.to_owned()
    } else if destination.ends_with('\\') {
        format!("{destination}{suffix}")
    } else {
        format!("{destination}\\{suffix}")
    };
    Some(replacement)
}

fn canonicalize_relocation_request(
    mut request: ResourceRelocationPreviewRequest,
) -> Result<ResourceRelocationPreviewRequest, AppError> {
    valid_uuid(&request.profile_id, "profile id")?;
    request.source_prefix = normalize_relocation_prefix(&request.source_prefix, "source prefix")?;
    request.destination_prefix =
        normalize_relocation_prefix(&request.destination_prefix, "destination prefix")?;
    if request
        .source_prefix
        .eq_ignore_ascii_case(&request.destination_prefix)
        || path_has_prefix(&request.source_prefix, &request.destination_prefix)
        || path_has_prefix(&request.destination_prefix, &request.source_prefix)
    {
        return Err(AppError::Invalid(
            "source and destination prefixes must be separate paths".into(),
        ));
    }
    if let Some(ids) = request.resource_ids.as_mut() {
        if ids.is_empty() || ids.len() > MAX_RELOCATION_RESOURCE_IDS {
            return Err(AppError::Invalid(
                "relocation resource selection is empty or exceeds its limit".into(),
            ));
        }
        let mut seen = std::collections::HashSet::with_capacity(ids.len());
        for id in ids.iter() {
            valid_uuid(id, "relocation resource id")?;
            if !seen.insert(id.clone()) {
                return Err(AppError::Invalid(
                    "relocation resource selection contains duplicates".into(),
                ));
            }
        }
        ids.sort();
    }
    Ok(request)
}

fn relocation_preview_token(
    request: &ResourceRelocationPreviewRequest,
    changes: &[ResourceRelocationChange],
) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(&(request, changes))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn append_relocation_matches(
    changes: &mut Vec<ResourceRelocationChange>,
    resource_id: String,
    target_id: String,
    from_uri: String,
    source_prefix: &str,
    destination_prefix: &str,
) -> Result<(), AppError> {
    if let Some(to_uri) = replace_relocation_prefix(&from_uri, source_prefix, destination_prefix) {
        if to_uri.chars().count() > 4096 || !valid_windows_absolute_path(&to_uri) {
            return Err(AppError::Invalid(
                "relocation would create an invalid target path".into(),
            ));
        }
        changes.push(ResourceRelocationChange {
            resource_id,
            target_id,
            from_uri,
            to_uri,
        });
        if changes.len() > MAX_RELOCATION_CHANGES {
            return Err(AppError::Invalid(
                "relocation matches too many targets".into(),
            ));
        }
    }
    Ok(())
}

fn resource_relocation_preview_on_conn(
    conn: &Connection,
    request: &ResourceRelocationPreviewRequest,
) -> Result<ResourceRelocationPreview, AppError> {
    let request = canonicalize_relocation_request(request.clone())?;
    let profile_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&request.profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !profile_exists {
        return Err(AppError::NotFound);
    }
    let mut changes = Vec::new();
    if let Some(resource_ids) = request.resource_ids.as_ref() {
        for resource_id in resource_ids {
            let member: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
                params![request.profile_id, resource_id],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )?;
            if !member {
                return Err(AppError::NotFound);
            }
            let mut stmt = conn.prepare(
                "SELECT id,canonical_uri FROM resource_targets WHERE resource_id=?1 AND kind IN ('file','folder') ORDER BY id",
            )?;
            for row in stmt.query_map([resource_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (target_id, uri) = row?;
                append_relocation_matches(
                    &mut changes,
                    resource_id.clone(),
                    target_id,
                    uri,
                    &request.source_prefix,
                    &request.destination_prefix,
                )?;
            }
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT pr.resource_id,rt.id,rt.canonical_uri FROM profile_resources pr JOIN resource_targets rt ON rt.resource_id=pr.resource_id WHERE pr.profile_id=?1 AND rt.kind IN ('file','folder') ORDER BY pr.resource_id,rt.id LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![request.profile_id, (MAX_RELOCATION_CHANGES + 1) as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?;
        let mut scanned_targets = 0usize;
        for row in rows {
            scanned_targets += 1;
            if scanned_targets > MAX_RELOCATION_CHANGES {
                return Err(AppError::Invalid(
                    "profile has too many file targets for bounded relocation".into(),
                ));
            }
            let (resource_id, target_id, uri) = row?;
            append_relocation_matches(
                &mut changes,
                resource_id,
                target_id,
                uri,
                &request.source_prefix,
                &request.destination_prefix,
            )?;
        }
    }
    changes.sort_by(|a, b| (&a.resource_id, &a.target_id).cmp(&(&b.resource_id, &b.target_id)));
    let preview_token = relocation_preview_token(&request, &changes)?;
    Ok(ResourceRelocationPreview {
        profile_id: request.profile_id,
        source_prefix: request.source_prefix,
        destination_prefix: request.destination_prefix,
        resource_ids: request.resource_ids,
        preview_token,
        changes,
    })
}

fn resource_relocation_commit_on_conn(
    conn: &mut Connection,
    preview: &ResourceRelocationPreview,
) -> Result<ResourceRelocationResult, AppError> {
    let request = ResourceRelocationPreviewRequest {
        profile_id: preview.profile_id.clone(),
        source_prefix: preview.source_prefix.clone(),
        destination_prefix: preview.destination_prefix.clone(),
        resource_ids: preview.resource_ids.clone(),
    };
    let tx = conn.transaction()?;
    let current = resource_relocation_preview_on_conn(&tx, &request)?;
    if preview.preview_token != current.preview_token || preview.changes != current.changes {
        return Err(AppError::Invalid(
            "relocation preview is stale; create a new preview".into(),
        ));
    }
    let mut resource_ids = std::collections::BTreeSet::new();
    for change in &current.changes {
        let changed = tx.execute(
            "UPDATE resource_targets SET canonical_uri=?4 WHERE id=?1 AND resource_id=?2 AND canonical_uri=?3 AND kind IN ('file','folder') AND EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?5 AND resource_id=?2)",
            params![change.target_id, change.resource_id, change.from_uri, change.to_uri, current.profile_id],
        )?;
        if changed != 1 {
            return Err(AppError::Invalid(
                "relocation target changed while applying the preview".into(),
            ));
        }
        resource_ids.insert(change.resource_id.clone());
    }
    if !resource_ids.is_empty() {
        let updated_utc = now_utc();
        for resource_id in &resource_ids {
            let updated = tx.execute(
                "UPDATE resources SET updated_utc=?2 WHERE id=?1 AND EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?3 AND resource_id=?1)",
                params![resource_id, updated_utc, current.profile_id],
            )?;
            if updated != 1 {
                return Err(AppError::Invalid(
                    "relocation resource changed while applying the preview".into(),
                ));
            }
        }
    }
    tx.commit()?;
    Ok(ResourceRelocationResult {
        updated_target_count: current.changes.len(),
        updated_resource_ids: resource_ids.into_iter().collect(),
    })
}

#[tauri::command]
fn resource_relocation_preview(
    state: State<'_, AppState>,
    request: ResourceRelocationPreviewRequest,
) -> Result<ResourceRelocationPreview, AppError> {
    let conn = lock(&state)?;
    resource_relocation_preview_on_conn(&conn, &request)
}

#[tauri::command]
fn resource_relocation_commit(
    state: State<'_, AppState>,
    preview: ResourceRelocationPreview,
) -> Result<ResourceRelocationResult, AppError> {
    let mut conn = lock(&state)?;
    resource_relocation_commit_on_conn(&mut conn, &preview)
}

#[tauri::command]
fn resource_search(
    state: State<'_, AppState>,
    query: String,
    profile_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<Resource>, AppError> {
    validate_search_query(&query)?;
    let is_favorite = query
        .split_whitespace()
        .any(|part| part.eq_ignore_ascii_case("is:favorite"));
    let q = query
        .split_whitespace()
        .filter(|part| !part.eq_ignore_ascii_case("is:favorite"))
        .collect::<Vec<_>>()
        .join(" ");
    let conn = lock(&state)?;
    search_resources(
        &conn,
        &q,
        profile_id.as_deref(),
        is_favorite,
        limit.unwrap_or(50),
    )
}

const MAX_SEARCH_QUERY_BYTES: usize = 512;
const MAX_SEARCH_QUERY_CHARS: usize = 128;
const MAX_SEARCH_CANDIDATES: usize = 250;
const MAX_SEARCH_FIELD_CHARS: usize = 256;
const MAX_SEARCH_ALIAS_OR_TAG_FIELDS: usize = 32;

fn validate_search_query(query: &str) -> Result<(), AppError> {
    if query.len() > MAX_SEARCH_QUERY_BYTES || query.chars().count() > MAX_SEARCH_QUERY_CHARS {
        return Err(AppError::Invalid(format!(
            "search query exceeds {MAX_SEARCH_QUERY_CHARS} characters or {MAX_SEARCH_QUERY_BYTES} bytes"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TextMatch {
    quality: u8, // exact, prefix, substring, then edit-distance match
    field: u8,   // name, alias, tag, description
    distance: usize,
}

fn edit_distance_at_most(left: &str, right: &str, max_distance: usize) -> Option<usize> {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.len().abs_diff(right.len()) > max_distance {
        return None;
    }
    let cap = max_distance + 1;
    let mut previous: Vec<usize> = (0..=right.len())
        .map(|index| if index <= max_distance { index } else { cap })
        .collect();
    for (i, left_char) in left.iter().enumerate() {
        let row = i + 1;
        let mut current = vec![cap; right.len() + 1];
        if row <= max_distance {
            current[0] = row;
        }
        let start = row.saturating_sub(max_distance).max(1);
        let end = (row + max_distance).min(right.len());
        let mut row_min = current[0];
        for col in start..=end {
            current[col] = (previous[col] + 1)
                .min(current[col - 1] + 1)
                .min(previous[col - 1] + usize::from(*left_char != right[col - 1]));
            row_min = row_min.min(current[col]);
        }
        if row_min > max_distance {
            return None;
        }
        previous = current;
    }
    (previous[right.len()] <= max_distance).then_some(previous[right.len()])
}

fn best_field_match(query: &str, value: &str, field: u8) -> Option<TextMatch> {
    let query = query.to_lowercase();
    let value = value
        .chars()
        .take(MAX_SEARCH_FIELD_CHARS)
        .collect::<String>()
        .to_lowercase();
    if query.is_empty() {
        return None;
    }
    if query == value {
        return Some(TextMatch {
            quality: 0,
            field,
            distance: 0,
        });
    }
    if value.starts_with(&query) {
        return Some(TextMatch {
            quality: 1,
            field,
            distance: 0,
        });
    }
    if value.contains(&query) {
        return Some(TextMatch {
            quality: 2,
            field,
            distance: 0,
        });
    }

    let max_distance = if query.chars().count() >= 7 {
        2
    } else if query.chars().count() >= 4 {
        1
    } else {
        0
    };
    if max_distance == 0 {
        return None;
    }
    let distance = value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .filter_map(|word| edit_distance_at_most(&query, word, max_distance))
        .min()?;
    (distance <= max_distance).then_some(TextMatch {
        quality: 3,
        field,
        distance,
    })
}

fn resource_match(query: &str, resource: &Resource) -> Option<TextMatch> {
    let mut best = best_field_match(query, &resource.display_name, 0);
    for alias in resource.aliases.iter().take(MAX_SEARCH_ALIAS_OR_TAG_FIELDS) {
        best = best
            .into_iter()
            .chain(best_field_match(query, alias, 1))
            .min();
    }
    for tag in resource.tags.iter().take(MAX_SEARCH_ALIAS_OR_TAG_FIELDS) {
        best = best
            .into_iter()
            .chain(best_field_match(query, tag, 2))
            .min();
    }
    if let Some(description) = &resource.description {
        best = best
            .into_iter()
            .chain(best_field_match(query, description, 3))
            .min();
    }
    best
}

fn recency_rank(last_launched: Option<&str>) -> i64 {
    let Some(last) = last_launched else {
        return i64::MAX;
    };
    chrono::DateTime::parse_from_rfc3339(last)
        .map(|dt| -dt.timestamp())
        .unwrap_or(i64::MAX)
}

fn search_resources(
    conn: &Connection,
    q: &str,
    profile_id: Option<&str>,
    is_favorite: bool,
    limit: usize,
) -> Result<Vec<Resource>, AppError> {
    let query = q.trim();
    validate_search_query(query)?;
    let lowered = query.to_lowercase();
    let prefix = format!("{lowered}%");
    let contains = format!("%{lowered}%");
    let mut stmt = conn.prepare("SELECT r.id,r.resource_type,r.display_name,r.description,r.icon,r.default_action,r.created_utc,r.updated_utc FROM resources r WHERE (?1 IS NULL OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND pr.profile_id=?1)) AND (?2=0 OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND (?1 IS NULL OR pr.profile_id=?1) AND pr.favorite=1)) ORDER BY CASE WHEN lower(r.display_name)=?3 THEN 0 WHEN lower(r.display_name) LIKE ?4 THEN 1 WHEN lower(r.display_name) LIKE ?5 THEN 2 WHEN EXISTS(SELECT 1 FROM resource_terms a WHERE a.resource_id=r.id AND a.kind='alias' AND lower(a.value) LIKE ?5) OR EXISTS(SELECT 1 FROM resource_terms t WHERE t.resource_id=r.id AND t.kind='tag' AND lower(t.value) LIKE ?5) THEN 3 ELSE 4 END, r.display_name COLLATE NOCASE LIMIT ?6")?;
    let rows = stmt.query_map(
        params![
            profile_id,
            if is_favorite { 1i64 } else { 0i64 },
            lowered,
            prefix,
            contains,
            MAX_SEARCH_CANDIDATES as i64
        ],
        resource_from_row,
    )?;
    let mut ranked = Vec::new();
    for row in rows {
        let mut resource = row?;
        load_resource_details_bounded(conn, &mut resource, profile_id)?;
        let text_match = if query.is_empty() {
            Some(TextMatch {
                quality: 4,
                field: 0,
                distance: 0,
            })
        } else {
            resource_match(query, &resource)
        };
        if let Some(text_match) = text_match {
            ranked.push((
                text_match,
                !resource.favorite,
                recency_rank(resource.last_launched_utc.as_deref()),
                resource.display_name.to_lowercase(),
                resource,
            ));
        }
    }
    ranked.sort_by(|a, b| (&a.0, a.1, a.2, &a.3).cmp(&(&b.0, b.1, b.2, &b.3)));
    Ok(ranked
        .into_iter()
        .take(limit.clamp(1, 500))
        .map(|(_, _, _, _, resource)| resource)
        .collect())
}

const MAX_LAYOUT_BYTES: usize = 1_048_576;

fn is_valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

fn validate_layout(layout: &DashboardLayout) -> Result<(), AppError> {
    if !is_valid_uuid(&layout.profile_id) {
        return Err(AppError::Invalid("profileId must be a UUID".into()));
    }
    if layout.version != 1 || !matches!(layout.surface.as_str(), "dashboard" | "dock") {
        return Err(AppError::Invalid(
            "unsupported layout version or surface".into(),
        ));
    }
    if layout.cards.len() > 10_000 {
        return Err(AppError::Invalid("layout exceeds 10000 cards".into()));
    }
    let mut ids = std::collections::HashSet::with_capacity(layout.cards.len());
    for card in &layout.cards {
        if !is_valid_uuid(&card.resource_id)
            || !ids.insert(&card.resource_id)
            || card.x < 0
            || card.x > 11
            || card.y < 0
            || card.y > 9999
            || card.w < 1
            || card.w > 12
            || card.h < 1
            || card.h > 100
            || card.x + card.w > 12
            || !matches!(
                card.card_type.as_str(),
                "icon" | "compact" | "info" | "group"
            )
        {
            return Err(AppError::Invalid(
                "layout card violates UUID, bounds, or uniqueness".into(),
            ));
        }
    }
    Ok(())
}

fn layout_get_from_conn(
    conn: &Connection,
    profile_id: &str,
    surface: &str,
) -> Result<DashboardLayout, AppError> {
    if !is_valid_uuid(profile_id) {
        return Err(AppError::Invalid("profileId must be a UUID".into()));
    }
    if !matches!(surface, "dashboard" | "dock") {
        return Err(AppError::Invalid("invalid layout surface".into()));
    }
    let profile_exists: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get(0),
    )?;
    if profile_exists == 0 {
        return Err(AppError::NotFound);
    }

    let stored: Option<(i64, String)> = conn
        .query_row(
            "SELECT version,layout_json FROM layouts WHERE profile_id=?1 AND surface=?2",
            params![profile_id, surface],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((row_version, json)) = stored else {
        return Ok(DashboardLayout {
            profile_id: profile_id.to_owned(),
            surface: surface.to_owned(),
            version: 1,
            cards: Vec::new(),
        });
    };
    if json.len() > MAX_LAYOUT_BYTES {
        return Err(AppError::Invalid("stored layout exceeds 1 MiB".into()));
    }
    let layout: DashboardLayout = serde_json::from_str(&json)?;
    validate_layout(&layout)?;
    if layout.profile_id != profile_id || layout.surface != surface || layout.version != row_version
    {
        return Err(AppError::Invalid(
            "stored layout identity/version does not match its row and request".into(),
        ));
    }
    let mut stmt = conn.prepare("SELECT EXISTS(SELECT 1 FROM resources r JOIN profile_resources pr ON pr.resource_id=r.id WHERE r.id=?1 AND pr.profile_id=?2)")?;
    let mut filtered = layout;
    let mut cards = Vec::with_capacity(filtered.cards.len());
    for card in filtered.cards.drain(..) {
        if stmt.query_row(params![card.resource_id, profile_id], |row| {
            row.get::<_, i64>(0)
        })? != 0
        {
            cards.push(card);
        }
    }
    filtered.cards = cards;
    Ok(filtered)
}

fn layout_save_to_conn(
    conn: &mut Connection,
    layout: &DashboardLayout,
) -> Result<DashboardLayout, AppError> {
    // Tauri's IPC transport has a host-controlled payload limit. This command independently
    // rejects oversized serialized layouts before validation, opening a transaction, or touching state.
    let json = serde_json::to_string(layout)?;
    if json.len() > MAX_LAYOUT_BYTES {
        return Err(AppError::Invalid("layout exceeds 1 MiB".into()));
    }
    validate_layout(layout)?;
    let tx = conn.transaction()?;
    let profile_exists: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&layout.profile_id],
        |row| row.get(0),
    )?;
    if profile_exists == 0 {
        return Err(AppError::NotFound);
    }
    let mut stmt = tx.prepare("SELECT EXISTS(SELECT 1 FROM resources r JOIN profile_resources pr ON pr.resource_id=r.id WHERE r.id=?1 AND pr.profile_id=?2)")?;
    for card in &layout.cards {
        if stmt.query_row(params![card.resource_id, layout.profile_id], |row| {
            row.get::<_, i64>(0)
        })? == 0
        {
            return Err(AppError::Invalid(
                "layout contains a resource not assigned to this profile".into(),
            ));
        }
    }
    drop(stmt);
    tx.execute("INSERT INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(profile_id,surface) DO UPDATE SET version=excluded.version,layout_json=excluded.layout_json,updated_utc=excluded.updated_utc", params![layout.profile_id,layout.surface,layout.version,json,now_utc()])?;
    tx.commit()?;
    Ok(layout.clone())
}

#[tauri::command]
fn layout_get(
    state: State<'_, AppState>,
    profile_id: String,
    surface: String,
) -> Result<DashboardLayout, AppError> {
    let conn = lock(&state)?;
    layout_get_from_conn(&conn, &profile_id, &surface)
}

#[tauri::command]
fn layout_save(
    state: State<'_, AppState>,
    layout: DashboardLayout,
) -> Result<DashboardLayout, AppError> {
    let mut conn = lock(&state)?;
    layout_save_to_conn(&mut conn, &layout)
}

fn resource_relationship_list_on_conn(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<ResourceRelationship>, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare("SELECT id,profile_id,source_resource_id,target_resource_id,label FROM resource_relationships WHERE profile_id=?1 ORDER BY label COLLATE NOCASE,id")?;
    let relationships = stmt
        .query_map([profile_id], |row| {
            Ok(ResourceRelationship {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                source_resource_id: row.get(2)?,
                target_resource_id: row.get(3)?,
                label: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(relationships)
}

fn resource_relationship_save_on_conn(
    conn: &mut Connection,
    mut relationship: ResourceRelationship,
) -> Result<ResourceRelationship, AppError> {
    valid_uuid(&relationship.id, "relationship id")?;
    valid_uuid(&relationship.profile_id, "profile id")?;
    valid_uuid(&relationship.source_resource_id, "source resource id")?;
    valid_uuid(&relationship.target_resource_id, "target resource id")?;
    relationship.label = relationship.label.trim().to_owned();
    if relationship.source_resource_id == relationship.target_resource_id {
        return Err(AppError::Invalid(
            "a resource cannot be related to itself".into(),
        ));
    }
    if relationship.label.is_empty() || relationship.label.chars().count() > 80 {
        return Err(AppError::Invalid(
            "relationship label must contain 1 to 80 characters".into(),
        ));
    }
    // Edges are undirected, so canonical ordering makes reversed inputs identical.
    if relationship.source_resource_id > relationship.target_resource_id {
        std::mem::swap(
            &mut relationship.source_resource_id,
            &mut relationship.target_resource_id,
        );
    }
    let tx = conn.transaction()?;
    let profile_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&relationship.profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !profile_exists {
        return Err(AppError::NotFound);
    }
    for resource_id in [
        &relationship.source_resource_id,
        &relationship.target_resource_id,
    ] {
        let member: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
            params![relationship.profile_id, resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !member {
            return Err(AppError::Invalid(
                "both related resources must belong to the profile".into(),
            ));
        }
    }
    let existing_id: Option<(String, String, String)> = tx
        .query_row(
            "SELECT profile_id,source_resource_id,target_resource_id FROM resource_relationships WHERE id=?1",
            [&relationship.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if existing_id
        .as_ref()
        .is_some_and(|(profile, source, target)| {
            profile != &relationship.profile_id
                || source != &relationship.source_resource_id
                || target != &relationship.target_resource_id
        })
    {
        return Err(AppError::NotFound);
    }
    let duplicate_pair: Option<String> = tx
        .query_row(
            "SELECT id FROM resource_relationships WHERE profile_id=?1 AND source_resource_id=?2 AND target_resource_id=?3",
            params![relationship.profile_id, relationship.source_resource_id, relationship.target_resource_id],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate_pair
        .as_deref()
        .is_some_and(|id| id != relationship.id)
    {
        return Err(AppError::Invalid(
            "this resource pair already has a relationship".into(),
        ));
    }
    if existing_id.is_none() {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM resource_relationships WHERE profile_id=?1",
            [&relationship.profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_RESOURCE_RELATIONSHIPS_PER_PROFILE {
            return Err(AppError::Invalid(
                "profile has reached its resource relationship limit".into(),
            ));
        }
        let entity_collision: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 UNION ALL SELECT 1 FROM workspace_snapshots WHERE id=?1)", [&relationship.id], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
        if entity_collision {
            return Err(AppError::Invalid("relationship ID is already used".into()));
        }
    }
    tx.execute("INSERT INTO resource_relationships(id,profile_id,source_resource_id,target_resource_id,label) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET label=excluded.label", params![relationship.id, relationship.profile_id, relationship.source_resource_id, relationship.target_resource_id, relationship.label])?;
    tx.commit()?;
    Ok(relationship)
}

fn resource_relationship_delete_on_conn(
    conn: &Connection,
    profile_id: &str,
    id: &str,
) -> Result<(), AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "relationship id")?;
    if conn.execute(
        "DELETE FROM resource_relationships WHERE id=?1 AND profile_id=?2",
        params![id, profile_id],
    )? == 0
    {
        return Err(AppError::NotFound);
    }
    Ok(())
}

#[tauri::command]
fn resource_relationship_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<ResourceRelationship>, AppError> {
    let conn = lock(&state)?;
    resource_relationship_list_on_conn(&conn, &profile_id)
}

#[tauri::command]
fn resource_relationship_save(
    state: State<'_, AppState>,
    relationship: ResourceRelationship,
) -> Result<ResourceRelationship, AppError> {
    let mut conn = lock(&state)?;
    resource_relationship_save_on_conn(&mut conn, relationship)
}

#[tauri::command]
fn resource_relationship_delete(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), AppError> {
    let conn = lock(&state)?;
    resource_relationship_delete_on_conn(&conn, &profile_id, &id)
}

fn workspace_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceSnapshot> {
    let json: String = row.get(4)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn workspace_snapshot_list_on_conn(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<WorkspaceSnapshot>, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare("SELECT id,profile_id,name,created_utc,snapshot_json FROM workspace_snapshots WHERE profile_id=?1 ORDER BY created_utc DESC,id")?;
    let snapshots = stmt
        .query_map([profile_id], workspace_snapshot_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    for snapshot in &snapshots {
        if snapshot.profile_id != profile_id
            || !is_valid_uuid(&snapshot.id)
            || snapshot.resource_ids.len() > MAX_WORKSPACE_SNAPSHOT_RESOURCE_IDS
        {
            return Err(AppError::Invalid(
                "stored workspace snapshot identity or limits are invalid".into(),
            ));
        }
        validate_layout(&snapshot.dashboard)?;
        validate_layout(&snapshot.dock)?;
        if snapshot.dashboard.profile_id != profile_id
            || snapshot.dashboard.surface != "dashboard"
            || snapshot.dock.profile_id != profile_id
            || snapshot.dock.surface != "dock"
        {
            return Err(AppError::Invalid(
                "stored workspace snapshot layouts do not match profile and surface".into(),
            ));
        }
    }
    Ok(snapshots)
}

fn workspace_snapshot_create_on_conn(
    conn: &mut Connection,
    profile_id: &str,
    name: &str,
) -> Result<WorkspaceSnapshot, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::Invalid(
            "snapshot name must contain 1 to 100 characters".into(),
        ));
    }
    let tx = conn.transaction()?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM workspace_snapshots WHERE profile_id=?1",
        [profile_id],
        |row| row.get(0),
    )?;
    if count >= MAX_WORKSPACE_SNAPSHOTS_PER_PROFILE {
        return Err(AppError::Invalid(
            "profile has reached its workspace snapshot limit".into(),
        ));
    }
    let name_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspace_snapshots WHERE profile_id=?1 AND name=?2 COLLATE NOCASE)",
        params![profile_id, name],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if name_exists {
        return Err(AppError::Invalid(
            "workspace snapshot name already exists in this profile".into(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    let created_utc = now_utc();
    let mut resource_stmt = tx.prepare(
        "SELECT resource_id FROM profile_resources WHERE profile_id=?1 ORDER BY resource_id",
    )?;
    let resource_ids = resource_stmt
        .query_map([profile_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(resource_stmt);
    if resource_ids.len() > MAX_WORKSPACE_SNAPSHOT_RESOURCE_IDS {
        return Err(AppError::Invalid(
            "workspace snapshot contains too many resources".into(),
        ));
    }
    let snapshot = WorkspaceSnapshot {
        id,
        profile_id: profile_id.to_owned(),
        name: name.to_owned(),
        created_utc,
        resource_ids,
        dashboard: layout_get_from_conn(&tx, profile_id, "dashboard")?,
        dock: layout_get_from_conn(&tx, profile_id, "dock")?,
    };
    let json = serde_json::to_string(&snapshot)?;
    if json.len() > MAX_WORKSPACE_SNAPSHOT_BYTES {
        return Err(AppError::Invalid("workspace snapshot exceeds 1 MiB".into()));
    }
    tx.execute("INSERT INTO workspace_snapshots(id,profile_id,name,created_utc,snapshot_json) VALUES (?1,?2,?3,?4,?5)", params![snapshot.id,snapshot.profile_id,snapshot.name,snapshot.created_utc,json])?;
    tx.commit()?;
    Ok(snapshot)
}

fn workspace_snapshot_delete_on_conn(
    conn: &Connection,
    profile_id: &str,
    id: &str,
) -> Result<(), AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "snapshot id")?;
    if conn.execute(
        "DELETE FROM workspace_snapshots WHERE id=?1 AND profile_id=?2",
        params![id, profile_id],
    )? == 0
    {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn workspace_snapshot_restore_on_conn(
    conn: &mut Connection,
    profile_id: &str,
    id: &str,
) -> Result<WorkspaceSnapshotRestoreResult, AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "snapshot id")?;
    let tx = conn.transaction()?;
    let stored: Option<String> = tx
        .query_row(
            "SELECT snapshot_json FROM workspace_snapshots WHERE id=?1 AND profile_id=?2",
            params![id, profile_id],
            |row| row.get(0),
        )
        .optional()?;
    let json = stored.ok_or(AppError::NotFound)?;
    if json.len() > MAX_WORKSPACE_SNAPSHOT_BYTES {
        return Err(AppError::Invalid("stored snapshot exceeds 1 MiB".into()));
    }
    let snapshot: WorkspaceSnapshot = serde_json::from_str(&json)?;
    if snapshot.id != id
        || snapshot.profile_id != profile_id
        || snapshot.resource_ids.len() > MAX_WORKSPACE_SNAPSHOT_RESOURCE_IDS
    {
        return Err(AppError::Invalid(
            "stored workspace snapshot identity or limits are invalid".into(),
        ));
    }
    validate_layout(&snapshot.dashboard)?;
    validate_layout(&snapshot.dock)?;
    if snapshot.dashboard.profile_id != profile_id
        || snapshot.dashboard.surface != "dashboard"
        || snapshot.dock.profile_id != profile_id
        || snapshot.dock.surface != "dock"
    {
        return Err(AppError::Invalid(
            "stored workspace snapshot layouts do not match profile and surface".into(),
        ));
    }
    let mut current_stmt =
        tx.prepare("SELECT resource_id FROM profile_resources WHERE profile_id=?1")?;
    let current = current_stmt
        .query_map([profile_id], |row| row.get::<_, String>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    drop(current_stmt);
    let mut missing = std::collections::BTreeSet::new();
    for resource_id in &snapshot.resource_ids {
        valid_uuid(resource_id, "snapshot resource id")?;
        if !current.contains(resource_id) {
            missing.insert(resource_id.clone());
        }
    }
    for resource_id in snapshot
        .dashboard
        .cards
        .iter()
        .chain(snapshot.dock.cards.iter())
        .map(|card| &card.resource_id)
    {
        if !current.contains(resource_id) {
            missing.insert(resource_id.clone());
        }
    }
    let mut restore_layout = |saved: &DashboardLayout| -> Result<(), AppError> {
        let existing = layout_get_from_conn(&tx, profile_id, &saved.surface)?;
        let saved_ids = saved
            .cards
            .iter()
            .map(|card| card.resource_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut cards = saved
            .cards
            .iter()
            .filter(|card| current.contains(&card.resource_id))
            .cloned()
            .collect::<Vec<_>>();
        // Preserve workspace cards added since the checkpoint; saved positions win on overlaps.
        cards.extend(
            existing
                .cards
                .into_iter()
                .filter(|card| !saved_ids.contains(card.resource_id.as_str())),
        );
        let merged = DashboardLayout {
            profile_id: profile_id.to_owned(),
            surface: saved.surface.clone(),
            version: saved.version,
            cards,
        };
        let serialized = serde_json::to_string(&merged)?;
        if serialized.len() > MAX_LAYOUT_BYTES {
            return Err(AppError::Invalid("restored layout exceeds 1 MiB".into()));
        }
        validate_layout(&merged)?;
        tx.execute("INSERT INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(profile_id,surface) DO UPDATE SET version=excluded.version,layout_json=excluded.layout_json,updated_utc=excluded.updated_utc", params![profile_id,merged.surface,merged.version,serialized,now_utc()])?;
        Ok(())
    };
    restore_layout(&snapshot.dashboard)?;
    restore_layout(&snapshot.dock)?;
    drop(restore_layout);
    tx.commit()?;
    Ok(WorkspaceSnapshotRestoreResult {
        snapshot_id: id.to_owned(),
        restored_layouts: 2,
        missing_resource_ids: missing.into_iter().collect(),
    })
}

#[tauri::command]
fn workspace_snapshot_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<WorkspaceSnapshot>, AppError> {
    let conn = lock(&state)?;
    workspace_snapshot_list_on_conn(&conn, &profile_id)
}

#[tauri::command]
fn workspace_snapshot_create(
    state: State<'_, AppState>,
    profile_id: String,
    name: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let mut conn = lock(&state)?;
    workspace_snapshot_create_on_conn(&mut conn, &profile_id, &name)
}

#[tauri::command]
fn workspace_snapshot_delete(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), AppError> {
    let conn = lock(&state)?;
    workspace_snapshot_delete_on_conn(&conn, &profile_id, &id)
}

#[tauri::command]
fn workspace_snapshot_restore(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<WorkspaceSnapshotRestoreResult, AppError> {
    let mut conn = lock(&state)?;
    workspace_snapshot_restore_on_conn(&mut conn, &profile_id, &id)
}

#[tauri::command]
fn profile_list(state: State<'_, AppState>) -> Result<Vec<Profile>, AppError> {
    let conn = lock(&state)?;
    let mut stmt=conn.prepare("SELECT id,name,theme_id,settings_json,created_utc,updated_utc FROM profiles ORDER BY name COLLATE NOCASE")?;
    let profiles = stmt
        .query_map([], profile_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(profiles)
}

fn profile_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: r.get(0)?,
        name: r.get(1)?,
        theme_id: r.get(2)?,
        settings_json: r.get(3)?,
        created_utc: r.get(4)?,
        updated_utc: r.get(5)?,
    })
}

fn default_profile_id(conn: &Connection) -> Result<String, AppError> {
    Ok(conn.query_row(
        "SELECT id FROM profiles WHERE name='Default' COLLATE NOCASE",
        [],
        |row| row.get(0),
    )?)
}

#[tauri::command]
fn profile_create(state: State<'_, AppState>, name: String) -> Result<Profile, AppError> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::Invalid(
            "profile name must contain 1 to 80 printable characters".into(),
        ));
    }
    let ts = now_utc();
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: name.trim().into(),
        theme_id: "system".into(),
        settings_json: "{}".into(),
        created_utc: ts.clone(),
        updated_utc: ts,
    };
    lock(&state)?.execute(
        "INSERT INTO profiles VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            profile.id,
            profile.name,
            profile.theme_id,
            profile.settings_json,
            profile.created_utc,
            profile.updated_utc
        ],
    )?;
    Ok(profile)
}

fn profile_theme_set_on_conn(
    conn: &mut Connection,
    profile_id: &str,
    theme: &ProfileThemeInput,
) -> Result<Profile, AppError> {
    valid_uuid(profile_id, "profile id")?;
    if !["light", "dark", "system"].contains(&theme.mode.as_str())
        || !valid_hex_color(&theme.accent)
        || theme.density.as_ref().is_some_and(|density| {
            !["compact", "balanced", "comfortable"].contains(&density.as_str())
        })
    {
        return Err(AppError::Invalid("invalid profile theme".into()));
    }
    let tx = conn.transaction()?;
    let settings_json: String = tx
        .query_row(
            "SELECT settings_json FROM profiles WHERE id=?1",
            [profile_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    let mut settings: serde_json::Value = serde_json::from_str(&settings_json)?;
    let Some(settings) = settings.as_object_mut() else {
        return Err(AppError::Invalid(
            "profile settings must be a JSON object".into(),
        ));
    };
    settings.insert(
        "accent".into(),
        serde_json::Value::String(theme.accent.clone()),
    );
    if let Some(density) = &theme.density {
        settings.insert("density".into(), serde_json::Value::String(density.clone()));
    } else {
        settings.remove("density");
    }
    if settings.len() > 50 {
        return Err(AppError::Invalid(
            "profile settings exceed 50 properties".into(),
        ));
    }
    let updated_utc = now_utc();
    tx.execute(
        "UPDATE profiles SET theme_id=?2,settings_json=?3,updated_utc=?4 WHERE id=?1",
        params![
            profile_id,
            theme.mode,
            serde_json::Value::Object(settings.clone()).to_string(),
            updated_utc
        ],
    )?;
    let profile = tx.query_row(
        "SELECT id,name,theme_id,settings_json,created_utc,updated_utc FROM profiles WHERE id=?1",
        [profile_id],
        profile_from_row,
    )?;
    tx.commit()?;
    Ok(profile)
}

#[tauri::command]
fn profile_theme_set(
    state: State<'_, AppState>,
    profile_id: String,
    theme: ProfileThemeInput,
) -> Result<Profile, AppError> {
    let mut conn = lock(&state)?;
    profile_theme_set_on_conn(&mut conn, &profile_id, &theme)
}

fn validate_smart_collection_fields(
    id: &str,
    name: &str,
    filters: &SmartCollectionFilters,
) -> Result<(), AppError> {
    valid_uuid(id, "smart collection id")?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(AppError::Invalid(
            "smart collection name must contain 1 to 100 printable characters".into(),
        ));
    }
    if filters
        .resource_type
        .as_ref()
        .is_some_and(|kind| !["file", "folder", "app", "shortcut", "url"].contains(&kind.as_str()))
    {
        return Err(AppError::Invalid(
            "unsupported smart collection resource type".into(),
        ));
    }
    for (field, value, maximum) in [
        ("category", filters.category.as_ref(), 80usize),
        ("tag", filters.tag.as_ref(), 80usize),
        ("text", filters.text.as_ref(), 200usize),
    ] {
        if value.is_some_and(|value| {
            value.trim().is_empty()
                || value.chars().count() > maximum
                || value.chars().any(char::is_control)
        }) {
            return Err(AppError::Invalid(format!(
                "smart collection {field} must contain 1 to {maximum} printable characters"
            )));
        }
    }
    let encoded = serde_json::to_vec(filters)?;
    if encoded.len() > 4096 {
        return Err(AppError::Invalid(
            "smart collection filters exceed size limit".into(),
        ));
    }
    Ok(())
}

fn validate_smart_collection(collection: &SmartCollection) -> Result<(), AppError> {
    valid_uuid(&collection.profile_id, "profile id")?;
    validate_smart_collection_fields(&collection.id, &collection.name, &collection.filters)
}

fn smart_collection_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SmartCollection> {
    let filters_json: String = row.get(3)?;
    let filters = serde_json::from_str(&filters_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            filters_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(SmartCollection {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        name: row.get(2)?,
        filters,
    })
}

fn smart_collection_list_on_conn(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<SmartCollection>, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare(
        "SELECT id,profile_id,name,filters_json FROM smart_collections WHERE profile_id=?1 ORDER BY name COLLATE NOCASE,id",
    )?;
    let rows = stmt.query_map([profile_id], smart_collection_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[tauri::command]
fn smart_collection_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<SmartCollection>, AppError> {
    let conn = lock(&state)?;
    smart_collection_list_on_conn(&conn, &profile_id)
}

fn smart_collection_save_on_conn(
    conn: &mut Connection,
    mut collection: SmartCollection,
) -> Result<SmartCollection, AppError> {
    collection.name = collection.name.trim().to_string();
    validate_smart_collection(&collection)?;
    let tx = conn.transaction()?;
    let profile_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&collection.profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !profile_exists {
        return Err(AppError::NotFound);
    }
    let existing_profile: Option<String> = tx
        .query_row(
            "SELECT profile_id FROM smart_collections WHERE id=?1",
            [&collection.id],
            |row| row.get(0),
        )
        .optional()?;
    if existing_profile
        .as_deref()
        .is_some_and(|profile_id| profile_id != collection.profile_id)
    {
        return Err(AppError::NotFound);
    }
    let filters_json = serde_json::to_string(&collection.filters)?;
    let timestamp = now_utc();
    if existing_profile.is_none() {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM smart_collections WHERE profile_id=?1",
            [&collection.profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_SMART_COLLECTIONS_PER_PROFILE {
            return Err(AppError::Invalid(
                "profile already has the maximum number of smart collections".into(),
            ));
        }
    }
    if existing_profile.is_some() {
        tx.execute(
            "UPDATE smart_collections SET name=?2,filters_json=?3,updated_utc=?4 WHERE id=?1 AND profile_id=?5",
            params![collection.id, collection.name, filters_json, timestamp, collection.profile_id],
        )?;
    } else {
        tx.execute(
            "INSERT INTO smart_collections(id,profile_id,name,filters_json,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?5)",
            params![collection.id, collection.profile_id, collection.name, filters_json, timestamp],
        )?;
    }
    tx.commit()?;
    Ok(collection)
}

#[tauri::command]
fn smart_collection_save(
    state: State<'_, AppState>,
    collection: SmartCollection,
) -> Result<SmartCollection, AppError> {
    let mut conn = lock(&state)?;
    smart_collection_save_on_conn(&mut conn, collection)
}

fn smart_collection_delete_on_conn(
    conn: &Connection,
    profile_id: &str,
    id: &str,
) -> Result<(), AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "smart collection id")?;
    let changed = conn.execute(
        "DELETE FROM smart_collections WHERE id=?1 AND profile_id=?2",
        params![id, profile_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

#[tauri::command]
fn smart_collection_delete(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), AppError> {
    let conn = lock(&state)?;
    smart_collection_delete_on_conn(&conn, &profile_id, &id)
}

fn validate_dynamic_shortcut(shortcut: &DynamicShortcut) -> Result<(), AppError> {
    valid_uuid(&shortcut.id, "dynamic shortcut id")?;
    valid_uuid(&shortcut.profile_id, "profile id")?;
    valid_uuid(&shortcut.resource_id, "resource id")?;
    if shortcut.name.trim().is_empty()
        || shortcut.name.trim().chars().count() > 100
        || shortcut.name.chars().any(char::is_control)
    {
        return Err(AppError::Invalid(
            "dynamic shortcut name must contain 1 to 100 printable characters".into(),
        ));
    }
    if let Some(target_id) = shortcut.target_id.as_deref() {
        valid_uuid(target_id, "target id")?;
    }
    Ok(())
}

fn validate_workspace_sequence_fields(
    id: &str,
    name: &str,
    steps: &[WorkspaceSequenceStep],
) -> Result<(), AppError> {
    valid_uuid(id, "workspace sequence id")?;
    if name.trim().is_empty()
        || name.trim().chars().count() > 100
        || name.chars().any(char::is_control)
    {
        return Err(AppError::Invalid(
            "workspace sequence name must contain 1 to 100 printable characters".into(),
        ));
    }
    if steps.is_empty() || steps.len() > MAX_WORKSPACE_SEQUENCE_STEPS {
        return Err(AppError::Invalid(format!(
            "workspace sequence must contain 1 to {MAX_WORKSPACE_SEQUENCE_STEPS} steps"
        )));
    }
    let mut step_ids = std::collections::HashSet::new();
    for step in steps {
        valid_uuid(&step.id, "workspace sequence step id")?;
        valid_uuid(&step.resource_id, "workspace sequence resource id")?;
        if step.id == id || !step_ids.insert(step.id.as_str()) {
            return Err(AppError::Invalid(
                "workspace sequence step IDs must be unique".into(),
            ));
        }
        if let Some(target_id) = step.target_id.as_deref() {
            valid_uuid(target_id, "workspace sequence target id")?;
        }
    }
    Ok(())
}

fn dynamic_shortcut_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DynamicShortcut> {
    Ok(DynamicShortcut {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        name: row.get(2)?,
        resource_id: row.get(3)?,
        target_id: row.get(4)?,
    })
}

fn dynamic_shortcut_list_on_conn(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<DynamicShortcut>, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare(
        "SELECT id,profile_id,name,resource_id,target_id FROM dynamic_shortcuts WHERE profile_id=?1 ORDER BY name COLLATE NOCASE,id",
    )?;
    let rows = stmt.query_map([profile_id], dynamic_shortcut_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[tauri::command]
fn dynamic_shortcut_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<DynamicShortcut>, AppError> {
    let conn = lock(&state)?;
    dynamic_shortcut_list_on_conn(&conn, &profile_id)
}

fn dynamic_shortcut_save_on_conn(
    conn: &mut Connection,
    mut shortcut: DynamicShortcut,
) -> Result<DynamicShortcut, AppError> {
    shortcut.name = shortcut.name.trim().to_string();
    validate_dynamic_shortcut(&shortcut)?;
    let tx = conn.transaction()?;
    let member: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles p JOIN profile_resources pr ON pr.profile_id=p.id WHERE p.id=?1 AND pr.resource_id=?2)",
        params![shortcut.profile_id, shortcut.resource_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !member {
        return Err(AppError::NotFound);
    }
    if let Some(target_id) = shortcut.target_id.as_deref() {
        let owned_active: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)",
            params![target_id, shortcut.resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !owned_active {
            return Err(AppError::NotFound);
        }
    }
    let existing_profile: Option<String> = tx
        .query_row(
            "SELECT profile_id FROM dynamic_shortcuts WHERE id=?1",
            [&shortcut.id],
            |row| row.get(0),
        )
        .optional()?;
    if existing_profile
        .as_deref()
        .is_some_and(|profile_id| profile_id != shortcut.profile_id)
    {
        return Err(AppError::NotFound);
    }
    if existing_profile.is_none() {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM dynamic_shortcuts WHERE profile_id=?1",
            [&shortcut.profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_DYNAMIC_SHORTCUTS_PER_PROFILE {
            return Err(AppError::Invalid(
                "profile already has the maximum number of dynamic shortcuts".into(),
            ));
        }
    }
    let now = now_utc();
    if existing_profile.is_some() {
        tx.execute(
            "UPDATE dynamic_shortcuts SET name=?2,resource_id=?3,target_id=?4,updated_utc=?5 WHERE id=?1 AND profile_id=?6",
            params![shortcut.id, shortcut.name, shortcut.resource_id, shortcut.target_id, now, shortcut.profile_id],
        )?;
    } else {
        tx.execute(
            "INSERT INTO dynamic_shortcuts(id,profile_id,name,resource_id,target_id,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?6,?6)",
            params![shortcut.id, shortcut.profile_id, shortcut.name, shortcut.resource_id, shortcut.target_id, now],
        )?;
    }
    tx.commit()?;
    Ok(shortcut)
}

#[tauri::command]
fn dynamic_shortcut_save(
    state: State<'_, AppState>,
    shortcut: DynamicShortcut,
) -> Result<DynamicShortcut, AppError> {
    let mut conn = lock(&state)?;
    dynamic_shortcut_save_on_conn(&mut conn, shortcut)
}

fn dynamic_shortcut_delete_on_conn(
    conn: &Connection,
    profile_id: &str,
    id: &str,
) -> Result<(), AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "dynamic shortcut id")?;
    let changed = conn.execute(
        "DELETE FROM dynamic_shortcuts WHERE id=?1 AND profile_id=?2",
        params![id, profile_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

#[tauri::command]
fn dynamic_shortcut_delete(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), AppError> {
    let conn = lock(&state)?;
    dynamic_shortcut_delete_on_conn(&conn, &profile_id, &id)
}

fn workspace_sequence_list_on_conn(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<WorkspaceSequence>, AppError> {
    valid_uuid(profile_id, "profile id")?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare("SELECT id,profile_id,name FROM workspace_sequences WHERE profile_id=?1 ORDER BY name COLLATE NOCASE,id")?;
    let mut sequences = stmt
        .query_map([profile_id], |row| {
            Ok(WorkspaceSequence {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                name: row.get(2)?,
                steps: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for sequence in &mut sequences {
        let mut steps = conn.prepare("SELECT id,resource_id,target_id FROM workspace_sequence_steps WHERE profile_id=?1 AND sequence_id=?2 ORDER BY position")?;
        sequence.steps = steps
            .query_map(params![profile_id, sequence.id], |row| {
                Ok(WorkspaceSequenceStep {
                    id: row.get(0)?,
                    resource_id: row.get(1)?,
                    target_id: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(sequences)
}

#[tauri::command]
fn workspace_sequence_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Vec<WorkspaceSequence>, AppError> {
    let conn = lock(&state)?;
    workspace_sequence_list_on_conn(&conn, &profile_id)
}

fn workspace_sequence_save_on_conn(
    conn: &mut Connection,
    mut sequence: WorkspaceSequence,
) -> Result<WorkspaceSequence, AppError> {
    sequence.name = sequence.name.trim().to_string();
    validate_workspace_sequence_fields(&sequence.id, &sequence.name, &sequence.steps)?;
    valid_uuid(&sequence.profile_id, "profile id")?;
    let tx = conn.transaction()?;
    let profile_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&sequence.profile_id],
        |row| row.get::<_, i64>(0).map(|value| value != 0),
    )?;
    if !profile_exists {
        return Err(AppError::NotFound);
    }
    let existing_profile: Option<String> = tx
        .query_row(
            "SELECT profile_id FROM workspace_sequences WHERE id=?1",
            [&sequence.id],
            |row| row.get(0),
        )
        .optional()?;
    if existing_profile
        .as_deref()
        .is_some_and(|owner| owner != sequence.profile_id)
    {
        return Err(AppError::NotFound);
    }
    if existing_profile.is_none() {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM workspace_sequences WHERE profile_id=?1",
            [&sequence.profile_id],
            |row| row.get(0),
        )?;
        if count >= MAX_WORKSPACE_SEQUENCES_PER_PROFILE {
            return Err(AppError::Invalid(
                "profile has reached its workspace sequence limit".into(),
            ));
        }
        let entity_collision: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1)", [&sequence.id], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
        if entity_collision {
            return Err(AppError::Invalid(
                "workspace sequence ID is already used".into(),
            ));
        }
    }
    let name_collision: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM workspace_sequences WHERE profile_id=?1 AND name=?2 COLLATE NOCASE AND id<>?3)", params![sequence.profile_id,sequence.name,sequence.id], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
    if name_collision {
        return Err(AppError::Invalid(
            "workspace sequence name already exists in this profile".into(),
        ));
    }
    for step in &sequence.steps {
        let member: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
            params![sequence.profile_id, step.resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !member {
            return Err(AppError::Invalid(
                "every sequence resource must belong to its profile".into(),
            ));
        }
        if let Some(target_id) = step.target_id.as_deref() {
            let active_owned: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)", params![target_id,step.resource_id], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
            if !active_owned {
                return Err(AppError::Invalid(
                    "sequence target must be active and belong to its resource".into(),
                ));
            }
        }
        let id_collision: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 UNION ALL SELECT 1 FROM resources WHERE id=?1 UNION ALL SELECT 1 FROM resource_targets WHERE id=?1 UNION ALL SELECT 1 FROM smart_collections WHERE id=?1 UNION ALL SELECT 1 FROM dynamic_shortcuts WHERE id=?1 UNION ALL SELECT 1 FROM workspace_sequences WHERE id=?1 AND id<>?2 UNION ALL SELECT 1 FROM workspace_sequence_steps WHERE id=?1 AND NOT (sequence_id=?3 AND profile_id=?4))", params![step.id,sequence.id,sequence.id,sequence.profile_id], |row| row.get::<_, i64>(0).map(|value| value != 0))?;
        if id_collision {
            return Err(AppError::Invalid(
                "workspace sequence step ID is already used".into(),
            ));
        }
    }
    let timestamp = now_utc();
    tx.execute("INSERT INTO workspace_sequences(id,profile_id,name,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,updated_utc=excluded.updated_utc", params![sequence.id,sequence.profile_id,sequence.name,timestamp])?;
    tx.execute(
        "DELETE FROM workspace_sequence_steps WHERE sequence_id=?1 AND profile_id=?2",
        params![sequence.id, sequence.profile_id],
    )?;
    for (position, step) in sequence.steps.iter().enumerate() {
        tx.execute("INSERT INTO workspace_sequence_steps(sequence_id,profile_id,id,position,resource_id,target_id) VALUES (?1,?2,?3,?4,?5,?6)", params![sequence.id,sequence.profile_id,step.id,position as i64,step.resource_id,step.target_id])?;
    }
    tx.commit()?;
    Ok(sequence)
}

#[tauri::command]
fn workspace_sequence_save(
    state: State<'_, AppState>,
    sequence: WorkspaceSequence,
) -> Result<WorkspaceSequence, AppError> {
    let mut conn = lock(&state)?;
    workspace_sequence_save_on_conn(&mut conn, sequence)
}

fn workspace_sequence_delete_on_conn(
    conn: &Connection,
    profile_id: &str,
    id: &str,
) -> Result<(), AppError> {
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(id, "workspace sequence id")?;
    let changed = conn.execute(
        "DELETE FROM workspace_sequences WHERE id=?1 AND profile_id=?2",
        params![id, profile_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

#[tauri::command]
fn workspace_sequence_delete(
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
) -> Result<(), AppError> {
    let conn = lock(&state)?;
    workspace_sequence_delete_on_conn(&conn, &profile_id, &id)
}

fn sequence_step_request_id(run_request_id: &str, step_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"smart-workspace-launcher/sequence-step/v1\0");
    hasher.update(run_request_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(step_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn compact_terminal_workspace_sequence_runs(
    conn: &Connection,
    profile_id: &str,
    older_than_utc: Option<&str>,
) -> Result<(), AppError> {
    let rows = {
        let mut stmt = conn.prepare("SELECT request_id,result_json,updated_utc FROM workspace_sequence_runs WHERE profile_id=?1")?;
        stmt.query_map([profile_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    for (request_id, result_json, updated_utc) in rows {
        let mut result: WorkspaceSequenceRunResult = serde_json::from_str(&result_json)?;
        if result.status == "in_progress" {
            continue;
        }
        let expired = older_than_utc.is_none_or(|cutoff| {
            updated_utc.as_str() < cutoff
                || result
                    .steps
                    .iter()
                    .any(|step| step.result.launched_at_utc.as_str() < cutoff)
        });
        if !expired {
            continue;
        }
        result.steps.clear();
        result.stopped_at_step_id = None;
        conn.execute(
            "UPDATE workspace_sequence_runs SET sequence_json='',result_json=?2 WHERE request_id=?1",
            params![request_id, serde_json::to_string(&result)?],
        )?;
    }
    Ok(())
}

fn persist_workspace_sequence_result(
    conn: &Connection,
    request_id: &str,
    profile_id: &str,
    sequence: &WorkspaceSequence,
    result: &WorkspaceSequenceRunResult,
) -> Result<(), AppError> {
    let terminal = result.status != "in_progress";
    let settings = usage_settings_for(conn, profile_id)?;
    let history_includes_all_steps = settings.recording_enabled
        && sequence.steps.iter().all(|step| {
            !settings
                .excluded_resource_ids
                .iter()
                .any(|excluded| excluded == &step.resource_id)
        });
    let mut stored_result = result.clone();
    let sequence_json = if terminal && !history_includes_all_steps {
        stored_result.steps.clear();
        stored_result.stopped_at_step_id = None;
        String::new()
    } else {
        serde_json::to_string(sequence)?
    };
    conn.execute(
        "UPDATE workspace_sequence_runs SET sequence_json=?2,result_json=?3,updated_utc=?4 WHERE request_id=?1",
        params![request_id, sequence_json, serde_json::to_string(&stored_result)?, now_utc()],
    )?;
    Ok(())
}

fn record_failed_sequence_launch(
    conn: &mut Connection,
    request: &LaunchRequest,
    error_code: &str,
    message: &str,
) -> Result<LaunchEvent, AppError> {
    valid_uuid(&request.request_id, "launch request id")?;
    valid_uuid(&request.resource_id, "resource id")?;
    let profile_id = request
        .profile_id
        .as_deref()
        .ok_or_else(|| AppError::Invalid("sequence launch requires a profile".into()))?;
    valid_uuid(profile_id, "profile id")?;
    if request.source != "sequence" {
        return Err(AppError::Invalid("invalid sequence launch source".into()));
    }
    let intent_hash = launch_intent_hash(request)?;
    if let Some(mut event) = conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,CASE WHEN status='handed_off' THEN 'Windows accepted the Shell handoff; target load is not verified.' WHEN error_code='launch_outcome_unknown' THEN 'The prior launch outcome is unknown and was not retried.' ELSE 'The target could not be handed to Windows Shell.' END,duration_ms FROM launch_request_guard WHERE request_id=?1", [&request.request_id], launch_event_from_row).optional()? {
        let stored_hash: String = conn.query_row("SELECT intent_hash FROM launch_request_guard WHERE request_id=?1", [&request.request_id], |r|r.get(0))?;
        if (!stored_hash.starts_with("legacy:") && stored_hash != intent_hash) || event.resource_id != request.resource_id || event.target_id != request.target_id || event.profile_id.as_deref() != Some(profile_id) || event.source != request.source {
            return Err(AppError::Invalid("launch request ID was already used for a different action".into()));
        }
        if event.status == "requested" {
            conn.execute("UPDATE launch_request_guard SET status='failed',error_code='launch_outcome_unknown' WHERE request_id=?1 AND status='requested'", [&request.request_id])?;
            conn.execute("UPDATE launch_events SET status='failed',error_code='launch_outcome_unknown',message='The prior launch handoff was interrupted; it was not retried to avoid opening the resource twice.' WHERE request_id=?1 AND status='requested'", [&request.request_id])?;
            event = usage_event_from_guard(conn, &request.request_id)?;
        }
        return Ok(event);
    }
    let launched_at_utc = now_utc();
    conn.execute("INSERT INTO launch_request_guard(request_id,intent_hash,resource_id,target_id,profile_id,source,launched_at_utc,status,error_code,duration_ms) VALUES (?1,?2,?3,?4,?5,'sequence',?6,'failed',?7,0)", params![request.request_id,intent_hash,request.resource_id,request.target_id,profile_id,launched_at_utc,error_code])?;
    let mut event = usage_event_from_guard(conn, &request.request_id)?;
    event.message = Some(message.to_string());
    record_guard_event(conn, request, &event)?;
    Ok(event)
}

fn workspace_sequence_run_with<F>(
    conn: &mut Connection,
    sequence_id: &str,
    profile_id: &str,
    request_id: &str,
    mut launch: F,
) -> Result<WorkspaceSequenceRunResult, AppError>
where
    F: FnMut(&ResourceTarget) -> Result<(), String>,
{
    valid_uuid(sequence_id, "workspace sequence id")?;
    valid_uuid(profile_id, "profile id")?;
    valid_uuid(request_id, "sequence request id")?;
    let _settings = usage_settings_for(conn, profile_id)?;
    let existing_run = conn.query_row("SELECT sequence_id,profile_id,sequence_json,result_json FROM workspace_sequence_runs WHERE request_id=?1", [request_id], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?))).optional()?;
    let (sequence, mut result, resumed) =
        if let Some((stored_sequence_id, stored_profile_id, stored_snapshot, stored_result)) =
            existing_run
        {
            if stored_sequence_id != sequence_id || stored_profile_id != profile_id {
                return Err(AppError::Invalid(
                    "sequence request ID was already used for a different action".into(),
                ));
            }
            let stored_result: WorkspaceSequenceRunResult = serde_json::from_str(&stored_result)?;
            if stored_result.status != "in_progress" {
                return Ok(stored_result);
            }
            let stored_sequence: WorkspaceSequence = serde_json::from_str(&stored_snapshot)?;
            (stored_sequence, stored_result, true)
        } else {
            let sequence = workspace_sequence_list_on_conn(conn, profile_id)?
                .into_iter()
                .find(|sequence| sequence.id == sequence_id)
                .ok_or(AppError::NotFound)?;
            let result = WorkspaceSequenceRunResult {
                sequence_id: sequence.id.clone(),
                status: "in_progress".into(),
                total_steps: sequence.steps.len(),
                completed_steps: 0,
                stopped_at_step_id: None,
                steps: Vec::new(),
            };
            (sequence, result, false)
        };
    if sequence.steps.is_empty() || sequence.steps.len() > MAX_WORKSPACE_SEQUENCE_STEPS {
        return Err(AppError::Invalid(
            "workspace sequence has no valid steps".into(),
        ));
    }
    if !resumed {
        let snapshot = serde_json::to_string(&sequence)?;
        let result_json = serde_json::to_string(&result)?;
        let timestamp = now_utc();
        conn.execute("INSERT INTO workspace_sequence_runs(request_id,sequence_id,profile_id,sequence_json,result_json,created_utc,updated_utc) VALUES (?1,?2,?3,?4,?5,?6,?6)", params![request_id,sequence_id,profile_id,snapshot,result_json,timestamp])?;
    }
    let first_pending = result.steps.len();
    for step in sequence.steps.iter().skip(first_pending).cloned() {
        let step_request_id = sequence_step_request_id(request_id, &step.id);
        let request = LaunchRequest {
            request_id: step_request_id,
            resource_id: step.resource_id.clone(),
            target_id: step.target_id.clone(),
            profile_id: Some(profile_id.to_string()),
            source: "sequence".into(),
        };
        let target_is_still_valid = if let Some(target_id) = step.target_id.as_deref() {
            conn.query_row("SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)", params![target_id,step.resource_id], |row| row.get::<_,i64>(0).map(|value| value != 0))?
        } else {
            true
        };
        let profile_still_contains_resource: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
            params![profile_id, step.resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        let (step_result, event_status) = if !target_is_still_valid
            || !profile_still_contains_resource
        {
            let code = if target_is_still_valid {
                "resource_not_in_profile"
            } else {
                "target_not_found"
            };
            let event = record_failed_sequence_launch(conn, &request, code, "The sequence step is no longer available. Review the resource and target before running again.")?;
            (
                LaunchResult {
                    request_id: event.request_id.clone(),
                    status: event.status.clone(),
                    launched_at_utc: event.launched_at_utc.clone(),
                    error_code: event.error_code.clone(),
                    message: event.message.clone(),
                },
                event.status,
            )
        } else {
            let mut event = launch_event_with(conn, &request, |target| launch(target))?;
            if event.status == "requested" {
                conn.execute("UPDATE launch_events SET status='failed',error_code='launch_outcome_unknown',message='The prior launch handoff was interrupted; it was not retried to avoid opening the resource twice.' WHERE request_id=?1 AND status='requested'", [&event.request_id])?;
                event = conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE request_id=?1", [&event.request_id], launch_event_from_row)?;
            }
            (
                LaunchResult {
                    request_id: event.request_id.clone(),
                    status: event.status.clone(),
                    launched_at_utc: event.launched_at_utc.clone(),
                    error_code: event.error_code.clone(),
                    message: event.message.clone(),
                },
                event.status,
            )
        };
        result.steps.push(WorkspaceSequenceStepResult {
            step_id: step.id.clone(),
            resource_id: step.resource_id.clone(),
            target_id: step.target_id.clone(),
            result: step_result,
        });
        if event_status == "handed_off" {
            result.completed_steps += 1;
        } else {
            result.status = "failed".into();
            result.stopped_at_step_id = Some(step.id);
        }
        persist_workspace_sequence_result(conn, request_id, profile_id, &sequence, &result)?;
        if result.status == "failed" {
            return Ok(result);
        }
    }
    result.status = "completed".into();
    persist_workspace_sequence_result(conn, request_id, profile_id, &sequence, &result)?;
    Ok(result)
}

#[tauri::command]
fn workspace_sequence_run(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    id: String,
    request_id: String,
) -> Result<WorkspaceSequenceRunResult, AppError> {
    let mut conn = lock(&state)?;
    let result = workspace_sequence_run_with(&mut conn, &id, &profile_id, &request_id, |target| {
        launch_target(&app, target).map_err(|error| error.to_string())
    })?;
    for step in &result.steps {
        if let Some(event) = conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE request_id=?1", [&step.result.request_id], launch_event_from_row).optional()? {
            let _ = app.emit("launch-result", &event);
        }
    }
    Ok(result)
}

#[tauri::command]
fn profile_export(
    state: State<'_, AppState>,
    profile_id: String,
    output_path: String,
    include_history: Option<bool>,
) -> Result<ProfileExportResult, AppError> {
    valid_uuid(&profile_id, "profile id")?;
    let include_history = include_history.unwrap_or(false);
    let conn = lock(&state)?;
    let package = profile_package_from_db(&conn, &profile_id, include_history)?;
    let profile_name = package.profile.name.clone();
    let resource_count = package.resources.resources.len();
    let history_event_count = package
        .history
        .as_ref()
        .map_or(0, |history| history.events.len());
    export_profile_to_path(&conn, &profile_id, Path::new(&output_path), include_history)?;
    Ok(ProfileExportResult {
        path: output_path,
        profile_name,
        resource_count,
        history_event_count,
    })
}

#[tauri::command]
fn profile_import_preview(
    state: State<'_, AppState>,
    package_path: String,
    target_profile_id: Option<String>,
) -> Result<ProfileImportPreview, AppError> {
    let (package, package_sha256) = read_profile_package_with_digest(Path::new(&package_path))?;
    let conn = lock(&state)?;
    profile_import_preview_from_conn(
        &conn,
        &package,
        &package_sha256,
        target_profile_id.as_deref(),
    )
}

#[tauri::command]
fn profile_import_commit(
    state: State<'_, AppState>,
    package_path: String,
    strategy: String,
    target_profile_id: Option<String>,
    package_sha256: String,
    preview_token: String,
) -> Result<ProfileImportResult, AppError> {
    let mut conn = lock(&state)?;
    import_profile_package_from_path(
        &mut conn,
        Path::new(&package_path),
        &strategy,
        target_profile_id.as_deref(),
        &package_sha256,
        &preview_token,
    )
}

#[tauri::command]
fn profile_switch(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey_status: State<'_, HotkeyStatusState>,
    active_profile: State<'_, ActiveProfileState>,
    profile_id: String,
) -> Result<Profile, AppError> {
    Uuid::parse_str(&profile_id)
        .map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    let conn = lock(&state)?;
    let profile = conn.query_row(
        "SELECT id,name,theme_id,settings_json,created_utc,updated_utc FROM profiles WHERE id=?1",
        [&profile_id], profile_from_row,
    ).optional()?.ok_or(AppError::NotFound)?;
    let status = apply_profile_shortcut(&app, &conn, &profile_id, &hotkey_status)?;
    // A registration conflict leaves the old shortcut active, while selecting the requested profile still succeeds.
    record_active_profile(&active_profile, &profile_id);
    let _ = app.emit("quick-launch-hotkey-status", status);
    Ok(profile)
}

#[tauri::command]
fn quick_launch_hotkey_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey_status: State<'_, HotkeyStatusState>,
    active_profile: State<'_, ActiveProfileState>,
    profile_id: String,
    shortcut: String,
) -> Result<QuickLaunchHotkeyStatus, AppError> {
    Uuid::parse_str(&profile_id)
        .map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    let requested = normalize_shortcut(&shortcut)?;
    let mut conn = lock(&state)?;
    let active_id = active_profile
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let mut status_state = hotkey_status
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if profile_id != active_id {
        let status = save_inactive_profile_shortcut(
            &conn,
            &profile_id,
            &requested,
            status_state.active_shortcut.clone(),
        )?;
        drop(status_state);
        let _ = app.emit("quick-launch-hotkey-status", &status);
        return Ok(status);
    }

    let previous = status_state.active_shortcut.clone();
    let status = set_profile_shortcut_with(
        &mut conn,
        &profile_id,
        &requested,
        previous.as_deref(),
        |value| {
            app.global_shortcut()
                .register(value)
                .map_err(|error| error.to_string())
        },
        |value| {
            app.global_shortcut()
                .unregister(value)
                .map_err(|error| error.to_string())
        },
    )?;
    *status_state = status.clone();
    drop(status_state);
    let _ = app.emit("quick-launch-hotkey-status", &status);
    Ok(status)
}

#[tauri::command]
fn resource_launch(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    active_profile: State<'_, ActiveProfileState>,
    mut request: LaunchRequest,
) -> Result<LaunchResult, AppError> {
    if request.profile_id.is_none() {
        request.profile_id = Some(
            active_profile
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        );
    }
    let mut conn = lock(&state)?;
    let event = launch_event_with(&mut conn, &request, |target| {
        launch_target(&app, target).map_err(|e| e.to_string())
    })?;
    drop(conn);
    let result = LaunchResult {
        request_id: event.request_id.clone(),
        status: event.status.clone(),
        launched_at_utc: event.launched_at_utc.clone(),
        error_code: event.error_code.clone(),
        message: event.message.clone(),
    };
    let _ = app.emit("launch-result", &event);
    Ok(result)
}

fn launch_event_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LaunchEvent> {
    Ok(LaunchEvent {
        request_id: r.get(0)?,
        resource_id: r.get(1)?,
        target_id: r.get(2)?,
        profile_id: r.get(3)?,
        source: r.get(4)?,
        status: r.get(5)?,
        launched_at_utc: r.get(6)?,
        error_code: r.get(7)?,
        message: r.get(8)?,
        duration_ms: r.get(9)?,
    })
}

fn launch_intent_hash(request: &LaunchRequest) -> Result<String, AppError> {
    let intent = serde_json::to_vec(&(
        request.profile_id.as_deref(),
        request.resource_id.as_str(),
        request.target_id.as_deref(),
        request.source.as_str(),
    ))?;
    Ok(format!("{:x}", Sha256::digest(intent)))
}

fn usage_settings_for(conn: &Connection, profile_id: &str) -> Result<UsageSettings, AppError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [profile_id],
        |r| r.get::<_, i64>(0).map(|n| n != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let stored = conn.query_row("SELECT recording_enabled,retention_days,excluded_resource_ids_json FROM usage_settings WHERE profile_id=?1", [profile_id], |r| Ok((r.get::<_, i64>(0)?,r.get::<_, Option<u16>>(1)?,r.get::<_, String>(2)?))).optional()?;
    let (recording, retention, excluded) = stored.unwrap_or((1, None, "[]".into()));
    let cutoff = retention.map(|days| {
        (Utc::now() - chrono::Duration::days(days as i64))
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    });
    if recording == 0 {
        compact_terminal_workspace_sequence_runs(conn, profile_id, None)?;
    } else if let Some(cutoff_utc) = cutoff.as_deref() {
        compact_terminal_workspace_sequence_runs(conn, profile_id, Some(cutoff_utc))?;
    }
    if let Some(cutoff_utc) = cutoff.as_deref() {
        conn.execute(
            "DELETE FROM launch_events WHERE profile_id=?1 AND launched_at_utc<?2",
            params![profile_id, cutoff_utc],
        )?;
    }
    Ok(UsageSettings {
        profile_id: profile_id.to_string(),
        recording_enabled: recording != 0,
        retention_days: retention,
        excluded_resource_ids: serde_json::from_str(&excluded)?,
    })
}

fn should_record_usage(
    conn: &Connection,
    profile_id: Option<&str>,
    resource_id: &str,
) -> Result<bool, AppError> {
    let Some(profile_id) = profile_id else {
        return Ok(false);
    };
    let settings = usage_settings_for(conn, profile_id)?;
    Ok(settings.recording_enabled
        && !settings
            .excluded_resource_ids
            .iter()
            .any(|id| id == resource_id))
}

fn usage_event_from_guard(conn: &Connection, request_id: &str) -> Result<LaunchEvent, AppError> {
    Ok(conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,CASE WHEN status='handed_off' THEN 'Windows accepted the Shell handoff; target load is not verified.' WHEN error_code='launch_outcome_unknown' THEN 'The prior launch outcome is unknown and was not retried.' ELSE 'The target could not be handed to Windows Shell.' END,duration_ms FROM launch_request_guard WHERE request_id=?1", [request_id], launch_event_from_row)?)
}

fn record_guard_event(
    conn: &Connection,
    request: &LaunchRequest,
    event: &LaunchEvent,
) -> Result<(), AppError> {
    if should_record_usage(conn, request.profile_id.as_deref(), &request.resource_id)? {
        conn.execute("INSERT OR IGNORE INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status,error_code,message,duration_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![Uuid::new_v4().to_string(),event.request_id,event.resource_id,event.target_id,event.profile_id,event.source,event.launched_at_utc,event.status,event.error_code,event.message,event.duration_ms])?;
    }
    Ok(())
}

fn launch_event_with<F>(
    conn: &mut Connection,
    request: &LaunchRequest,
    launch: F,
) -> Result<LaunchEvent, AppError>
where
    F: FnOnce(&ResourceTarget) -> Result<(), String>,
{
    Uuid::parse_str(&request.request_id)
        .map_err(|_| AppError::Invalid("request id must be a UUID".into()))?;
    Uuid::parse_str(&request.resource_id)
        .map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    if let Some(pid) = request.profile_id.as_ref() {
        Uuid::parse_str(pid).map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    }
    if let Some(target_id) = request.target_id.as_ref() {
        Uuid::parse_str(target_id)
            .map_err(|_| AppError::Invalid("target id must be a UUID".into()))?;
        let active_target_belongs_to_resource: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)",
            params![target_id, request.resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !active_target_belongs_to_resource {
            return Err(AppError::NotFound);
        }
    }
    if ![
        "dashboard",
        "quick_launch",
        "dock",
        "explorer",
        "sequence",
        "other",
    ]
    .contains(&request.source.as_str())
    {
        return Err(AppError::Invalid("unsupported launch source".into()));
    }
    let intent_hash = launch_intent_hash(request)?;
    if let Some(existing) = conn.query_row("SELECT request_id,intent_hash,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,duration_ms FROM launch_request_guard WHERE request_id=?1", [&request.request_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<i64>>(9)?))).optional()? {
        let is_legacy = existing.1.starts_with("legacy:");
        let legacy_target_matches = if !is_legacy { true } else if let Some(target_id) = request.target_id.as_deref() { existing.3.as_deref() == Some(target_id) } else {
            let current_default: Option<String> = conn.query_row("SELECT id FROM resource_targets WHERE resource_id=?1 AND active=1 ORDER BY priority DESC LIMIT 1", [&request.resource_id], |r|r.get(0)).optional()?;
            existing.3 == current_default
        };
        if (!is_legacy && existing.1 != intent_hash) || existing.2 != request.resource_id || existing.4 != request.profile_id || existing.5 != request.source || !legacy_target_matches {
            return Err(AppError::Invalid("launch request ID was already used for a different action".into()));
        }
        if existing.6 == "requested" {
            conn.execute("UPDATE launch_request_guard SET status='failed',error_code='launch_outcome_unknown' WHERE request_id=?1", [&request.request_id])?;
            conn.execute("UPDATE launch_events SET status='failed',error_code='launch_outcome_unknown',message='The prior launch handoff was interrupted; it was not retried to avoid opening the resource twice.' WHERE request_id=?1 AND status='requested'", [&request.request_id])?;
        }
        return usage_event_from_guard(conn, &request.request_id);
    }
    if let Some(profile_id) = request.profile_id.as_deref() {
        let allowed: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM profiles p JOIN profile_resources pr ON pr.profile_id=p.id WHERE p.id=?1 AND pr.resource_id=?2)",
            params![profile_id, request.resource_id],
            |row| row.get::<_, i64>(0).map(|value| value != 0),
        )?;
        if !allowed {
            return Err(AppError::NotFound);
        }
    }
    let target:Option<ResourceTarget>=conn.query_row("SELECT id,kind,canonical_uri,priority,arguments_json,working_directory,active FROM resource_targets WHERE resource_id=?1 AND active=1 AND (?2 IS NULL OR id=?2) ORDER BY priority DESC LIMIT 1",params![request.resource_id,request.target_id],|r|Ok(ResourceTarget{id:r.get(0)?,kind:r.get(1)?,canonical_uri:r.get(2)?,priority:r.get(3)?,arguments:serde_json::from_str(&r.get::<_,String>(4)?).unwrap_or_default(),working_directory:r.get(5)?,active:r.get::<_,i64>(6)?!=0})).optional()?;
    let target_id = target.as_ref().map(|t| t.id.clone());
    let launched_at_utc = now_utc();
    // Caller holds the DB mutex across reservation and handoff. The privacy-minimal guard survives history purge.
    conn.execute("INSERT INTO launch_request_guard(request_id,intent_hash,resource_id,target_id,profile_id,source,status,launched_at_utc) VALUES (?1,?2,?3,?4,?5,?6,'requested',?7)",params![request.request_id,intent_hash,request.resource_id,target_id,request.profile_id,request.source,launched_at_utc])?;
    let started = std::time::Instant::now();
    let outcome = match target.as_ref() {
        Some(t) => validate_target(t)
            .map_err(|e| e.to_string())
            .and_then(|_| launch(t)),
        None => Err("target_not_found".into()),
    };
    let duration_ms = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    let (status, error_code, message) = match outcome {
        Ok(()) => (
            "handed_off".to_string(),
            None,
            Some("Windows accepted the Shell handoff; target load is not verified.".to_string()),
        ),
        Err(e) => (
            "failed".to_string(),
            Some(e),
            Some("The target could not be handed to Windows Shell.".to_string()),
        ),
    };
    conn.execute("UPDATE launch_request_guard SET status=?2,error_code=?3,duration_ms=?4 WHERE request_id=?1", params![request.request_id,status,error_code,duration_ms])?;
    let event = usage_event_from_guard(conn, &request.request_id)?;
    record_guard_event(conn, request, &event)?;
    Ok(event)
}

#[tauri::command]
fn quick_launch_hotkey_status(state: State<'_, HotkeyStatusState>) -> QuickLaunchHotkeyStatus {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[tauri::command]
fn analytics_query(
    state: State<'_, AppState>,
    profile_id: String,
    from_utc: Option<String>,
    to_utc: Option<String>,
    source: Option<String>,
    status: Option<String>,
    resource_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<LaunchEvent>, AppError> {
    valid_uuid(&profile_id, "profile id")?;
    let filter = validate_usage_filter(UsageFilter {
        from_utc,
        to_utc,
        source,
        status,
        resource_id,
    })?;
    let conn = lock(&state)?;
    let _settings = usage_settings_for(&conn, &profile_id)?;
    let limit = limit.unwrap_or(100).clamp(1, 5000);
    let offset = offset.unwrap_or(0).min(1_000_000);
    let mut stmt = conn.prepare("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE profile_id=?1 AND (?2 IS NULL OR launched_at_utc>=?2) AND (?3 IS NULL OR launched_at_utc<=?3) AND (?4 IS NULL OR source=?4) AND (?5 IS NULL OR status=?5) AND (?6 IS NULL OR resource_id=?6) ORDER BY launched_at_utc DESC,request_id DESC LIMIT ?7 OFFSET ?8")?;
    let events = stmt
        .query_map(
            params![
                profile_id,
                filter.from_utc,
                filter.to_utc,
                filter.source,
                filter.status,
                filter.resource_id,
                limit as i64,
                offset as i64
            ],
            launch_event_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

fn normalize_usage_timestamp(value: Option<String>) -> Result<Option<String>, AppError> {
    value
        .map(|v| {
            if v.len() > 64 {
                return Err(AppError::Invalid("usage timestamp is too long".into()));
            }
            chrono::DateTime::parse_from_rfc3339(&v)
                .map(|dt| {
                    dt.with_timezone(&Utc)
                        .to_rfc3339_opts(SecondsFormat::Millis, true)
                })
                .map_err(|_| AppError::Invalid("usage timestamps must be RFC 3339".into()))
        })
        .transpose()
}

fn validate_usage_filter(mut filter: UsageFilter) -> Result<UsageFilter, AppError> {
    filter.from_utc = normalize_usage_timestamp(filter.from_utc)?;
    filter.to_utc = normalize_usage_timestamp(filter.to_utc)?;
    if filter
        .from_utc
        .as_ref()
        .zip(filter.to_utc.as_ref())
        .is_some_and(|(from, to)| from > to)
    {
        return Err(AppError::Invalid(
            "fromUtc must not be later than toUtc".into(),
        ));
    }
    if filter.source.as_deref().is_some_and(|s| {
        ![
            "dashboard",
            "quick_launch",
            "dock",
            "explorer",
            "sequence",
            "other",
        ]
        .contains(&s)
    }) {
        return Err(AppError::Invalid("unsupported usage source".into()));
    }
    if filter
        .status
        .as_deref()
        .is_some_and(|s| !["requested", "handed_off", "failed"].contains(&s))
    {
        return Err(AppError::Invalid("unsupported usage status".into()));
    }
    if let Some(id) = filter.resource_id.as_deref() {
        valid_uuid(id, "resource id")?;
    }
    Ok(filter)
}

#[tauri::command]
fn usage_settings_get(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<UsageSettings, AppError> {
    valid_uuid(&profile_id, "profile id")?;
    usage_settings_for(&lock(&state)?, &profile_id)
}

#[tauri::command]
fn usage_settings_set(
    state: State<'_, AppState>,
    settings: UsageSettings,
) -> Result<UsageSettings, AppError> {
    valid_uuid(&settings.profile_id, "profile id")?;
    if settings
        .retention_days
        .is_some_and(|d| ![30, 90, 365].contains(&d))
    {
        return Err(AppError::Invalid(
            "retentionDays must be 30, 90, 365 or null".into(),
        ));
    }
    if settings.excluded_resource_ids.len() > 10_000 {
        return Err(AppError::Invalid("too many excluded resources".into()));
    }
    let mut ids = settings.excluded_resource_ids.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != settings.excluded_resource_ids.len() {
        return Err(AppError::Invalid(
            "excluded resource IDs must be unique".into(),
        ));
    }
    for id in &ids {
        valid_uuid(id, "excluded resource id")?;
    }
    let mut conn = lock(&state)?;
    let tx = conn.transaction()?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
        [&settings.profile_id],
        |r| r.get::<_, i64>(0).map(|v| v != 0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let valid_members: i64 = tx.query_row("SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1 AND resource_id IN (SELECT value FROM json_each(?2))",params![settings.profile_id,serde_json::to_string(&ids)?],|r|r.get(0))?;
    if valid_members as usize != ids.len() {
        return Err(AppError::Invalid(
            "excluded resources must belong to the selected profile".into(),
        ));
    }
    tx.execute("INSERT INTO usage_settings(profile_id,recording_enabled,retention_days,excluded_resource_ids_json) VALUES (?1,?2,?3,?4) ON CONFLICT(profile_id) DO UPDATE SET recording_enabled=excluded.recording_enabled,retention_days=excluded.retention_days,excluded_resource_ids_json=excluded.excluded_resource_ids_json",params![settings.profile_id,settings.recording_enabled as i64,settings.retention_days,serde_json::to_string(&ids)?])?;
    let cutoff = settings.retention_days.map(|days| {
        (Utc::now() - chrono::Duration::days(days as i64))
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    });
    if !settings.recording_enabled {
        compact_terminal_workspace_sequence_runs(&tx, &settings.profile_id, None)?;
    } else if let Some(cutoff_utc) = cutoff.as_deref() {
        compact_terminal_workspace_sequence_runs(&tx, &settings.profile_id, Some(cutoff_utc))?;
    }
    if let Some(cutoff_utc) = cutoff.as_deref() {
        tx.execute(
            "DELETE FROM launch_events WHERE profile_id=?1 AND launched_at_utc<?2",
            params![settings.profile_id, cutoff_utc],
        )?;
    }
    tx.commit()?;
    usage_settings_for(&conn, &settings.profile_id)
}

#[tauri::command]
fn usage_history_purge(state: State<'_, AppState>, profile_id: String) -> Result<u64, AppError> {
    valid_uuid(&profile_id, "profile id")?;
    let conn = lock(&state)?;
    purge_usage_history_for_profile(&conn, &profile_id)
}

fn purge_usage_history_for_profile(conn: &Connection, profile_id: &str) -> Result<u64, AppError> {
    let _settings = usage_settings_for(conn, profile_id)?;
    compact_terminal_workspace_sequence_runs(conn, profile_id, None)?;
    let removed = conn.execute(
        "DELETE FROM launch_events WHERE profile_id=?1",
        [profile_id],
    )?;
    Ok(removed as u64)
}

fn csv_safe(value: &str) -> String {
    let trimmed = value.trim_start_matches(|c: char| c.is_whitespace() || c.is_control());
    let value = if trimmed
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@'))
    {
        format!("'{}", value)
    } else {
        value.to_string()
    };
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[tauri::command]
fn usage_export_csv(
    state: State<'_, AppState>,
    query: UsageQuery,
    output_path: String,
) -> Result<UsageExportResult, AppError> {
    let conn = lock(&state)?;
    usage_export_csv_to_path(&conn, query, &output_path)
}

fn usage_export_csv_to_path(
    conn: &Connection,
    query: UsageQuery,
    output_path: &str,
) -> Result<UsageExportResult, AppError> {
    if output_path.len() > 4096 {
        return Err(AppError::Invalid("output path is too long".into()));
    }
    let path = Path::new(&output_path);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
    {
        return Err(AppError::Invalid(
            "usage export path must end in .csv".into(),
        ));
    }
    valid_uuid(&query.profile_id, "profile id")?;
    let _settings = usage_settings_for(conn, &query.profile_id)?;
    let filter = validate_usage_filter(UsageFilter {
        from_utc: query.from_utc,
        to_utc: query.to_utc,
        source: query.source,
        status: query.status,
        resource_id: query.resource_id,
    })?;
    let name = path
        .file_name()
        .ok_or_else(|| AppError::Invalid("output path must name a file".into()))?
        .to_string_lossy();
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| AppError::Invalid(format!("cannot create CSV export: {e}")))?;
    let written = (|| -> Result<usize, AppError> {
        writeln!(
            file,
            "request_id,resource_id,launched_at_utc,source,status,duration_ms"
        )
        .map_err(|e| AppError::Invalid(format!("cannot write CSV: {e}")))?;
        let mut stmt = conn.prepare("SELECT request_id,resource_id,launched_at_utc,source,status,duration_ms FROM launch_events WHERE profile_id=?1 AND (?2 IS NULL OR launched_at_utc>=?2) AND (?3 IS NULL OR launched_at_utc<=?3) AND (?4 IS NULL OR source=?4) AND (?5 IS NULL OR status=?5) AND (?6 IS NULL OR resource_id=?6) ORDER BY launched_at_utc DESC,request_id DESC")?;
        let rows = stmt.query_map(
            params![
                query.profile_id,
                filter.from_utc,
                filter.to_utc,
                filter.source,
                filter.status,
                filter.resource_id
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            },
        )?;
        let mut count = 0;
        for row in rows {
            let (request_id, resource_id, utc, source, status, duration) = row?;
            writeln!(
                file,
                "{},{},{},{},{},{}",
                csv_safe(&request_id),
                csv_safe(&resource_id),
                csv_safe(&utc),
                csv_safe(&source),
                csv_safe(&status),
                duration.map(|d| d.to_string()).unwrap_or_default()
            )
            .map_err(|e| AppError::Invalid(format!("cannot write CSV: {e}")))?;
            count += 1;
        }
        Ok(count)
    })();
    let event_count = match written {
        Ok(count) => count,
        Err(error) => {
            drop(file);
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = std::fs::remove_file(&temporary);
        return Err(AppError::Invalid(format!("cannot sync CSV: {error}")));
    }
    drop(file);
    if let Err(error) = finalize_export_no_clobber(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(UsageExportResult {
        path: output_path.to_string(),
        event_count,
    })
}

/// Metadata checks, including errors, are reused for 15 minutes unless forceRefresh is set.
const METADATA_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
// A filesystem metadata syscall cannot be forcibly interrupted; cancellation is checked between targets.
const METADATA_INSPECTION_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_METADATA_TARGETS: usize = 250;
const MAX_METADATA_CACHE_ROWS: usize = 10_000;
const MAX_CONCURRENT_METADATA_INSPECTIONS: usize = 2;
static ACTIVE_METADATA_INSPECTIONS: AtomicUsize = AtomicUsize::new(0);

struct MetadataInspectionPermit;

impl MetadataInspectionPermit {
    fn acquire() -> Result<Self, AppError> {
        loop {
            let active = ACTIVE_METADATA_INSPECTIONS.load(Ordering::Acquire);
            if active >= MAX_CONCURRENT_METADATA_INSPECTIONS {
                return Err(AppError::Invalid("metadata inspection is busy".into()));
            }
            if ACTIVE_METADATA_INSPECTIONS
                .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(Self);
            }
        }
    }
}

impl Drop for MetadataInspectionPermit {
    fn drop(&mut self) {
        ACTIVE_METADATA_INSPECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[tauri::command]
async fn resource_inspect(
    state: State<'_, AppState>,
    resource_id: String,
    force_refresh: Option<bool>,
) -> Result<Vec<ResourceMetadata>, AppError> {
    Uuid::parse_str(&resource_id)
        .map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    let conn = Arc::clone(&state.0);
    let force_refresh = force_refresh.unwrap_or(false);
    let permit = MetadataInspectionPermit::acquire()?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let inspection = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        inspect_resource_metadata_on_db_with_cancel(
            &conn,
            &resource_id,
            force_refresh,
            Some(worker_cancelled),
        )
    });
    match tokio::time::timeout(METADATA_INSPECTION_TIMEOUT, inspection).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AppError::Invalid("metadata inspection task failed".into())),
        Err(_) => {
            cancelled.store(true, Ordering::Release);
            Err(AppError::Invalid("metadata inspection timed out".into()))
        }
    }
}

#[cfg(test)]
fn inspect_resource_metadata(
    conn: &Connection,
    resource_id: &str,
    force_refresh: bool,
) -> Result<Vec<ResourceMetadata>, AppError> {
    let snapshot = load_metadata_snapshot(conn, resource_id)?;
    let records = inspect_metadata_snapshot(
        resource_id,
        snapshot,
        force_refresh,
        None,
        inspect_target_filesystem,
        |record| store_metadata_record(conn, record),
    )?;
    prune_metadata_cache(conn)?;
    Ok(records)
}

fn inspect_resource_metadata_on_db_with_cancel(
    db: &Arc<Mutex<Connection>>,
    resource_id: &str,
    force_refresh: bool,
    cancelled: Option<Arc<AtomicBool>>,
) -> Result<Vec<ResourceMetadata>, AppError> {
    inspect_resource_metadata_on_db_with(
        db,
        resource_id,
        force_refresh,
        cancelled,
        inspect_target_filesystem,
    )
}

fn inspect_resource_metadata_on_db_with<F>(
    db: &Arc<Mutex<Connection>>,
    resource_id: &str,
    force_refresh: bool,
    cancelled: Option<Arc<AtomicBool>>,
    inspector: F,
) -> Result<Vec<ResourceMetadata>, AppError>
where
    F: FnMut(&str, &ResourceTarget) -> ResourceMetadata,
{
    let snapshot = {
        let conn = db
            .lock()
            .map_err(|_| AppError::Invalid("database lock poisoned".into()))?;
        load_metadata_snapshot(&conn, resource_id)?
    };
    let records = inspect_metadata_snapshot(
        resource_id,
        snapshot,
        force_refresh,
        cancelled.as_deref(),
        inspector,
        |record| {
            let conn = db
                .lock()
                .map_err(|_| AppError::Invalid("database lock poisoned".into()))?;
            store_metadata_record(&conn, record)
        },
    )?;
    let conn = db
        .lock()
        .map_err(|_| AppError::Invalid("database lock poisoned".into()))?;
    prune_metadata_cache(&conn)?;
    Ok(records)
}

fn load_metadata_snapshot(
    conn: &Connection,
    resource_id: &str,
) -> Result<Vec<(ResourceTarget, Option<ResourceMetadata>)>, AppError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1)",
        [resource_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let mut stmt = conn.prepare("SELECT id,kind,canonical_uri,priority,arguments_json,working_directory,active FROM resource_targets WHERE resource_id=?1 AND active=1 ORDER BY priority DESC,id LIMIT ?2")?;
    let targets = stmt
        .query_map(
            params![resource_id, (MAX_METADATA_TARGETS + 1) as i64],
            |row| {
                Ok(ResourceTarget {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    canonical_uri: row.get(2)?,
                    priority: row.get(3)?,
                    arguments: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    working_directory: row.get(5)?,
                    active: row.get::<_, i64>(6)? != 0,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if targets.len() > MAX_METADATA_TARGETS {
        return Err(AppError::Invalid(format!(
            "resource has more than {MAX_METADATA_TARGETS} active targets"
        )));
    }
    targets
        .into_iter()
        .map(|target| {
            let cached = load_cached_metadata(conn, resource_id, &target.id)?;
            Ok((target, cached))
        })
        .collect()
}

fn inspect_metadata_snapshot<F, S>(
    resource_id: &str,
    snapshot: Vec<(ResourceTarget, Option<ResourceMetadata>)>,
    force_refresh: bool,
    cancelled: Option<&AtomicBool>,
    mut inspector: F,
    mut store: S,
) -> Result<Vec<ResourceMetadata>, AppError>
where
    F: FnMut(&str, &ResourceTarget) -> ResourceMetadata,
    S: FnMut(&ResourceMetadata) -> Result<(), AppError>,
{
    let mut records = Vec::with_capacity(snapshot.len());
    for (target, cached) in snapshot {
        if cancelled.is_some_and(|value| value.load(Ordering::Acquire)) {
            return Err(AppError::Invalid("metadata inspection cancelled".into()));
        }
        if !force_refresh {
            if let Some(mut cached_record) = cached.clone() {
                if cache_is_fresh(&cached_record) {
                    if cached_record.cache_status != "error" {
                        cached_record.cache_status = "cached".into();
                    }
                    records.push(cached_record);
                    continue;
                }
            }
        }
        let mut inspected = inspector(resource_id, &target);
        if cancelled.is_some_and(|value| value.load(Ordering::Acquire)) {
            return Err(AppError::Invalid("metadata inspection cancelled".into()));
        }
        if inspected.cache_status == "error" {
            if let Some(previous) = cached {
                inspected.file_size_bytes = previous.file_size_bytes;
                inspected.created_utc = previous.created_utc;
                inspected.modified_utc = previous.modified_utc;
                inspected.provenance = previous.provenance;
            }
        }
        store(&inspected)?;
        records.push(inspected);
    }
    Ok(records)
}

fn load_cached_metadata(
    conn: &Connection,
    resource_id: &str,
    target_id: &str,
) -> Result<Option<ResourceMetadata>, AppError> {
    Ok(conn
        .query_row(
            "SELECT inspected_utc,file_size_bytes,created_utc,modified_utc,availability,provenance,cache_status,error_code FROM metadata_cache WHERE resource_id=?1 AND target_id=?2",
            params![resource_id, target_id],
            |row| {
                Ok(ResourceMetadata {
                    resource_id: resource_id.to_string(),
                    target_id: target_id.to_string(),
                    inspected_utc: row.get(0)?,
                    file_size_bytes: row.get::<_, Option<i64>>(1)?.and_then(|size| u64::try_from(size).ok()),
                    created_utc: row.get(2)?,
                    modified_utc: row.get(3)?,
                    availability: row.get(4)?,
                    provenance: row.get(5)?,
                    cache_status: row.get(6)?,
                    error_code: row.get(7)?,
                })
            },
        )
        .optional()?)
}

fn cache_is_fresh(cached: &ResourceMetadata) -> bool {
    chrono::DateTime::parse_from_rfc3339(&cached.inspected_utc)
        .map(|inspected| {
            Utc::now().signed_duration_since(inspected)
                < chrono::Duration::from_std(METADATA_CACHE_TTL).unwrap_or_default()
        })
        .unwrap_or(false)
}

fn store_metadata_record(conn: &Connection, record: &ResourceMetadata) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO metadata_cache(resource_id,target_id,inspected_utc,file_size_bytes,created_utc,modified_utc,availability,provenance,cache_status,error_code) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(resource_id,target_id) DO UPDATE SET inspected_utc=excluded.inspected_utc,file_size_bytes=excluded.file_size_bytes,created_utc=excluded.created_utc,modified_utc=excluded.modified_utc,availability=excluded.availability,provenance=excluded.provenance,cache_status=excluded.cache_status,error_code=excluded.error_code",
        params![record.resource_id,record.target_id,record.inspected_utc,record.file_size_bytes.and_then(|value|i64::try_from(value).ok()),record.created_utc,record.modified_utc,record.availability,record.provenance,record.cache_status,record.error_code],
    )?;
    Ok(())
}

fn prune_metadata_cache(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM metadata_cache WHERE rowid IN (SELECT rowid FROM metadata_cache ORDER BY inspected_utc DESC LIMIT -1 OFFSET ?1)",
        [MAX_METADATA_CACHE_ROWS as i64],
    )?;
    Ok(())
}

fn inspect_target_filesystem(resource_id: &str, target: &ResourceTarget) -> ResourceMetadata {
    let inspected_utc = now_utc();
    let mut result = ResourceMetadata {
        resource_id: resource_id.into(),
        target_id: target.id.clone(),
        inspected_utc,
        file_size_bytes: None,
        created_utc: None,
        modified_utc: None,
        availability: "unsupported".into(),
        provenance: "unsupported".into(),
        cache_status: "fresh".into(),
        error_code: None,
    };
    if is_network_path(&target.canonical_uri) {
        result.error_code = Some("network_path_skipped".into());
        return result;
    }
    if !matches!(target.kind.as_str(), "file" | "folder" | "app" | "shortcut") {
        result.error_code = Some("not_local_file".into());
        return result;
    }
    result.provenance = "local-filesystem".into();
    match std::fs::metadata(&target.canonical_uri) {
        Ok(metadata) => {
            result.availability = "available".into();
            if metadata.is_file() {
                result.file_size_bytes = Some(metadata.len());
            }
            result.created_utc = metadata.created().ok().map(system_time_to_utc);
            result.modified_utc = metadata.modified().ok().map(system_time_to_utc);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            result.availability = "missing".into();
            result.error_code = Some("not_found".into());
        }
        Err(error) => {
            result.availability = "inaccessible".into();
            result.cache_status = "error".into();
            result.error_code = Some(if error.kind() == std::io::ErrorKind::PermissionDenied {
                "permission_denied".into()
            } else {
                "metadata_error".into()
            });
        }
    }
    result
}

#[cfg(not(windows))]
fn is_network_path(path: &str) -> bool {
    path.replace('/', "\\").starts_with("\\\\")
}

#[cfg(windows)]
fn is_network_path(path: &str) -> bool {
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;

    if path.replace('/', "\\").starts_with("\\\\") {
        return true;
    }
    let bytes = path.as_bytes();
    if bytes.len() < 3 || bytes[1] != b':' || bytes[2] != b'\\' && bytes[2] != b'/' {
        return false;
    }
    let root = format!("{}:\\", bytes[0] as char);
    let root: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    const DRIVE_REMOTE: u32 = 4;
    unsafe { GetDriveTypeW(windows::core::PCWSTR(root.as_ptr())) == DRIVE_REMOTE }
}

fn system_time_to_utc(value: std::time::SystemTime) -> String {
    chrono::DateTime::<Utc>::from(value).to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[tauri::command]
fn analytics_summary(
    state: State<'_, AppState>,
    profile_id: String,
    from_utc: Option<String>,
    to_utc: Option<String>,
    source: Option<String>,
    status: Option<String>,
    resource_id: Option<String>,
) -> Result<AnalyticsSummary, AppError> {
    valid_uuid(&profile_id, "profile id")?;
    let filter = validate_usage_filter(UsageFilter {
        from_utc,
        to_utc,
        source,
        status,
        resource_id,
    })?;
    let conn = lock(&state)?;
    let _settings = usage_settings_for(&conn, &profile_id)?;
    analytics_summary_for(&conn, &profile_id, &filter)
}

fn analytics_summary_for(
    conn: &Connection,
    profile_id: &str,
    filter: &UsageFilter,
) -> Result<AnalyticsSummary, AppError> {
    let mut stmt = conn.prepare("SELECT source,status,COUNT(*),MIN(launched_at_utc),MAX(launched_at_utc) FROM launch_events WHERE profile_id=?1 AND (?2 IS NULL OR launched_at_utc>=?2) AND (?3 IS NULL OR launched_at_utc<=?3) AND (?4 IS NULL OR source=?4) AND (?5 IS NULL OR status=?5) AND (?6 IS NULL OR resource_id=?6) GROUP BY source,status ORDER BY source,status")?;
    let mut summary = AnalyticsSummary {
        total_launches: 0,
        by_source: BTreeMap::new(),
        by_status: BTreeMap::new(),
        first_event_utc: None,
        last_event_utc: None,
    };
    for row in stmt.query_map(
        params![
            profile_id,
            filter.from_utc,
            filter.to_utc,
            filter.source,
            filter.status,
            filter.resource_id
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        },
    )? {
        let (source, status, count, first, last) = row?;
        summary.total_launches += count;
        *summary.by_source.entry(source).or_default() += count;
        *summary.by_status.entry(status).or_default() += count;
        if let Some(first) = first {
            if summary
                .first_event_utc
                .as_ref()
                .is_none_or(|current| first < *current)
            {
                summary.first_event_utc = Some(first);
            }
        }
        if let Some(last) = last {
            if summary
                .last_event_utc
                .as_ref()
                .is_none_or(|current| last > *current)
            {
                summary.last_event_utc = Some(last);
            }
        }
    }
    Ok(summary)
}

#[cfg(windows)]
fn launch_target(_app: &tauri::AppHandle, target: &ResourceTarget) -> Result<(), AppError> {
    use windows::{
        core::PCWSTR, Win32::UI::Shell::ShellExecuteW,
        Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
    };
    validate_target(target)?;
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let path: Vec<u16> = format!("{}\0", target.canonical_uri)
        .encode_utf16()
        .collect();
    let params = if target.arguments.is_empty() {
        None
    } else {
        Some(
            target
                .arguments
                .iter()
                .map(|arg| quote_windows_argument(arg))
                .collect::<Vec<_>>()
                .join(" "),
        )
    };
    let params_w = params
        .as_ref()
        .map(|p| format!("{}\0", p).encode_utf16().collect::<Vec<_>>());
    let work_w = target
        .working_directory
        .as_ref()
        .map(|p| format!("{}\0", p).encode_utf16().collect::<Vec<_>>());
    let code = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(path.as_ptr()),
            params_w
                .as_ref()
                .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            work_w
                .as_ref()
                .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            SW_SHOWNORMAL,
        )
    };
    if code.0 as isize <= 32 {
        return Err(AppError::Launch(format!(
            "windows_shell_error_{}",
            code.0 as isize
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn quote_windows_argument(arg: &str) -> String {
    // Windows command-line escaping: quote every argument and double backslashes
    // before a quote or the closing quote, rather than concatenating raw strings.
    let mut out = String::from("\"");
    let mut slashes = 0;
    for ch in arg.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            out.push_str(&"\\".repeat(slashes * 2 + 1));
            out.push('"');
        } else {
            out.push_str(&"\\".repeat(slashes));
            out.push(ch);
        }
        slashes = 0;
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

#[cfg(not(windows))]
fn launch_target(_app: &tauri::AppHandle, _target: &ResourceTarget) -> Result<(), AppError> {
    Err(AppError::Launch(
        "windows_shell_unavailable_on_this_platform".into(),
    ))
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if quick_launch_toggle_pressed(shortcut, &event) {
                        toggle_quick_launch(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let path = app.path().app_local_data_dir()?.join("launcher.sqlite3");
            let conn = open_database(&path).map_err(|e| std::io::Error::other(e.to_string()))?;
            let default_id =
                default_profile_id(&conn).map_err(|e| std::io::Error::other(e.to_string()))?;
            let desired = profile_shortcut(&conn, &default_id).unwrap_or_else(|error| {
                eprintln!("Could not load default profile shortcut: {error}");
                QUICK_LAUNCH_HOTKEY_SPEC.into()
            });
            app.manage(AppState(Arc::new(Mutex::new(conn))));
            let status = match normalize_shortcut(&desired) {
                Ok(requested) => switch_hotkey(
                    &requested,
                    None,
                    |value| {
                        app.global_shortcut()
                            .register(value)
                            .map_err(|error| error.to_string())
                    },
                    |_| Ok(()),
                ),
                Err(_) => QuickLaunchHotkeyStatus {
                    registered: false,
                    shortcut: desired,
                    active_shortcut: None,
                    error_code: Some("invalid_shortcut".into()),
                },
            };
            if !status.registered {
                eprintln!(
                    "Global shortcut {} failed: {}",
                    status.shortcut,
                    status.error_code.as_deref().unwrap_or("unknown")
                );
            }
            app.manage(HotkeyStatusState(Mutex::new(status.clone())));
            app.manage(ActiveProfileState(Mutex::new(default_id)));
            let _ = app.emit("quick-launch-hotkey-status", status);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            resource_list,
            resource_create,
            resource_update,
            resource_duplicate,
            resource_relocation_preview,
            resource_relocation_commit,
            resource_delete,
            resource_search,
            resource_launch,
            resource_inspect,
            profile_list,
            profile_create,
            profile_theme_set,
            smart_collection_list,
            smart_collection_save,
            smart_collection_delete,
            dynamic_shortcut_list,
            dynamic_shortcut_save,
            dynamic_shortcut_delete,
            workspace_sequence_list,
            workspace_sequence_save,
            workspace_sequence_delete,
            workspace_sequence_run,
            resource_relationship_list,
            resource_relationship_save,
            resource_relationship_delete,
            workspace_snapshot_list,
            workspace_snapshot_create,
            workspace_snapshot_delete,
            workspace_snapshot_restore,
            profile_export,
            profile_import_preview,
            profile_import_commit,
            profile_switch,
            layout_get,
            layout_save,
            quick_launch_hotkey_status,
            quick_launch_hotkey_set,
            analytics_query,
            analytics_summary,
            usage_settings_get,
            usage_settings_set,
            usage_history_purge,
            usage_export_csv
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("failed to build Smart Workspace Launcher");
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let registered = handle
                .try_state::<HotkeyStatusState>()
                .map(|state| {
                    state
                        .0
                        .lock()
                        .map(|status| status.registered)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if registered {
                if let Err(error) = handle
                    .global_shortcut()
                    .unregister(QUICK_LAUNCH_HOTKEY_SPEC)
                {
                    eprintln!("Global shortcut cleanup failed: {error}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate_database(&conn).unwrap();
        conn
    }

    #[test]
    fn production_database_enables_busy_timeout_and_untrusted_schema_protection() {
        let directory = std::env::temp_dir().join(format!("sw011-db-{}", Uuid::new_v4()));
        let path = directory.join("launcher.sqlite3");
        let conn = open_database(&path).unwrap();
        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        let trusted_schema: i64 = conn
            .query_row("PRAGMA trusted_schema", [], |row| row.get(0))
            .unwrap();
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(busy_timeout, 5000);
        assert_eq!(trusted_schema, 0);
        assert_eq!(foreign_keys, 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn profile_archive_rejects_unsafe_paths_and_noncanonical_assets() {
        for path in [
            "../profile.json",
            "a/../../profile.json",
            "/absolute",
            "C:/drive",
            "a\\b",
            "a//b",
        ] {
            assert!(!safe_archive_path(path), "accepted unsafe path: {path}");
        }
        assert!(safe_archive_path(
            "assets/icons/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png"
        ));
        assert!(!valid_asset_name(
            "assets/icons/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.png"
        ));
        assert!(!valid_asset_name(
            "assets/icons/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.exe"
        ));
    }

    #[test]
    fn profile_export_finalization_never_overwrites_existing_destination() {
        let directory = std::env::temp_dir().join(format!("swprofile-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let temporary = directory.join("temporary");
        let destination = directory.join("existing.swprofile");
        std::fs::write(&temporary, b"new archive").unwrap();
        std::fs::write(&destination, b"existing data").unwrap();
        assert!(finalize_export_no_clobber(&temporary, &destination).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing data");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn profile_archive_fixture_passes_strict_rust_validation() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/valid/example.swprofile");
        let package = read_profile_package(&path).expect("approved fixture should parse");
        assert_eq!(package.profile.name, "Demo Work");
        assert_eq!(package.resources.resources.len(), 1);
    }

    fn write_test_archive(entries: &[(&str, Vec<u8>)]) -> PathBuf {
        write_test_archive_with_compression(entries, CompressionMethod::Stored)
    }

    fn write_test_archive_with_compression(
        entries: &[(&str, Vec<u8>)],
        compression: CompressionMethod,
    ) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{}.swprofile", Uuid::new_v4()));
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        for (name, bytes) in entries {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default().compression_method(compression),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn write_duplicate_name_archive(entries: &[(&str, Vec<u8>)]) -> PathBuf {
        let path = write_test_archive(entries);
        let mut bytes = std::fs::read(&path).unwrap();
        let from = b"profile.jsox";
        let to = b"profile.json";
        assert_eq!(from.len(), to.len());
        let mut offset = 0;
        while let Some(relative) = bytes[offset..]
            .windows(from.len())
            .position(|window| window == from)
        {
            let start = offset + relative;
            bytes[start..start + to.len()].copy_from_slice(to);
            offset = start + to.len();
        }
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn serialized_documents(package: &ValidatedProfilePackage) -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (
                "manifest.json",
                serde_json::to_vec(&package.manifest).unwrap(),
            ),
            (
                "profile.json",
                serde_json::to_vec(&package.profile).unwrap(),
            ),
            (
                "resources.json",
                serde_json::to_vec(&package.resources).unwrap(),
            ),
            (
                "layouts.json",
                serde_json::to_vec(&package.layouts).unwrap(),
            ),
        ]
    }

    #[test]
    fn production_zip_reader_rejects_duplicate_and_traversal_members() {
        let package = package("url");
        let docs = serialized_documents(&package);
        let mut duplicate = docs.clone();
        duplicate.push(("profile.jsox", b"{}".to_vec()));
        let duplicate_path = write_duplicate_name_archive(&duplicate);
        assert!(read_profile_package(&duplicate_path).is_err());
        let _ = std::fs::remove_file(duplicate_path);

        let mut traversal = docs;
        traversal.push(("../payload", b"ignored".to_vec()));
        let traversal_path = write_test_archive(&traversal);
        assert!(read_profile_package(&traversal_path).is_err());
        let _ = std::fs::remove_file(traversal_path);
    }

    #[test]
    fn production_zip_reader_rejects_high_ratio_and_asset_checksum_mismatch() {
        let package = package("url");
        let mut high_ratio = serialized_documents(&package);
        high_ratio.push(("padding.bin", vec![0; 1024 * 1024]));
        let high_ratio_path =
            write_test_archive_with_compression(&high_ratio, CompressionMethod::Deflated);
        assert!(read_profile_package(&high_ratio_path).is_err());
        let _ = std::fs::remove_file(high_ratio_path);

        let mut package = package("url");
        let asset_path =
            "assets/icons/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png";
        package.manifest.assets.push(ProfilePackageAsset {
            path: asset_path.into(),
            sha256: "0".repeat(64),
        });
        let mut mismatch = serialized_documents(&package);
        mismatch.push((asset_path, b"not-the-declared-hash".to_vec()));
        let mismatch_path = write_test_archive(&mismatch);
        assert!(read_profile_package(&mismatch_path).is_err());
        let _ = std::fs::remove_file(mismatch_path);
    }

    #[test]
    fn production_zip_reader_rejects_expanded_member_over_limit_within_archive_limit() {
        let package = package("url");
        let mut entries = serialized_documents(&package);
        let mut payload = Vec::with_capacity(PROFILE_ARCHIVE_MAX_ENTRY as usize + 1);
        let mut state = 0x9e37_79b9_u32;
        for _ in 0..(PROFILE_ARCHIVE_MAX_ENTRY as usize - 1024) {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            payload.push(state as u8);
        }
        payload.extend(std::iter::repeat_n(0u8, 1025));
        entries.push(("oversized.bin", payload));
        let path = write_test_archive_with_compression(&entries, CompressionMethod::Deflated);
        assert!(std::fs::metadata(&path).unwrap().len() <= PROFILE_ARCHIVE_MAX_COMPRESSED);
        assert!(read_profile_package(&path).is_err());
        let _ = std::fs::remove_file(path);
    }

    fn profile(conn: &Connection) -> String {
        let id = Uuid::new_v4().to_string();
        let ts = now_utc();
        conn.execute("INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,'Work','system','{}',?2,?2)",params![id,ts]).unwrap();
        id
    }

    fn package(kind: &str) -> ValidatedProfilePackage {
        let profile_id = Uuid::new_v4().to_string();
        let resource_id = Uuid::new_v4().to_string();
        let target_id = Uuid::new_v4().to_string();
        let profile = ProfilePackageDocument {
            schema_version: 1,
            id: profile_id,
            name: "Portable profile".into(),
            description: None,
            theme: ProfilePackageTheme {
                mode: "dark".into(),
                accent: "#6858CE".into(),
                density: None,
            },
            settings: serde_json::json!({"apiToken": "must-not-export"}),
            memberships: vec![resource_id.clone()],
            membership_details: vec![ProfilePackageMembership {
                resource_id: resource_id.clone(),
                favorite: true,
                category: Some("Work".into()),
                sort_position: 0,
            }],
            smart_collections: Vec::new(),
            dynamic_shortcuts: Vec::new(),
            workspace_sequences: Vec::new(),
        };
        let package = ValidatedProfilePackage {
            manifest: ProfilePackageManifest {
                format: "swprofile".into(),
                schema_version: 1,
                exported_at_utc: now_utc(),
                app_version: None,
                profile_file: "profile.json".into(),
                resources_file: "resources.json".into(),
                layouts_file: "layouts.json".into(),
                history_file: None,
                include_history: false,
                assets: Vec::new(),
            },
            profile,
            resources: ProfileResourcesDocument {
                schema_version: 1,
                resources: vec![ProfilePackageResource {
                    schema_version: 1,
                    id: resource_id.clone(),
                    name: "Calculator".into(),
                    resource_type: kind.into(),
                    description: None,
                    tags: vec!["tools".into()],
                    aliases: vec![],
                    icon: None,
                    targets: vec![ProfilePackageTarget {
                        id: target_id,
                        kind: kind.into(),
                        value: "calc.exe".into(),
                        priority: 0,
                        device_label: None,
                        argv: vec![],
                        working_directory: None,
                        active: true,
                    }],
                }],
            },
            layouts: ProfileLayoutsDocument {
                schema_version: 1,
                layouts: vec![ProfilePackageLayout {
                    surface: Some("dashboard".into()),
                    breakpoint: "desktop".into(),
                    cards: vec![ProfilePackageLayoutCard {
                        resource_id,
                        x: 0,
                        y: 0,
                        w: 2,
                        h: 2,
                        card_type: "icon".into(),
                    }],
                }],
            },
            history: None,
        };
        validate_package_fields(&package).unwrap();
        package
    }

    fn install_package_resource(
        conn: &mut Connection,
        package: &ValidatedProfilePackage,
        profile_id: &str,
    ) {
        let tx = conn.transaction().unwrap();
        let source = &package.resources.resources[0];
        let value = Resource {
            id: source.id.clone(),
            resource_type: source.resource_type.clone(),
            display_name: source.name.clone(),
            description: None,
            icon: None,
            default_action: "open".into(),
            created_utc: now_utc(),
            updated_utc: now_utc(),
            targets: source
                .targets
                .iter()
                .map(|target| ResourceTarget {
                    id: target.id.clone(),
                    kind: target.kind.clone(),
                    canonical_uri: target.value.clone(),
                    priority: target.priority,
                    arguments: target.argv.clone(),
                    working_directory: target.working_directory.clone(),
                    active: target.active,
                })
                .collect(),
            tags: source.tags.clone(),
            aliases: source.aliases.clone(),
            category: None,
            favorite: false,
            launch_count: 0,
            last_launched_utc: None,
        };
        persist_resource_in_transaction(&tx, &value, true, Some(profile_id)).unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn profile_import_duplicate_remaps_ids_and_disables_app_target_without_launching() {
        let mut conn = database();
        let package = package("app");
        let result = import_profile_package(&mut conn, &package, "duplicate", None).unwrap();
        assert_ne!(result.profile_id, package.profile.id);
        let imported_resource: String = conn
            .query_row(
                "SELECT resource_id FROM profile_resources WHERE profile_id=?1",
                [&result.profile_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(imported_resource, package.resources.resources[0].id);
        let (target_id, active): (String, i64) = conn
            .query_row(
                "SELECT id,active FROM resource_targets WHERE resource_id=?1",
                [&imported_resource],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_ne!(target_id, package.resources.resources[0].targets[0].id);
        assert_eq!(active, 0, "import must not activate application targets");
        let saved_settings: String = conn
            .query_row(
                "SELECT settings_json FROM profiles WHERE id=?1",
                [&result.profile_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!saved_settings.contains("apiToken"));
    }

    #[test]
    fn profile_import_merge_reuses_resources_and_replace_overwrites_memberships() {
        let mut conn = database();
        let package = package("url");
        let target_profile = profile(&conn);
        install_package_resource(&mut conn, &package, &target_profile);
        let merged =
            import_profile_package(&mut conn, &package, "merge", Some(&target_profile)).unwrap();
        assert_eq!(merged.reused_resources, 1);
        assert_eq!(merged.imported_resources, 0);
        let replaced =
            import_profile_package(&mut conn, &package, "replace", Some(&target_profile)).unwrap();
        assert_eq!(replaced.imported_resources, 1);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1",
                [&target_profile],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn profile_import_merge_remaps_differing_resource_collision_and_previews_values() {
        let mut conn = database();
        let package = package("url");
        let target_profile = profile(&conn);
        install_package_resource(&mut conn, &package, &target_profile);
        conn.execute(
            "UPDATE resources SET display_name='Local Calculator' WHERE id=?1",
            [&package.resources.resources[0].id],
        )
        .unwrap();
        let preview =
            profile_import_preview_from_conn(&conn, &package, "a".repeat(64).as_str(), None)
                .unwrap();
        assert_eq!(preview.resource_conflicts.len(), 1);
        let conflict = &preview.resource_conflicts[0];
        assert_eq!(conflict.resolution, "import_as_new");
        assert_eq!(conflict.imported.name, "Calculator");
        assert_eq!(conflict.existing.name, "Local Calculator");
        assert_eq!(conflict.proposed.name, "Calculator");

        let result =
            import_profile_package(&mut conn, &package, "merge", Some(&target_profile)).unwrap();
        assert_eq!(result.reused_resources, 0);
        assert_eq!(result.imported_resources, 1);
        let imported_id: String = conn
            .query_row(
                "SELECT resource_id FROM profile_resources WHERE profile_id=?1 AND resource_id<>?2",
                params![target_profile, package.resources.resources[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(imported_id, package.resources.resources[0].id);
        let old_name: String = conn
            .query_row(
                "SELECT display_name FROM resources WHERE id=?1",
                [&package.resources.resources[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_name, "Local Calculator");
    }

    #[test]
    fn profile_import_preview_binds_target_and_shows_collection_shortcut_collisions() {
        let mut conn = database();
        let mut package = package("file");
        let collection = ProfilePackageSmartCollection {
            id: Uuid::new_v4().to_string(),
            name: "Work items".into(),
            filters: SmartCollectionFilters::default(),
        };
        let shortcut = ProfilePackageDynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Pinned item".into(),
            resource_id: package.resources.resources[0].id.clone(),
            target_id: Some(package.resources.resources[0].targets[0].id.clone()),
        };
        package.profile.smart_collections = vec![collection.clone()];
        package.profile.dynamic_shortcuts = vec![shortcut.clone()];
        let target_profile = profile(&conn);
        package.profile.id = target_profile.clone();
        validate_package_fields(&package).unwrap();
        install_package_resource(&mut conn, &package, &target_profile);
        smart_collection_save_on_conn(
            &mut conn,
            SmartCollection {
                id: collection.id.clone(),
                profile_id: target_profile.clone(),
                name: collection.name.clone(),
                filters: collection.filters.clone(),
            },
        )
        .unwrap();
        dynamic_shortcut_save_on_conn(
            &mut conn,
            DynamicShortcut {
                id: shortcut.id.clone(),
                profile_id: target_profile.clone(),
                name: shortcut.name.clone(),
                resource_id: shortcut.resource_id.clone(),
                target_id: shortcut.target_id.clone(),
            },
        )
        .unwrap();

        let default_preview =
            profile_import_preview_from_conn(&conn, &package, &"a".repeat(64), None).unwrap();
        assert_eq!(default_preview.target_profile_id, None);
        assert!(default_preview.smart_collection_name_collisions.is_empty());
        assert!(default_preview.dynamic_shortcut_name_collisions.is_empty());
        let targeted_preview = profile_import_preview_from_conn(
            &conn,
            &package,
            &"a".repeat(64),
            Some(&target_profile),
        )
        .unwrap();
        assert_eq!(targeted_preview.target_profile_id, Some(target_profile));
        assert_eq!(targeted_preview.smart_collection_count, 1);
        assert_eq!(targeted_preview.dynamic_shortcut_count, 1);
        assert_eq!(
            targeted_preview.smart_collection_id_collisions,
            vec![collection.id.clone()]
        );
        assert_eq!(
            targeted_preview.smart_collection_name_collisions,
            vec![collection.name]
        );
        assert_eq!(
            targeted_preview.dynamic_shortcut_id_collisions,
            vec![shortcut.id.clone()]
        );
        assert_eq!(
            targeted_preview.dynamic_shortcut_name_collisions,
            vec![shortcut.name]
        );
        assert_ne!(
            default_preview.preview_token,
            targeted_preview.preview_token
        );
    }

    #[test]
    fn profile_import_commit_rejects_changed_package_or_workspace_after_preview() {
        let mut conn = database();
        let mut package = package("url");
        let mut archive_entries = serialized_documents(&package);
        let path = write_test_archive(
            &archive_entries
                .iter()
                .map(|(name, bytes)| (*name, bytes.clone()))
                .collect::<Vec<_>>(),
        );
        let (parsed, digest) = read_profile_package_with_digest(&path).unwrap();
        let preview = profile_import_preview_from_conn(&conn, &parsed, &digest, None).unwrap();

        let profile_count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();
        assert!(matches!(
            import_profile_package_from_path(
                &mut conn,
                &path,
                "merge",
                None,
                &preview.package_sha256,
                &preview.preview_token,
            ),
            Err(AppError::Invalid(_))
        ));
        let profile_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();
        assert_eq!(profile_count_after, profile_count_before);

        let local_profile = Uuid::new_v4().to_string();
        let timestamp = now_utc();
        conn.execute(
            "INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,?2,'system','{}',?3,?3)",
            params![local_profile, package.profile.name, timestamp],
        ).unwrap();
        assert!(
            import_profile_package_from_path(
                &mut conn,
                &path,
                "duplicate",
                None,
                &preview.package_sha256,
                &preview.preview_token,
            )
            .is_err(),
            "workspace changed after preview must reject commit"
        );

        let preview_after_workspace_change =
            profile_import_preview_from_conn(&conn, &parsed, &digest, None).unwrap();
        package.profile.name = "Changed package after preview".into();
        archive_entries = serialized_documents(&package);
        let changed_path = write_test_archive(
            &archive_entries
                .iter()
                .map(|(name, bytes)| (*name, bytes.clone()))
                .collect::<Vec<_>>(),
        );
        std::fs::copy(changed_path, &path).unwrap();
        assert!(
            import_profile_package_from_path(
                &mut conn,
                &path,
                "duplicate",
                None,
                &preview_after_workspace_change.package_sha256,
                &preview_after_workspace_change.preview_token,
            )
            .is_err(),
            "package digest mismatch must reject commit"
        );
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(changed_path);
        let _ = conn.execute("DELETE FROM profiles WHERE id=?1", [&local_profile]);
    }

    #[test]
    fn profile_import_rolls_back_profile_when_resource_constraint_fails() {
        let mut conn = database();
        let package = package("url");
        conn.execute_batch("CREATE TRIGGER reject_profile_import BEFORE INSERT ON resources BEGIN SELECT RAISE(ABORT,'injected resource failure'); END;").unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();
        assert!(import_profile_package(&mut conn, &package, "duplicate", None).is_err());
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            before, after,
            "failed import must roll back profile creation"
        );
    }

    #[test]
    fn profile_export_import_roundtrip_preserves_smart_collections_and_old_packages_default_empty()
    {
        let mut conn = database();
        let source_profile = profile(&conn);
        let source_collection = SmartCollection {
            id: Uuid::new_v4().to_string(),
            profile_id: source_profile.clone(),
            name: "Quarterly files".into(),
            filters: SmartCollectionFilters {
                resource_type: Some("file".into()),
                category: Some("Finance".into()),
                tag: Some("reports".into()),
                favorite_only: Some(true),
                text: Some("quarterly".into()),
            },
        };
        smart_collection_save_on_conn(&mut conn, source_collection.clone()).unwrap();
        let path = std::env::temp_dir().join(format!("{}.swprofile", Uuid::new_v4()));
        export_profile_to_path(&conn, &source_profile, &path, false).unwrap();
        let package = read_profile_package(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert_eq!(package.profile.smart_collections.len(), 1);
        assert_eq!(
            package.profile.smart_collections[0].filters,
            source_collection.filters
        );

        let mut old_package = package.clone();
        let mut old_profile_json = serde_json::to_value(&old_package.profile).unwrap();
        old_profile_json
            .as_object_mut()
            .unwrap()
            .remove("smart_collections");
        old_package.profile = serde_json::from_value(old_profile_json).unwrap();
        assert!(old_package.profile.smart_collections.is_empty());
        validate_package_fields(&old_package).unwrap();

        let imported = import_profile_package(&mut conn, &package, "duplicate", None).unwrap();
        let imported_collections =
            smart_collection_list_on_conn(&conn, &imported.profile_id).unwrap();
        assert_eq!(imported_collections.len(), 1);
        assert_ne!(imported_collections[0].id, source_collection.id);
        assert_eq!(imported_collections[0].name, source_collection.name);
        assert_eq!(imported_collections[0].filters, source_collection.filters);
    }

    #[test]
    fn profile_import_merge_remaps_smart_collection_id_and_name_collisions() {
        let mut conn = database();
        let mut package = package("url");
        let imported_collection = ProfilePackageSmartCollection {
            id: Uuid::new_v4().to_string(),
            name: "Reports".into(),
            filters: SmartCollectionFilters {
                text: Some("quarterly".into()),
                ..SmartCollectionFilters::default()
            },
        };
        package.profile.smart_collections = vec![imported_collection.clone()];
        validate_package_fields(&package).unwrap();
        let target_profile = profile(&conn);
        smart_collection_save_on_conn(
            &mut conn,
            SmartCollection {
                id: imported_collection.id.clone(),
                profile_id: target_profile.clone(),
                name: imported_collection.name.clone(),
                filters: SmartCollectionFilters {
                    text: Some("annual".into()),
                    ..SmartCollectionFilters::default()
                },
            },
        )
        .unwrap();

        import_profile_package(&mut conn, &package, "merge", Some(&target_profile)).unwrap();
        let collections = smart_collection_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(collections.len(), 2);
        let remapped = collections
            .iter()
            .find(|item| item.name.starts_with("Reports (imported"))
            .unwrap();
        assert_ne!(remapped.id, imported_collection.id);
        assert_eq!(remapped.filters, imported_collection.filters);
    }

    #[test]
    fn profile_import_replace_replaces_smart_collections() {
        let mut conn = database();
        let mut package = package("url");
        let incoming = ProfilePackageSmartCollection {
            id: Uuid::new_v4().to_string(),
            name: "Incoming".into(),
            filters: SmartCollectionFilters {
                favorite_only: Some(true),
                ..SmartCollectionFilters::default()
            },
        };
        package.profile.smart_collections = vec![incoming.clone()];
        let target_profile = profile(&conn);
        smart_collection_save_on_conn(
            &mut conn,
            SmartCollection {
                id: Uuid::new_v4().to_string(),
                profile_id: target_profile.clone(),
                name: "Old collection".into(),
                filters: SmartCollectionFilters::default(),
            },
        )
        .unwrap();

        import_profile_package(&mut conn, &package, "replace", Some(&target_profile)).unwrap();
        let collections = smart_collection_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, incoming.id);
        assert_eq!(collections[0].name, incoming.name);
        assert_eq!(collections[0].filters, incoming.filters);
    }

    #[test]
    fn profile_import_rolls_back_smart_collections_and_resources_on_collection_failure() {
        let mut conn = database();
        let mut package = package("url");
        package.profile.smart_collections = vec![ProfilePackageSmartCollection {
            id: Uuid::new_v4().to_string(),
            name: "Incoming".into(),
            filters: SmartCollectionFilters::default(),
        }];
        let target_profile = profile(&conn);
        let original_collection = SmartCollection {
            id: Uuid::new_v4().to_string(),
            profile_id: target_profile.clone(),
            name: "Original".into(),
            filters: SmartCollectionFilters::default(),
        };
        smart_collection_save_on_conn(&mut conn, original_collection.clone()).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_smart_collection_import BEFORE INSERT ON smart_collections BEGIN SELECT RAISE(ABORT,'injected smart collection failure'); END;").unwrap();

        assert!(
            import_profile_package(&mut conn, &package, "replace", Some(&target_profile)).is_err()
        );
        let profile_name: String = conn
            .query_row(
                "SELECT name FROM profiles WHERE id=?1",
                [&target_profile],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(profile_name, "Work");
        assert_eq!(
            smart_collection_list_on_conn(&conn, &target_profile).unwrap(),
            vec![original_collection]
        );
        let resource_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(resource_count, 0);
        let membership_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1",
                [&target_profile],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(membership_count, 0);
    }

    #[test]
    fn profile_export_roundtrip_omits_history_by_default_and_secrets() {
        let mut conn = database();
        let source_profile = profile(&conn);
        let mut resource = resource("Website");
        resource.resource_type = "url".into();
        let tx = conn.transaction().unwrap();
        persist_resource_in_transaction(&tx, &resource, true, Some(&source_profile)).unwrap();
        tx.commit().unwrap();
        conn.execute(
            "UPDATE profiles SET settings_json=?2 WHERE id=?1",
            params![source_profile, r#"{"apiToken":"secret","accent":"#6858CE"}"#],
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!("{}.swprofile", Uuid::new_v4()));
        export_profile_to_path(&conn, &source_profile, &path, false).unwrap();
        let package = read_profile_package(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert!(package.history.is_none());
        assert!(!package.profile.settings.to_string().contains("apiToken"));
        let imported = import_profile_package(&mut conn, &package, "duplicate", None).unwrap();
        assert_eq!(imported.imported_resources, 1);
    }

    #[test]
    fn workspace_sequence_archive_roundtrip_is_inert_and_replace_remaps_references() {
        let mut conn = database();
        let source = package("url");
        let source_result = import_profile_package(&mut conn, &source, "duplicate", None).unwrap();
        let resource_id: String = conn
            .query_row(
                "SELECT resource_id FROM profile_resources WHERE profile_id=?1",
                [&source_result.profile_id],
                |row| row.get(0),
            )
            .unwrap();
        let target_id: String = conn
            .query_row(
                "SELECT id FROM resource_targets WHERE resource_id=?1",
                [&resource_id],
                |row| row.get(0),
            )
            .unwrap();
        let source_resource = resource("source placeholder");
        let source_resource = Resource {
            id: resource_id.clone(),
            targets: vec![ResourceTarget {
                id: target_id.clone(),
                kind: "url".into(),
                canonical_uri: "https://example.com".into(),
                priority: 0,
                arguments: Vec::new(),
                working_directory: None,
                active: true,
            }],
            ..source_resource
        };
        let sequence = workspace_sequence(
            &source_result.profile_id,
            "Start day",
            &[(&source_resource, Some(&target_id))],
        );
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let source_path = std::env::temp_dir().join(format!("{}.swprofile", Uuid::new_v4()));
        export_profile_to_path(&conn, &source_result.profile_id, &source_path, false).unwrap();
        let exported = read_profile_package(&source_path).unwrap();
        let _ = std::fs::remove_file(source_path);
        assert_eq!(exported.profile.workspace_sequences.len(), 1);
        assert_eq!(
            exported.profile.workspace_sequences[0].steps[0].resource_id,
            resource_id
        );
        let duplicate_import =
            import_profile_package(&mut conn, &exported, "duplicate", None).unwrap();
        assert_eq!(duplicate_import.imported_workspace_sequences, 1);
        let duplicated =
            workspace_sequence_list_on_conn(&conn, &duplicate_import.profile_id).unwrap();
        assert_eq!(duplicated.len(), 1);
        assert_ne!(duplicated[0].id, sequence.id);
        assert_ne!(duplicated[0].steps[0].id, sequence.steps[0].id);
        assert_ne!(duplicated[0].steps[0].resource_id, resource_id);
        assert_ne!(
            duplicated[0].steps[0].target_id.as_deref(),
            Some(target_id.as_str())
        );

        let target_profile = profile(&conn);
        let old_resource = resource("Old resource");
        persist_resource(&mut conn, &old_resource, true, Some(&target_profile)).unwrap();
        let old_sequence =
            workspace_sequence(&target_profile, "Old sequence", &[(&old_resource, None)]);
        workspace_sequence_save_on_conn(&mut conn, old_sequence.clone()).unwrap();
        let preview = profile_import_preview_from_conn(
            &conn,
            &exported,
            &"a".repeat(64),
            Some(&target_profile),
        )
        .unwrap();
        assert_eq!(preview.workspace_sequence_count, 1);
        assert_eq!(
            preview.workspace_sequence_name_collisions,
            Vec::<String>::new()
        );
        let imported =
            import_profile_package(&mut conn, &exported, "replace", Some(&target_profile)).unwrap();
        assert_eq!(imported.imported_workspace_sequences, 1);
        let sequences = workspace_sequence_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(sequences.len(), 1);
        assert_ne!(sequences[0].id, old_sequence.id);
        assert_ne!(sequences[0].steps[0].resource_id, resource_id);
        assert_ne!(
            sequences[0].steps[0].target_id.as_deref(),
            Some(target_id.as_str())
        );
        let remapped_target = sequences[0].steps[0].target_id.as_deref().unwrap();
        assert!(conn.query_row("SELECT EXISTS(SELECT 1 FROM resource_targets WHERE id=?1 AND resource_id=?2 AND active=1)",params![remapped_target,sequences[0].steps[0].resource_id],|row| row.get::<_,i64>(0)).unwrap() != 0);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM workspace_sequence_runs", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0,
            "profile archive import must not execute sequences"
        );
    }

    #[test]
    fn workspace_sequence_import_insert_failure_rolls_back_replacement() {
        let mut conn = database();
        let package = package("url");
        let source_profile =
            import_profile_package(&mut conn, &package, "duplicate", None).unwrap();
        let source_resource_id: String = conn
            .query_row(
                "SELECT resource_id FROM profile_resources WHERE profile_id=?1",
                [&source_profile.profile_id],
                |row| row.get(0),
            )
            .unwrap();
        let source_target_id: String = conn
            .query_row(
                "SELECT id FROM resource_targets WHERE resource_id=?1",
                [&source_resource_id],
                |row| row.get(0),
            )
            .unwrap();
        let placeholder = resource("source");
        let source_resource = Resource {
            id: source_resource_id.clone(),
            targets: vec![ResourceTarget {
                id: source_target_id.clone(),
                kind: "url".into(),
                canonical_uri: "https://example.com".into(),
                priority: 0,
                arguments: Vec::new(),
                working_directory: None,
                active: true,
            }],
            ..placeholder
        };
        let incoming_sequence = workspace_sequence(
            &source_profile.profile_id,
            "Incoming",
            &[(&source_resource, Some(&source_target_id))],
        );
        workspace_sequence_save_on_conn(&mut conn, incoming_sequence).unwrap();
        let path = std::env::temp_dir().join(format!("{}.swprofile", Uuid::new_v4()));
        export_profile_to_path(&conn, &source_profile.profile_id, &path, false).unwrap();
        let archive = read_profile_package(&path).unwrap();
        let _ = std::fs::remove_file(path);
        let target_profile = profile(&conn);
        let prior = resource("Prior");
        persist_resource(&mut conn, &prior, true, Some(&target_profile)).unwrap();
        let prior_sequence =
            workspace_sequence(&target_profile, "Prior sequence", &[(&prior, None)]);
        workspace_sequence_save_on_conn(&mut conn, prior_sequence.clone()).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_sequence_step_insert BEFORE INSERT ON workspace_sequence_steps BEGIN SELECT RAISE(ABORT,'injected sequence step failure'); END;").unwrap();
        assert!(
            import_profile_package(&mut conn, &archive, "replace", Some(&target_profile)).is_err()
        );
        let sequences = workspace_sequence_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(sequences, vec![prior_sequence]);
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1",
                [&target_profile],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let name: String = conn
            .query_row(
                "SELECT name FROM profiles WHERE id=?1",
                [&target_profile],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Work");
    }

    fn resource(name: &str) -> Resource {
        Resource {
            id: Uuid::new_v4().to_string(),
            resource_type: "url".into(),
            display_name: name.into(),
            description: None,
            icon: None,
            default_action: "open".into(),
            created_utc: now_utc(),
            updated_utc: now_utc(),
            targets: vec![ResourceTarget {
                id: Uuid::new_v4().to_string(),
                kind: "url".into(),
                canonical_uri: "https://example.com".into(),
                priority: 0,
                arguments: vec![],
                working_directory: None,
                active: true,
            }],
            tags: vec!["finance".into()],
            aliases: vec!["ledger".into()],
            category: Some("Planning".into()),
            favorite: false,
            launch_count: 0,
            last_launched_utc: None,
        }
    }

    fn workspace_sequence(
        profile_id: &str,
        name: &str,
        resources: &[(&Resource, Option<&str>)],
    ) -> WorkspaceSequence {
        WorkspaceSequence {
            id: Uuid::new_v4().to_string(),
            profile_id: profile_id.to_string(),
            name: name.to_string(),
            steps: resources
                .iter()
                .map(|(resource, target_id)| WorkspaceSequenceStep {
                    id: Uuid::new_v4().to_string(),
                    resource_id: resource.id.clone(),
                    target_id: target_id.map(str::to_string),
                })
                .collect(),
        }
    }

    #[test]
    fn workspace_sequence_validation_bounds_step_count_and_ids() {
        let sequence_id = Uuid::new_v4().to_string();
        let resource_id = Uuid::new_v4().to_string();
        let one_step = || {
            vec![WorkspaceSequenceStep {
                id: Uuid::new_v4().to_string(),
                resource_id: resource_id.clone(),
                target_id: None,
            }]
        };
        assert!(validate_workspace_sequence_fields(&sequence_id, "Open work", &[]).is_err());
        assert!(validate_workspace_sequence_fields(&sequence_id, "Open work", &one_step()).is_ok());
        let twenty = (0..20)
            .map(|_| WorkspaceSequenceStep {
                id: Uuid::new_v4().to_string(),
                resource_id: resource_id.clone(),
                target_id: None,
            })
            .collect::<Vec<_>>();
        assert!(validate_workspace_sequence_fields(&sequence_id, "Open work", &twenty).is_ok());
        let twenty_one = (0..21)
            .map(|_| WorkspaceSequenceStep {
                id: Uuid::new_v4().to_string(),
                resource_id: resource_id.clone(),
                target_id: None,
            })
            .collect::<Vec<_>>();
        assert!(
            validate_workspace_sequence_fields(&sequence_id, "Open work", &twenty_one).is_err()
        );
        let duplicated_id = Uuid::new_v4().to_string();
        let duplicate_steps = vec![
            WorkspaceSequenceStep {
                id: duplicated_id.clone(),
                resource_id: resource_id.clone(),
                target_id: None,
            },
            WorkspaceSequenceStep {
                id: duplicated_id,
                resource_id,
                target_id: None,
            },
        ];
        assert!(
            validate_workspace_sequence_fields(&sequence_id, "Open work", &duplicate_steps)
                .is_err()
        );
    }

    #[test]
    fn workspace_sequences_are_profile_scoped_and_validate_membership_and_active_targets() {
        let mut conn = database();
        let owner = profile(&conn);
        let other = profile(&conn);
        let first = resource("First");
        let second = resource("Second");
        persist_resource(&mut conn, &first, true, Some(&owner)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&other)).unwrap();
        let mut sequence =
            workspace_sequence(&owner, "Morning", &[(&first, Some(&first.targets[0].id))]);
        assert!(workspace_sequence_save_on_conn(&mut conn, sequence.clone()).is_ok());
        assert_eq!(
            workspace_sequence_list_on_conn(&conn, &owner).unwrap(),
            vec![sequence.clone()]
        );
        assert!(workspace_sequence_list_on_conn(&conn, &other)
            .unwrap()
            .is_empty());
        sequence.id = Uuid::new_v4().to_string();
        sequence.steps[0].resource_id = second.id.clone();
        assert!(workspace_sequence_save_on_conn(&mut conn, sequence.clone()).is_err());
        sequence.steps[0].resource_id = first.id.clone();
        sequence.steps[0].target_id = Some(second.targets[0].id.clone());
        assert!(workspace_sequence_save_on_conn(&mut conn, sequence.clone()).is_err());
        conn.execute(
            "UPDATE resource_targets SET active=0 WHERE id=?1",
            [&first.targets[0].id],
        )
        .unwrap();
        sequence.steps[0].target_id = Some(first.targets[0].id.clone());
        assert!(workspace_sequence_save_on_conn(&mut conn, sequence).is_err());
    }

    #[test]
    fn workspace_sequence_run_is_ordered_and_replay_does_not_launch_twice() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let mut first = resource("First");
        let mut second = resource("Second");
        first.targets[0].canonical_uri = "https://first.example.com".into();
        second.targets[0].canonical_uri = "https://second.example.com".into();
        persist_resource(&mut conn, &first, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(
            &profile_id,
            "Start work",
            &[(&first, None), (&second, None)],
        );
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let request_id = Uuid::new_v4().to_string();
        let mut calls = Vec::new();
        let result = workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &request_id,
            |target| {
                calls.push(target.canonical_uri.clone());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            calls,
            vec![
                first.targets[0].canonical_uri.clone(),
                second.targets[0].canonical_uri.clone()
            ]
        );
        assert_eq!(result.status, "completed");
        assert_eq!(result.completed_steps, 2);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].step_id, sequence.steps[0].id);
        assert_eq!(result.steps[1].step_id, sequence.steps[1].id);
        let replay =
            workspace_sequence_run_with(&mut conn, &sequence.id, &profile_id, &request_id, |_| {
                panic!("replay must not call Shell")
            })
            .unwrap();
        assert_eq!(replay.status, "completed");
        assert_eq!(calls.len(), 2);
        let other_sequence = workspace_sequence(&profile_id, "Other", &[(&first, None)]);
        workspace_sequence_save_on_conn(&mut conn, other_sequence.clone()).unwrap();
        assert!(workspace_sequence_run_with(
            &mut conn,
            &other_sequence.id,
            &profile_id,
            &request_id,
            |_| Ok(())
        )
        .is_err());
    }

    #[test]
    fn workspace_sequence_run_stops_at_first_failed_handoff() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let first = resource("First");
        let second = resource("Failure");
        let third = resource("Never opened");
        persist_resource(&mut conn, &first, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &third, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(
            &profile_id,
            "Stop on error",
            &[(&first, None), (&second, None), (&third, None)],
        );
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let mut calls = Vec::new();
        let result = workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &Uuid::new_v4().to_string(),
            |target| {
                calls.push(target.canonical_uri.clone());
                if target.canonical_uri == second.targets[0].canonical_uri {
                    Err("synthetic_failure".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(result.status, "failed");
        assert_eq!(result.completed_steps, 1);
        assert_eq!(
            result.stopped_at_step_id.as_deref(),
            Some(sequence.steps[1].id.as_str())
        );
        assert_eq!(result.steps.len(), 2);
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_events WHERE profile_id=?1 AND source='sequence'",
                [&profile_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 2);
    }

    #[test]
    fn workspace_sequence_run_records_stale_target_and_does_not_continue() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let first = resource("First");
        let second = resource("Deactivated");
        persist_resource(&mut conn, &first, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(
            &profile_id,
            "Stale target",
            &[(&first, None), (&second, Some(&second.targets[0].id))],
        );
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        conn.execute(
            "UPDATE resource_targets SET active=0 WHERE id=?1",
            [&second.targets[0].id],
        )
        .unwrap();
        let mut calls = 0;
        let result = workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &Uuid::new_v4().to_string(),
            |_| {
                calls += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(result.status, "failed");
        assert_eq!(result.completed_steps, 1);
        assert_eq!(
            result.stopped_at_step_id.as_deref(),
            Some(sequence.steps[1].id.as_str())
        );
        assert_eq!(
            result.steps[1].result.error_code.as_deref(),
            Some("target_not_found")
        );
    }

    #[test]
    fn workspace_sequence_run_recovery_does_not_repeat_unknown_shell_handoff() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let resource = resource("Interrupted");
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(&profile_id, "Interrupted", &[(&resource, None)]);
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let request_id = Uuid::new_v4().to_string();
        let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = workspace_sequence_run_with(
                &mut conn,
                &sequence.id,
                &profile_id,
                &request_id,
                |_| panic!("simulate crash after launch reservation"),
            );
        }));
        assert!(panic_result.is_err());
        let recovered =
            workspace_sequence_run_with(&mut conn, &sequence.id, &profile_id, &request_id, |_| {
                panic!("uncertain handoff must never be repeated")
            })
            .unwrap();
        assert_eq!(recovered.status, "failed");
        assert_eq!(
            recovered.steps[0].result.error_code.as_deref(),
            Some("launch_outcome_unknown")
        );
        let event_status: String = conn
            .query_row(
                "SELECT status FROM launch_events WHERE request_id=?1",
                [&recovered.steps[0].result.request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_status, "failed");
    }

    #[test]
    fn rejects_non_https_urls() {
        assert!(validate_target(&ResourceTarget {
            kind: "url".into(),
            canonical_uri: "http://example.com".into(),
            ..sample_target()
        })
        .is_err());
        assert!(validate_target(&ResourceTarget {
            kind: "url".into(),
            canonical_uri: "javascript:alert(1)".into(),
            ..sample_target()
        })
        .is_err());
    }
    #[test]
    fn accepts_https_urls_and_schema_target_kinds() {
        assert!(validate_target(&ResourceTarget {
            kind: "url".into(),
            canonical_uri: "https://example.com/path".into(),
            ..sample_target()
        })
        .is_ok());
    }

    #[test]
    fn target_validation_rejects_credentials_controls_ads_and_device_paths() {
        assert!(validate_target(&ResourceTarget {
            kind: "url".into(),
            canonical_uri: "https://user:password@example.com/path".into(),
            ..sample_target()
        })
        .is_err());
        for path in [
            "C:\\safe\\..\\secret.txt",
            "C:\\file.txt:secret-stream",
            "\\\\.\\PhysicalDrive0",
            "\\\\?\\C:\\secret.txt",
            "C:\\NUL.txt",
            "relative\\file.txt",
        ] {
            assert!(
                validate_target(&ResourceTarget {
                    kind: "file".into(),
                    canonical_uri: path.into(),
                    ..sample_target()
                })
                .is_err(),
                "accepted unsafe Windows target path: {path}"
            );
        }
        assert!(validate_target(&ResourceTarget {
            working_directory: Some("C:\\folder\0\\child".into()),
            ..sample_target()
        })
        .is_err());
    }

    #[test]
    fn profile_theme_update_persists_validated_theme_and_preserves_other_settings() {
        let mut conn = database();
        let profile_id = profile(&conn);
        conn.execute(
            "UPDATE profiles SET settings_json=?2 WHERE id=?1",
            params![
                profile_id,
                r#"{"customSetting":{"keep":true},"density":"comfortable","theme":"legacy"}"#
            ],
        )
        .unwrap();
        let updated = profile_theme_set_on_conn(
            &mut conn,
            &profile_id,
            &ProfileThemeInput {
                mode: "dark".into(),
                accent: "#A1b2C3".into(),
                density: Some("compact".into()),
            },
        )
        .unwrap();
        assert_eq!(updated.theme_id, "dark");
        let settings: serde_json::Value = serde_json::from_str(&updated.settings_json).unwrap();
        assert_eq!(settings["accent"], "#A1b2C3");
        assert_eq!(settings["density"], "compact");
        assert_eq!(settings["customSetting"]["keep"], true);
        assert_eq!(settings["theme"], "legacy");

        let cleared = profile_theme_set_on_conn(
            &mut conn,
            &profile_id,
            &ProfileThemeInput {
                mode: "system".into(),
                accent: "#112233".into(),
                density: None,
            },
        )
        .unwrap();
        let settings: serde_json::Value = serde_json::from_str(&cleared.settings_json).unwrap();
        assert!(settings.get("density").is_none());
        assert_eq!(settings["customSetting"]["keep"], true);
        let export = profile_package_from_db(&conn, &profile_id, false).unwrap();
        assert_eq!(export.profile.theme.mode, "system");
        assert_eq!(export.profile.theme.accent, "#112233");
        assert!(export.profile.theme.density.is_none());
        assert!(profile_theme_set_on_conn(
            &mut conn,
            &profile_id,
            &ProfileThemeInput {
                mode: "custom".into(),
                accent: "red".into(),
                density: Some("loose".into()),
            }
        )
        .is_err());
        let unchanged = conn
            .query_row(
                "SELECT theme_id FROM profiles WHERE id=?1",
                [&profile_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(unchanged, "system");
    }
    #[test]
    fn rejects_unknown_target_kinds_and_non_app_arguments() {
        let mut t = sample_target();
        t.kind = "script".into();
        assert!(validate_target(&t).is_err());
        t.kind = "file".into();
        t.arguments.push("--bad".into());
        assert!(validate_target(&t).is_err());
    }
    #[test]
    fn resource_ids_must_be_uuid() {
        let mut r = resource("x");
        r.id = "path-as-id".into();
        assert!(validate_resource(&r).is_err());
    }

    #[test]
    fn create_update_and_profile_membership_round_trip() {
        let mut conn = database();
        let pid = profile(&conn);
        let mut r = resource("Budget");
        persist_resource(&mut conn, &r, true, Some(&pid)).unwrap();
        conn.execute(
            "UPDATE profile_resources SET favorite=1 WHERE profile_id=?1 AND resource_id=?2",
            params![pid, r.id],
        )
        .unwrap();
        let mut loaded=conn.query_row("SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",[&r.id],resource_from_row).unwrap();
        load_resource_details(&conn, &mut loaded, Some(&pid)).unwrap();
        assert_eq!(loaded.tags, vec!["finance"]);
        assert_eq!(loaded.aliases, vec!["ledger"]);
        assert!(loaded.favorite);
        assert_eq!(loaded.category.as_deref(), Some("Planning"));
        r.display_name = "Annual Budget".into();
        r.tags = vec!["planning".into()];
        persist_resource(&mut conn, &r, false, None).unwrap();
        let name: String = conn
            .query_row(
                "SELECT display_name FROM resources WHERE id=?1",
                [&r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Annual Budget");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1 AND resource_id=?2",
                params![pid, r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn update_with_nonexistent_valid_profile_rolls_back_resource_and_membership() {
        let mut conn = database();
        let existing_profile = profile(&conn);
        let r = resource("Original title");
        persist_resource(&mut conn, &r, true, Some(&existing_profile)).unwrap();
        let before = conn
            .query_row(
                "SELECT display_name,updated_utc FROM resources WHERE id=?1",
                [&r.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        let before_target: String = conn
            .query_row(
                "SELECT canonical_uri FROM resource_targets WHERE resource_id=?1",
                [&r.id],
                |row| row.get(0),
            )
            .unwrap();
        let unknown_profile = Uuid::new_v4().to_string();
        assert_ne!(unknown_profile, existing_profile);
        let error = resource_update_in_conn(
            &mut conn,
            &r.id,
            ResourcePatch {
                display_name: Some("Should not persist".into()),
                profile_id: Some(unknown_profile.clone()),
                favorite: Some(true),
                category: Some("Changed category".into()),
                ..ResourcePatch::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, AppError::NotFound));
        let after = conn
            .query_row(
                "SELECT display_name,updated_utc FROM resources WHERE id=?1",
                [&r.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(after, before);
        let after_target: String = conn
            .query_row(
                "SELECT canonical_uri FROM resource_targets WHERE resource_id=?1",
                [&r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after_target, before_target);
        let invalid_membership_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1 AND resource_id=?2",
                params![unknown_profile, r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invalid_membership_count, 0);
        let membership_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE resource_id=?1",
                [&r.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(membership_count, 1);
    }

    #[test]
    fn search_matches_aliases_and_tags_and_respects_profile_and_favorite() {
        let mut conn = database();
        let pid = profile(&conn);
        let r = resource("Quarter plan");
        persist_resource(&mut conn, &r, true, Some(&pid)).unwrap();
        let alias = search_resources(&conn, "ledger", Some(&pid), false, 20).unwrap();
        assert_eq!(alias.len(), 1);
        let tag = search_resources(&conn, "finance", Some(&pid), false, 20).unwrap();
        assert_eq!(tag.len(), 1);
        assert!(search_resources(
            &conn,
            "ledger",
            Some(&Uuid::new_v4().to_string()),
            false,
            20
        )
        .unwrap()
        .is_empty());
        assert!(search_resources(&conn, "", Some(&pid), true, 20)
            .unwrap()
            .is_empty());
        conn.execute(
            "UPDATE profile_resources SET favorite=1 WHERE profile_id=?1 AND resource_id=?2",
            params![pid, r.id],
        )
        .unwrap();
        assert_eq!(
            search_resources(&conn, "", Some(&pid), true, 20)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn search_ranks_exact_prefix_substring_and_typo_matches() {
        let mut conn = database();
        let pid = profile(&conn);
        for name in ["Calendar", "Calendar Notes", "Planning Calendar"] {
            let resource = resource(name);
            persist_resource(&mut conn, &resource, true, Some(&pid)).unwrap();
        }
        let exact_prefix = search_resources(&conn, "calendar", Some(&pid), false, 20).unwrap();
        assert_eq!(exact_prefix[0].display_name, "Calendar");
        assert_eq!(exact_prefix[1].display_name, "Calendar Notes");
        assert_eq!(exact_prefix[2].display_name, "Planning Calendar");
        let typo = search_resources(&conn, "calender", Some(&pid), false, 20).unwrap();
        assert!(typo
            .iter()
            .any(|resource| resource.display_name == "Calendar"));
    }

    #[test]
    fn search_rejects_queries_over_byte_or_character_caps() {
        assert!(validate_search_query(&"x".repeat(MAX_SEARCH_QUERY_CHARS + 1)).is_err());
        assert!(validate_search_query(&"é".repeat(MAX_SEARCH_QUERY_BYTES / 2 + 1)).is_err());
        assert!(validate_search_query(&"é".repeat(MAX_SEARCH_QUERY_CHARS)).is_ok());
    }

    #[test]
    fn search_candidate_cap_bounds_scoring_but_keeps_requested_output_limit() {
        let mut conn = database();
        let pid = profile(&conn);
        for index in (0..MAX_SEARCH_CANDIDATES + 5).rev() {
            let resource = resource(&format!("Work item {index:04}"));
            persist_resource(&mut conn, &resource, true, Some(&pid)).unwrap();
        }
        let results = search_resources(&conn, "work", Some(&pid), false, 500).unwrap();
        assert_eq!(results.len(), MAX_SEARCH_CANDIDATES);
        assert_eq!(results[0].display_name, "Work item 0000");
        assert_eq!(results.last().unwrap().display_name, "Work item 0249");
        let limited = search_resources(&conn, "work", Some(&pid), false, 3).unwrap();
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn bounded_edit_distance_handles_typo_threshold_and_long_mismatch() {
        assert_eq!(edit_distance_at_most("calender", "calendar", 2), Some(1));
        assert_eq!(edit_distance_at_most("calendar", "calendars", 1), Some(1));
        assert_eq!(edit_distance_at_most("calendar", "unrelated", 2), None);
        assert_eq!(edit_distance_at_most("a", &"b".repeat(300), 2), None);
    }

    #[test]
    fn search_uses_favorites_then_local_launch_recency_as_tiebreakers() {
        let mut conn = database();
        let pid = profile(&conn);
        let older = resource("Work Alpha");
        let favorite = resource("Work Beta");
        let newer = resource("Work Gamma");
        for resource in [&older, &favorite, &newer] {
            persist_resource(&mut conn, resource, true, Some(&pid)).unwrap();
        }
        conn.execute(
            "UPDATE profile_resources SET favorite=1 WHERE profile_id=?1 AND resource_id=?2",
            params![pid, favorite.id],
        )
        .unwrap();
        for (resource, timestamp) in [
            (&older, "2026-10-19T00:00:00Z"),
            (&newer, "2026-10-20T00:00:00Z"),
        ] {
            conn.execute("INSERT INTO launch_events(id,request_id,resource_id,source,launched_at_utc,status) VALUES (?1,?2,?3,'dashboard',?4,'handed_off')", params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string(),resource.id,timestamp]).unwrap();
        }
        let results = search_resources(&conn, "work", Some(&pid), false, 20).unwrap();
        assert_eq!(results[0].display_name, "Work Beta");
        conn.execute(
            "UPDATE profile_resources SET favorite=0 WHERE profile_id=?1",
            [&pid],
        )
        .unwrap();
        let results = search_resources(&conn, "work", Some(&pid), false, 20).unwrap();
        assert_eq!(results[0].display_name, "Work Gamma");
    }

    #[test]
    fn launch_event_is_persisted_once_and_mock_handler_runs_once() {
        let mut conn = database();
        let pid = profile(&conn);
        let r = resource("Launch me");
        persist_resource(&mut conn, &r, true, Some(&pid)).unwrap();
        let request = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: r.id.clone(),
            target_id: None,
            profile_id: Some(pid),
            source: "quick_launch".into(),
        };
        let calls = std::cell::Cell::new(0);
        let first = launch_event_with(&mut conn, &request, |_| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        let second = launch_event_with(&mut conn, &request, |_| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(first.status, "handed_off");
        assert_eq!(first.request_id, second.request_id);
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_events WHERE request_id=?1",
                [&request.request_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
        let mut loaded=conn.query_row("SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",[&r.id],resource_from_row).unwrap();
        load_resource_details(&conn, &mut loaded, None).unwrap();
        assert_eq!(loaded.launch_count, 1);
        assert!(loaded.last_launched_utc.is_some());
    }

    #[test]
    fn launch_request_replay_rejects_reused_id_and_checks_profile_membership() {
        let mut conn = database();
        let pid = profile(&conn);
        let mut allowed = resource("Allowed resource");
        let mut lower_priority_target = allowed.targets[0].clone();
        lower_priority_target.id = Uuid::new_v4().to_string();
        lower_priority_target.priority = 1;
        let mut default_target = allowed.targets[0].clone();
        default_target.id = Uuid::new_v4().to_string();
        default_target.priority = 10;
        allowed.targets = vec![lower_priority_target.clone(), default_target];
        let disallowed = resource("Other profile resource");
        persist_resource(&mut conn, &allowed, true, Some(&pid)).unwrap();
        persist_resource(&mut conn, &disallowed, true, None).unwrap();
        let request = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: allowed.id.clone(),
            target_id: None,
            profile_id: Some(pid.clone()),
            source: "dashboard".into(),
        };
        let explicit_lower_priority = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: allowed.id.clone(),
            target_id: Some(lower_priority_target.id),
            profile_id: Some(pid.clone()),
            source: "dashboard".into(),
        };
        launch_event_with(&mut conn, &explicit_lower_priority, |_| Ok(())).unwrap();
        let mut target_intent_changed = explicit_lower_priority.clone();
        target_intent_changed.target_id = None;
        assert!(matches!(
            launch_event_with(&mut conn, &target_intent_changed, |_| panic!(
                "must not relaunch"
            )),
            Err(AppError::Invalid(_))
        ));

        launch_event_with(&mut conn, &request, |_| Ok(())).unwrap();
        assert!(launch_event_with(&mut conn, &request, |_| panic!(
            "idempotent replay must not relaunch"
        ))
        .is_ok());
        let mut reused_id = request.clone();
        reused_id.resource_id = disallowed.id.clone();
        assert!(launch_event_with(&mut conn, &reused_id, |_| panic!("must not relaunch")).is_err());

        let unauthorized = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: disallowed.id,
            target_id: None,
            profile_id: Some(pid),
            source: "dashboard".into(),
        };
        assert!(matches!(
            launch_event_with(&mut conn, &unauthorized, |_| panic!("must not launch")),
            Err(AppError::NotFound)
        ));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM launch_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn launch_replay_rejects_unknown_explicit_target_when_original_had_none() {
        let mut conn = database();
        let r = resource("No active targets");
        let target_id = r.targets[0].id.clone();
        persist_resource(&mut conn, &r, true, None).unwrap();
        conn.execute(
            "UPDATE resource_targets SET active=0 WHERE id=?1",
            [&target_id],
        )
        .unwrap();
        let request = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: r.id.clone(),
            target_id: None,
            profile_id: None,
            source: "dashboard".into(),
        };
        let first =
            launch_event_with(&mut conn, &request, |_| panic!("no target to launch")).unwrap();
        assert_eq!(first.target_id, None);
        assert_eq!(first.status, "failed");

        let mut retry = request;
        retry.target_id = Some(Uuid::new_v4().to_string());
        assert!(matches!(
            launch_event_with(&mut conn, &retry, |_| panic!("must not launch")),
            Err(AppError::NotFound)
        ));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM launch_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn smart_collections_are_profile_scoped_validated_and_upserted() {
        let mut conn = database();
        let first_profile = profile(&conn);
        let second_profile = profile(&conn);
        conn.execute(
            "UPDATE profiles SET name='Other profile' WHERE id=?1",
            [&second_profile],
        )
        .unwrap();
        let collection_id = Uuid::new_v4().to_string();
        let collection = SmartCollection {
            id: collection_id.clone(),
            profile_id: first_profile.clone(),
            name: "  Work files  ".into(),
            filters: SmartCollectionFilters {
                resource_type: Some("file".into()),
                category: Some("Work".into()),
                tag: Some("important".into()),
                favorite_only: Some(true),
                text: Some("quarterly".into()),
            },
        };
        let saved = smart_collection_save_on_conn(&mut conn, collection.clone()).unwrap();
        assert_eq!(saved.name, "Work files");
        assert_eq!(
            smart_collection_list_on_conn(&conn, &first_profile).unwrap(),
            vec![saved.clone()]
        );
        assert!(smart_collection_list_on_conn(&conn, &second_profile)
            .unwrap()
            .is_empty());

        let updated = SmartCollection {
            name: "Quarterly reports".into(),
            filters: SmartCollectionFilters {
                text: Some("reports".into()),
                favorite_only: Some(false),
                ..SmartCollectionFilters::default()
            },
            ..saved
        };
        let updated = smart_collection_save_on_conn(&mut conn, updated.clone()).unwrap();
        assert_eq!(
            smart_collection_list_on_conn(&conn, &first_profile).unwrap(),
            vec![updated.clone()]
        );

        let wrong_owner = SmartCollection {
            profile_id: second_profile.clone(),
            ..updated.clone()
        };
        assert!(matches!(
            smart_collection_save_on_conn(&mut conn, wrong_owner),
            Err(AppError::NotFound)
        ));
        assert!(matches!(
            smart_collection_delete_on_conn(&conn, &second_profile, &collection_id),
            Err(AppError::NotFound)
        ));
        smart_collection_delete_on_conn(&conn, &first_profile, &collection_id).unwrap();
        assert!(smart_collection_list_on_conn(&conn, &first_profile)
            .unwrap()
            .is_empty());

        let duplicate_name = SmartCollection {
            id: Uuid::new_v4().to_string(),
            profile_id: first_profile.clone(),
            name: "Same name".into(),
            filters: SmartCollectionFilters::default(),
        };
        smart_collection_save_on_conn(&mut conn, duplicate_name.clone()).unwrap();
        let duplicate = SmartCollection {
            id: Uuid::new_v4().to_string(),
            name: "same NAME".into(),
            ..duplicate_name
        };
        assert!(smart_collection_save_on_conn(&mut conn, duplicate).is_err());
    }

    #[test]
    fn smart_collection_filter_dsl_rejects_unknown_fields_and_unbounded_values() {
        assert!(
            serde_json::from_value::<SmartCollectionFilters>(serde_json::json!({
                "resourceType": "file", "sql": "DROP TABLE resources"
            }))
            .is_err()
        );
        let collection = SmartCollection {
            id: Uuid::new_v4().to_string(),
            profile_id: Uuid::new_v4().to_string(),
            name: "Valid name".into(),
            filters: SmartCollectionFilters {
                text: Some("x".repeat(201)),
                ..SmartCollectionFilters::default()
            },
        };
        assert!(validate_smart_collection(&collection).is_err());
        let control_value = SmartCollection {
            filters: SmartCollectionFilters {
                category: Some("Work\nFiles".into()),
                ..SmartCollectionFilters::default()
            },
            ..SmartCollection {
                id: Uuid::new_v4().to_string(),
                profile_id: Uuid::new_v4().to_string(),
                name: "Valid name".into(),
                filters: SmartCollectionFilters::default(),
            }
        };
        assert!(validate_smart_collection(&control_value).is_err());
        let invalid_type = SmartCollection {
            filters: SmartCollectionFilters {
                resource_type: Some("script".into()),
                ..SmartCollectionFilters::default()
            },
            ..collection
        };
        assert!(validate_smart_collection(&invalid_type).is_err());
    }

    #[test]
    fn dynamic_shortcuts_are_profile_scoped_and_validate_membership_targets_and_names() {
        let mut conn = database();
        let source = package("file");
        let resource_id = source.resources.resources[0].id.clone();
        let target_id = source.resources.resources[0].targets[0].id.clone();
        let imported_profile = import_profile_package(&mut conn, &source, "duplicate", None)
            .unwrap()
            .profile_id;
        let imported_resource: String = conn
            .query_row(
                "SELECT resource_id FROM profile_resources WHERE profile_id=?1",
                [&imported_profile],
                |row| row.get(0),
            )
            .unwrap();
        let imported_target: String = conn
            .query_row(
                "SELECT id FROM resource_targets WHERE resource_id=?1",
                [&imported_resource],
                |row| row.get(0),
            )
            .unwrap();
        let first = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            profile_id: imported_profile.clone(),
            name: "  Quick Calculator  ".into(),
            resource_id: imported_resource.clone(),
            target_id: Some(imported_target.clone()),
        };
        let saved = dynamic_shortcut_save_on_conn(&mut conn, first.clone()).unwrap();
        assert_eq!(saved.name, "Quick Calculator");
        assert_eq!(
            dynamic_shortcut_list_on_conn(&conn, &imported_profile).unwrap(),
            vec![saved.clone()]
        );

        let other_profile = profile(&conn);
        assert!(dynamic_shortcut_list_on_conn(&conn, &other_profile)
            .unwrap()
            .is_empty());
        let wrong_owner = DynamicShortcut {
            profile_id: other_profile.clone(),
            ..saved.clone()
        };
        assert!(matches!(
            dynamic_shortcut_save_on_conn(&mut conn, wrong_owner),
            Err(AppError::NotFound)
        ));
        let wrong_target = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Wrong target".into(),
            target_id: Some(target_id),
            ..saved.clone()
        };
        assert!(matches!(
            dynamic_shortcut_save_on_conn(&mut conn, wrong_target),
            Err(AppError::NotFound)
        ));
        let unmembered = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Unmembered".into(),
            resource_id,
            target_id: None,
            ..saved.clone()
        };
        assert!(matches!(
            dynamic_shortcut_save_on_conn(&mut conn, unmembered),
            Err(AppError::NotFound)
        ));
        let duplicate_name = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "quick calculator".into(),
            ..saved.clone()
        };
        assert!(dynamic_shortcut_save_on_conn(&mut conn, duplicate_name).is_err());
        let invalid_name = DynamicShortcut {
            name: "Bad\nName".into(),
            ..saved.clone()
        };
        assert!(validate_dynamic_shortcut(&invalid_name).is_err());
        dynamic_shortcut_delete_on_conn(&conn, &imported_profile, &saved.id).unwrap();
        assert!(dynamic_shortcut_list_on_conn(&conn, &imported_profile)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn dynamic_shortcuts_roundtrip_and_duplicate_import_remaps_ids_and_names() {
        let mut conn = database();
        let mut source = package("file");
        let source_shortcut = ProfilePackageDynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Pinned calculator".into(),
            resource_id: source.resources.resources[0].id.clone(),
            target_id: Some(source.resources.resources[0].targets[0].id.clone()),
        };
        source.profile.dynamic_shortcuts = vec![source_shortcut.clone()];
        validate_package_fields(&source).unwrap();
        let first = import_profile_package(&mut conn, &source, "duplicate", None).unwrap();
        let first_records = dynamic_shortcut_list_on_conn(&conn, &first.profile_id).unwrap();
        assert_eq!(first_records.len(), 1);
        assert_ne!(first_records[0].id, source_shortcut.id);
        let exported = profile_package_from_db(&conn, &first.profile_id, false).unwrap();
        assert_eq!(exported.profile.dynamic_shortcuts.len(), 1);
        assert_eq!(
            exported.profile.dynamic_shortcuts[0].name,
            "Pinned calculator"
        );
        assert!(exported.history.is_none());

        let second = import_profile_package(&mut conn, &source, "duplicate", None).unwrap();
        let second_records = dynamic_shortcut_list_on_conn(&conn, &second.profile_id).unwrap();
        assert_eq!(second_records.len(), 1);
        assert_ne!(second_records[0].id, first_records[0].id);
        assert_eq!(second_records[0].name, "Pinned calculator");
    }

    #[test]
    fn dynamic_shortcut_merge_collision_preserves_existing_and_allocates_unique_name() {
        let mut conn = database();
        let mut source = package("file");
        let package_shortcut = ProfilePackageDynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Pinned item".into(),
            resource_id: source.resources.resources[0].id.clone(),
            target_id: Some(source.resources.resources[0].targets[0].id.clone()),
        };
        source.profile.dynamic_shortcuts = vec![package_shortcut.clone()];
        let target_profile = profile(&conn);
        install_package_resource(&mut conn, &source, &target_profile);
        let existing = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            profile_id: target_profile.clone(),
            name: package_shortcut.name.clone(),
            resource_id: package_shortcut.resource_id.clone(),
            target_id: None,
        };
        let existing = dynamic_shortcut_save_on_conn(&mut conn, existing).unwrap();

        import_profile_package(&mut conn, &source, "merge", Some(&target_profile)).unwrap();
        let records = dynamic_shortcut_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(records.len(), 2);
        assert!(records
            .iter()
            .any(|item| item.id == existing.id && item.target_id.is_none()));
        assert!(records
            .iter()
            .any(|item| item.name == "Pinned item (imported 2)"));
    }

    #[test]
    fn profile_import_replace_replaces_dynamic_shortcuts_and_remaps_references() {
        let mut conn = database();
        let mut package = package("url");
        let incoming = ProfilePackageDynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Incoming shortcut".into(),
            resource_id: package.resources.resources[0].id.clone(),
            target_id: Some(package.resources.resources[0].targets[0].id.clone()),
        };
        package.profile.dynamic_shortcuts = vec![incoming.clone()];
        validate_package_fields(&package).unwrap();

        let target_profile = profile(&conn);
        let old_resource = resource("Existing local resource");
        persist_resource(&mut conn, &old_resource, true, Some(&target_profile)).unwrap();
        let old_shortcut = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            profile_id: target_profile.clone(),
            name: "Old shortcut".into(),
            resource_id: old_resource.id.clone(),
            target_id: Some(old_resource.targets[0].id.clone()),
        };
        dynamic_shortcut_save_on_conn(&mut conn, old_shortcut.clone()).unwrap();

        import_profile_package(&mut conn, &package, "replace", Some(&target_profile)).unwrap();
        let shortcuts = dynamic_shortcut_list_on_conn(&conn, &target_profile).unwrap();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].id, incoming.id);
        assert_eq!(shortcuts[0].name, incoming.name);
        assert_ne!(shortcuts[0].resource_id, incoming.resource_id);
        assert_ne!(
            shortcuts[0].target_id.as_deref(),
            incoming.target_id.as_deref()
        );
        assert_eq!(shortcuts[0].profile_id, target_profile);
        let is_member: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profile_resources WHERE profile_id=?1 AND resource_id=?2)",
                params![target_profile, shortcuts[0].resource_id],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )
            .unwrap();
        assert!(is_member);
        assert_ne!(shortcuts[0].id, old_shortcut.id);
    }

    #[test]
    fn profile_import_rolls_back_replace_when_dynamic_shortcut_insert_fails() {
        let mut conn = database();
        let mut package = package("url");
        package.profile.dynamic_shortcuts = vec![ProfilePackageDynamicShortcut {
            id: Uuid::new_v4().to_string(),
            name: "Incoming shortcut".into(),
            resource_id: package.resources.resources[0].id.clone(),
            target_id: Some(package.resources.resources[0].targets[0].id.clone()),
        }];
        validate_package_fields(&package).unwrap();

        let target_profile = profile(&conn);
        let old_resource = resource("Existing local resource");
        persist_resource(&mut conn, &old_resource, true, Some(&target_profile)).unwrap();
        let old_shortcut = DynamicShortcut {
            id: Uuid::new_v4().to_string(),
            profile_id: target_profile.clone(),
            name: "Old shortcut".into(),
            resource_id: old_resource.id.clone(),
            target_id: Some(old_resource.targets[0].id.clone()),
        };
        dynamic_shortcut_save_on_conn(&mut conn, old_shortcut.clone()).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_dynamic_shortcut_import BEFORE INSERT ON dynamic_shortcuts BEGIN SELECT RAISE(ABORT,'injected dynamic shortcut failure'); END;").unwrap();
        let resource_count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
            .unwrap();

        assert!(
            import_profile_package(&mut conn, &package, "replace", Some(&target_profile)).is_err()
        );
        let profile_name: String = conn
            .query_row(
                "SELECT name FROM profiles WHERE id=?1",
                [&target_profile],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(profile_name, "Work");
        assert_eq!(
            dynamic_shortcut_list_on_conn(&conn, &target_profile).unwrap(),
            vec![old_shortcut]
        );
        let incoming_membership_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1 AND resource_id=?2",
                params![target_profile, package.resources.resources[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(incoming_membership_count, 0);
        let resource_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(resource_count_after, resource_count_before);
    }

    #[test]
    fn metadata_inspection_caches_size_and_force_refreshes_local_file() {
        let mut conn = database();
        let r = resource("Metadata file");
        persist_resource(&mut conn, &r, true, None).unwrap();
        let path = std::env::temp_dir().join(format!("sw007-{}.dat", Uuid::new_v4()));
        std::fs::write(&path, b"first").unwrap();
        conn.execute(
            "UPDATE resource_targets SET kind='file',canonical_uri=?1 WHERE resource_id=?2",
            params![path.to_string_lossy(), r.id],
        )
        .unwrap();
        let first = inspect_resource_metadata(&conn, &r.id, false).unwrap();
        assert_eq!(first[0].availability, "available");
        assert_eq!(first[0].provenance, "local-filesystem");
        assert_eq!(first[0].cache_status, "fresh");
        assert_eq!(first[0].file_size_bytes, Some(5));
        assert!(first[0].modified_utc.is_some());
        std::fs::write(&path, b"updated size").unwrap();
        let cached = inspect_resource_metadata(&conn, &r.id, false).unwrap();
        assert_eq!(cached[0].cache_status, "cached");
        assert_eq!(cached[0].file_size_bytes, Some(5));
        let refreshed = inspect_resource_metadata(&conn, &r.id, true).unwrap();
        assert_eq!(refreshed[0].cache_status, "fresh");
        assert_eq!(refreshed[0].file_size_bytes, Some(12));
        let stale_snapshot = load_metadata_snapshot(&conn, &r.id).unwrap();
        let transient_error = inspect_metadata_snapshot(
            &r.id,
            stale_snapshot,
            true,
            None,
            |resource_id, target| ResourceMetadata {
                resource_id: resource_id.into(),
                target_id: target.id.clone(),
                inspected_utc: now_utc(),
                file_size_bytes: None,
                created_utc: None,
                modified_utc: None,
                availability: "inaccessible".into(),
                provenance: "local-filesystem".into(),
                cache_status: "error".into(),
                error_code: Some("metadata_error".into()),
            },
            |record| store_metadata_record(&conn, record),
        )
        .unwrap();
        assert_eq!(transient_error[0].file_size_bytes, Some(12));
        assert_eq!(transient_error[0].availability, "inaccessible");
        assert_eq!(transient_error[0].cache_status, "error");
        std::fs::remove_file(path).unwrap();
        let missing = inspect_resource_metadata(&conn, &r.id, true).unwrap();
        assert_eq!(missing[0].availability, "missing");
        assert_eq!(missing[0].error_code.as_deref(), Some("not_found"));
    }

    #[test]
    fn metadata_filesystem_work_runs_without_holding_the_database_mutex() {
        let mut conn = database();
        let r = resource("Slow metadata");
        persist_resource(&mut conn, &r, true, None).unwrap();
        let db = Arc::new(Mutex::new(conn));
        let inspector_db = Arc::clone(&db);
        let lock_was_available = std::sync::atomic::AtomicBool::new(false);
        let lock_was_available_ref = &lock_was_available;
        let result = inspect_resource_metadata_on_db_with(
            &db,
            &r.id,
            true,
            None,
            move |resource_id, target| {
                lock_was_available_ref.store(inspector_db.try_lock().is_ok(), Ordering::Release);
                std::thread::sleep(Duration::from_millis(25));
                ResourceMetadata {
                    resource_id: resource_id.into(),
                    target_id: target.id.clone(),
                    inspected_utc: now_utc(),
                    file_size_bytes: Some(7),
                    created_utc: None,
                    modified_utc: None,
                    availability: "available".into(),
                    provenance: "local-filesystem".into(),
                    cache_status: "fresh".into(),
                    error_code: None,
                }
            },
        )
        .unwrap();
        assert!(lock_was_available.load(Ordering::Acquire));
        assert_eq!(result[0].file_size_bytes, Some(7));
    }

    #[test]
    fn timed_out_metadata_work_stops_at_the_next_cancellation_checkpoint() {
        let mut conn = database();
        let r = resource("Cancelled metadata");
        persist_resource(&mut conn, &r, true, None).unwrap();
        let db = Arc::new(Mutex::new(conn));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let result = inspect_resource_metadata_on_db_with(
            &db,
            &r.id,
            true,
            Some(Arc::clone(&cancelled)),
            move |resource_id, target| {
                worker_cancelled.store(true, Ordering::Release);
                ResourceMetadata {
                    resource_id: resource_id.into(),
                    target_id: target.id.clone(),
                    inspected_utc: now_utc(),
                    file_size_bytes: Some(7),
                    created_utc: None,
                    modified_utc: None,
                    availability: "available".into(),
                    provenance: "local-filesystem".into(),
                    cache_status: "fresh".into(),
                    error_code: None,
                }
            },
        );
        assert!(result.is_err());
        let conn = db.lock().unwrap();
        let cached_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM metadata_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(cached_count, 0);
    }

    #[test]
    fn metadata_skips_network_targets_and_caps_active_target_count() {
        let mut conn = database();
        let r = resource("Network target");
        persist_resource(&mut conn, &r, true, None).unwrap();
        conn.execute(
            "UPDATE resource_targets SET kind='file',canonical_uri='\\\\server\\share\\document.txt' WHERE resource_id=?1",
            [&r.id],
        )
        .unwrap();
        let skipped = inspect_resource_metadata(&conn, &r.id, true).unwrap();
        assert_eq!(skipped[0].availability, "unsupported");
        assert_eq!(skipped[0].provenance, "unsupported");
        assert_eq!(
            skipped[0].error_code.as_deref(),
            Some("network_path_skipped")
        );
        for _ in 0..MAX_METADATA_TARGETS {
            let mut target = sample_target();
            target.id = Uuid::new_v4().to_string();
            target.canonical_uri = format!("C:\\unused\\{}.txt", target.id);
            conn.execute(
                "INSERT INTO resource_targets(id,resource_id,kind,canonical_uri,priority,arguments_json,active) VALUES (?1,?2,'file',?3,0,'[]',1)",
                params![target.id, r.id, target.canonical_uri],
            )
            .unwrap();
        }
        assert!(inspect_resource_metadata(&conn, &r.id, true).is_err());
    }

    #[test]
    fn analytics_summary_aggregates_only_local_launch_events_by_profile_source_and_status() {
        let conn = database();
        let pid = profile(&conn);
        for (source, status, timestamp, profile_id) in [
            (
                "dashboard",
                "handed_off",
                "2026-10-19T10:00:00Z",
                Some(pid.as_str()),
            ),
            (
                "quick_launch",
                "failed",
                "2026-10-20T10:00:00Z",
                Some(pid.as_str()),
            ),
            ("dock", "handed_off", "2026-10-21T10:00:00Z", None),
        ] {
            conn.execute(
                "INSERT INTO launch_events(id,request_id,resource_id,profile_id,source,launched_at_utc,status) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string(),Uuid::new_v4().to_string(),profile_id,source,timestamp,status],
            )
            .unwrap();
        }
        let summary = analytics_summary_for(&conn, &pid, &UsageFilter::default()).unwrap();
        assert_eq!(summary.total_launches, 2);
        assert_eq!(summary.by_source.get("dashboard"), Some(&1));
        assert_eq!(summary.by_source.get("quick_launch"), Some(&1));
        assert_eq!(summary.by_status.get("handed_off"), Some(&1));
        assert_eq!(summary.by_status.get("failed"), Some(&1));
        assert_eq!(
            summary.first_event_utc.as_deref(),
            Some("2026-10-19T10:00:00Z")
        );
        assert_eq!(
            summary.last_event_utc.as_deref(),
            Some("2026-10-20T10:00:00Z")
        );
        let after_filter = UsageFilter {
            from_utc: Some("2026-10-20T00:00:00.000Z".into()),
            ..Default::default()
        };
        let after = analytics_summary_for(&conn, &pid, &after_filter).unwrap();
        assert_eq!(after.total_launches, 1);
        assert_eq!(after.by_source.get("quick_launch"), Some(&1));
    }

    #[test]
    fn stopped_recording_and_purge_preserve_request_replay_guard() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let resource = resource("No History");
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        conn.execute("INSERT INTO usage_settings(profile_id,recording_enabled,retention_days,excluded_resource_ids_json) VALUES (?1,0,NULL,'[]')", [&profile_id]).unwrap();
        let request = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: resource.id.clone(),
            profile_id: Some(profile_id.clone()),
            target_id: None,
            source: "dashboard".into(),
        };
        let mut launches = 0;
        let result = launch_event_with(&mut conn, &request, |_| {
            launches += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(result.status, "handed_off");
        assert_eq!(launches, 1);
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_events WHERE profile_id=?1",
                [&profile_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            event_count, 0,
            "disabled history must not write analytics events"
        );
        let guard_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_request_guard WHERE request_id=?1",
                [&request.request_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(guard_count, 1);
        conn.execute(
            "DELETE FROM launch_events WHERE profile_id=?1",
            [&profile_id],
        )
        .unwrap();
        let replay = launch_event_with(&mut conn, &request, |_| {
            panic!("replay must not call Shell twice")
        })
        .unwrap();
        assert_eq!(replay.status, "handed_off");
        let reused = LaunchRequest {
            source: "dock".into(),
            ..request.clone()
        };
        assert!(launch_event_with(&mut conn, &reused, |_| panic!(
            "different intent must not launch"
        ))
        .is_err());
        assert_eq!(launches, 1);
        let still_there: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_request_guard WHERE request_id=?1",
                [&request.request_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            still_there, 1,
            "purging analytics must preserve anti-replay state"
        );

        let sequence_resource = resource("Sequence No History");
        persist_resource(&mut conn, &sequence_resource, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(
            &profile_id,
            "No History Sequence",
            &[(&sequence_resource, None)],
        );
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let sequence_request = Uuid::new_v4().to_string();
        let first_run = workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &sequence_request,
            |_| {
                launches += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(first_run.status, "completed");
        let stored_sequence: String = conn
            .query_row(
                "SELECT sequence_json FROM workspace_sequence_runs WHERE request_id=?1",
                [&sequence_request],
                |r| r.get(0),
            )
            .unwrap();
        let stored_result_json: String = conn
            .query_row(
                "SELECT result_json FROM workspace_sequence_runs WHERE request_id=?1",
                [&sequence_request],
                |r| r.get(0),
            )
            .unwrap();
        let stored_result: WorkspaceSequenceRunResult =
            serde_json::from_str(&stored_result_json).unwrap();
        assert!(
            stored_sequence.is_empty(),
            "recording-off terminal run must discard its sequence snapshot"
        );
        assert!(
            stored_result.steps.is_empty(),
            "recording-off terminal run must discard per-step details"
        );
        let sequence_event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_events WHERE profile_id=?1 AND source='sequence'",
                [&profile_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sequence_event_count, 0);
        let replay = workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &sequence_request,
            |_| panic!("sequence replay must not call Shell twice"),
        )
        .unwrap();
        assert_eq!(replay.status, "completed");
        assert_eq!(launches, 2);
    }

    #[test]
    fn usage_filters_normalize_offset_timestamps_and_csv_neutralizes_formulas() {
        let filter = validate_usage_filter(UsageFilter {
            from_utc: Some("2026-10-21T03:00:00+03:00".into()),
            to_utc: Some("2026-10-21T02:00:00Z".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(filter.from_utc.as_deref(), Some("2026-10-21T00:00:00.000Z"));
        assert_eq!(filter.to_utc.as_deref(), Some("2026-10-21T02:00:00.000Z"));
        assert!(validate_usage_filter(UsageFilter {
            from_utc: Some("2026-10-22T00:00:00Z".into()),
            to_utc: Some("2026-10-21T00:00:00Z".into()),
            ..Default::default()
        })
        .is_err());
        assert_eq!(csv_safe("  =1+1"), "\"'  =1+1\"");
        assert_eq!(csv_safe("\t@SUM(A1:A2)"), "\"'\t@SUM(A1:A2)\"");
        assert_eq!(csv_safe("safe, text"), "\"safe, text\"");
    }

    #[test]
    fn usage_csv_never_exports_user_controlled_resource_names_or_error_text() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let resource = resource("C:\\private\\secret.txt");
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        let request_id = Uuid::new_v4().to_string();
        conn.execute("INSERT INTO launch_events(id,request_id,resource_id,profile_id,source,launched_at_utc,status,error_code,message) VALUES (?1,?2,?3,?4,'dashboard','2026-10-21T00:00:00.000Z','failed',?5,?5)", params![Uuid::new_v4().to_string(),request_id,resource.id,profile_id,"C:\\private\\secret.txt"]).unwrap();
        let path = std::env::temp_dir().join(format!("sw015-{}.csv", Uuid::new_v4()));
        let query = UsageQuery {
            profile_id,
            from_utc: None,
            to_utc: None,
            source: None,
            status: None,
            resource_id: None,
            limit: None,
            offset: None,
        };
        let result = usage_export_csv_to_path(&conn, query, path.to_str().unwrap()).unwrap();
        assert_eq!(result.event_count, 1);
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv.starts_with("request_id,resource_id,launched_at_utc,source,status,duration_ms"));
        assert!(!csv.contains("display_name"));
        assert!(!csv.contains("secret.txt"));
        assert!(csv.contains(&request_id));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn retention_prunes_only_analytics_rows_and_keeps_guard_records() {
        let conn = database();
        let profile_id = profile(&conn);
        let resource_id = Uuid::new_v4().to_string();
        let request_id = Uuid::new_v4().to_string();
        conn.execute("INSERT INTO usage_settings(profile_id,recording_enabled,retention_days,excluded_resource_ids_json) VALUES (?1,1,30,'[]')", [&profile_id]).unwrap();
        conn.execute("INSERT INTO launch_events(id,request_id,resource_id,profile_id,source,launched_at_utc,status) VALUES (?1,?2,?3,?4,'dashboard','2020-01-01T00:00:00.000Z','handed_off')",params![Uuid::new_v4().to_string(),request_id,resource_id,profile_id]).unwrap();
        conn.execute("INSERT INTO launch_request_guard(request_id,intent_hash,resource_id,profile_id,source,status,launched_at_utc) VALUES (?1,'hash',?2,?3,'dashboard','handed_off','2020-01-01T00:00:00.000Z')",params![request_id,resource_id,profile_id]).unwrap();
        let resource = resource("Retained sequence");
        let mut conn = conn;
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(&profile_id, "Old sequence", &[(&resource, None)]);
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let sequence_request = Uuid::new_v4().to_string();
        workspace_sequence_run_with(
            &mut conn,
            &sequence.id,
            &profile_id,
            &sequence_request,
            |_| Ok(()),
        )
        .unwrap();
        conn.execute("UPDATE workspace_sequence_runs SET updated_utc='2020-01-01T00:00:00.000Z' WHERE request_id=?1",[&sequence_request]).unwrap();
        conn.execute("UPDATE launch_events SET launched_at_utc='2020-01-01T00:00:00.000Z' WHERE profile_id=?1",[&profile_id]).unwrap();
        let settings = usage_settings_for(&conn, &profile_id).unwrap();
        assert_eq!(settings.retention_days, Some(30));
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_events WHERE profile_id=?1",
                [&profile_id],
                |r| r.get(0),
            )
            .unwrap();
        let guards: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM launch_request_guard WHERE request_id=?1",
                [&request_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(events, 0);
        assert_eq!(guards, 1);
        let (snapshot, result_json): (String, String) = conn
            .query_row(
                "SELECT sequence_json,result_json FROM workspace_sequence_runs WHERE request_id=?1",
                [&sequence_request],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let compact: WorkspaceSequenceRunResult = serde_json::from_str(&result_json).unwrap();
        assert!(
            snapshot.is_empty(),
            "retention must discard the full terminal sequence snapshot"
        );
        assert!(
            compact.steps.is_empty(),
            "retention must discard per-step results with expired history"
        );
    }

    #[test]
    fn explicit_history_purge_compacts_terminal_sequence_results_and_replay_stays_safe() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let resource = resource("Purge sequence");
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        let sequence = workspace_sequence(&profile_id, "Purge sequence", &[(&resource, None)]);
        workspace_sequence_save_on_conn(&mut conn, sequence.clone()).unwrap();
        let request_id = Uuid::new_v4().to_string();
        let mut launches = 0;
        let initial =
            workspace_sequence_run_with(&mut conn, &sequence.id, &profile_id, &request_id, |_| {
                launches += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(initial.steps.len(), 1);
        let event_count = purge_usage_history_for_profile(&conn, &profile_id).unwrap();
        assert_eq!(event_count, 1);
        let (sequence_json, result_json): (String, String) = conn
            .query_row(
                "SELECT sequence_json,result_json FROM workspace_sequence_runs WHERE request_id=?1",
                [&request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let compact: WorkspaceSequenceRunResult = serde_json::from_str(&result_json).unwrap();
        assert!(sequence_json.is_empty());
        assert!(compact.steps.is_empty());
        assert_eq!(compact.total_steps, 1);
        let replay =
            workspace_sequence_run_with(&mut conn, &sequence.id, &profile_id, &request_id, |_| {
                panic!("purged sequence replay must not relaunch")
            })
            .unwrap();
        assert_eq!(replay.status, "completed");
        assert!(replay.steps.is_empty());
        assert_eq!(launches, 1);
    }

    #[test]
    fn migration_removes_guard_messages_and_replays_generic_safe_text() {
        let conn = database();
        let profile_id = profile(&conn);
        let resource_id = Uuid::new_v4().to_string();
        let request_id = Uuid::new_v4().to_string();
        conn.execute_batch("ALTER TABLE launch_request_guard ADD COLUMN message TEXT;")
            .unwrap();
        conn.execute("INSERT INTO launch_request_guard(request_id,intent_hash,resource_id,profile_id,source,status,launched_at_utc,error_code,duration_ms,message) VALUES (?1,'hash',?2,?3,'dashboard','failed','2026-10-21T00:00:00Z','target_not_found',0,'C:\\\\private\\\\secret.txt')",params![request_id,resource_id,profile_id]).unwrap();
        migrate_database(&conn).unwrap();
        let has_message: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_table_info('launch_request_guard') WHERE name='message'",[],|r|r.get(0)).unwrap();
        assert_eq!(has_message, 0);
        let event = usage_event_from_guard(&conn, &request_id).unwrap();
        assert_eq!(
            event.message.as_deref(),
            Some("The target could not be handed to Windows Shell.")
        );
        assert!(!event.message.unwrap().contains("secret.txt"));
    }

    #[test]
    fn missing_target_is_recorded_as_failed_attempt() {
        let mut conn = database();
        let r = resource("No target");
        persist_resource(&mut conn, &r, true, None).unwrap();
        let request = LaunchRequest {
            request_id: Uuid::new_v4().to_string(),
            resource_id: r.id.clone(),
            target_id: Some(Uuid::new_v4().to_string()),
            profile_id: None,
            source: "explorer".into(),
        };
        let event = launch_event_with(&mut conn, &request, |_| {
            panic!("handler must not run without target")
        })
        .unwrap();
        assert_eq!(event.status, "failed");
        assert_eq!(event.error_code.as_deref(), Some("target_not_found"));
    }

    #[test]
    fn layout_save_get_round_trip_and_stale_membership_filter() {
        let mut conn = database();
        let pid = profile(&conn);
        let resource = resource("Pinned item");
        persist_resource(&mut conn, &resource, true, Some(&pid)).unwrap();
        let layout = DashboardLayout {
            profile_id: pid.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: resource.id.clone(),
                x: 0,
                y: 12,
                w: 3,
                h: 2,
                card_type: "compact".into(),
            }],
        };
        let saved = layout_save_to_conn(&mut conn, &layout).unwrap();
        assert_eq!(saved.cards.len(), 1);
        assert_eq!(
            layout_get_from_conn(&conn, &pid, "dashboard")
                .unwrap()
                .cards
                .len(),
            1
        );
        conn.execute(
            "DELETE FROM profile_resources WHERE profile_id=?1 AND resource_id=?2",
            params![pid, resource.id],
        )
        .unwrap();
        assert!(layout_get_from_conn(&conn, &pid, "dashboard")
            .unwrap()
            .cards
            .is_empty());
    }

    #[test]
    fn resource_relationships_are_profile_scoped_undirected_and_unique() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let other_profile_id = Uuid::new_v4().to_string();
        conn.execute("INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,'Other','system','{}',?2,?2)", params![other_profile_id, now_utc()]).unwrap();
        let first = resource("First");
        let second = resource("Second");
        persist_resource(&mut conn, &first, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&profile_id)).unwrap();

        let relationship_id = Uuid::new_v4().to_string();
        let saved = resource_relationship_save_on_conn(
            &mut conn,
            ResourceRelationship {
                id: relationship_id.clone(),
                profile_id: profile_id.clone(),
                source_resource_id: second.id.clone(),
                target_resource_id: first.id.clone(),
                label: "  supports  ".into(),
            },
        )
        .unwrap();
        assert_eq!(saved.label, "supports");
        assert!(saved.source_resource_id < saved.target_resource_id);
        assert_eq!(
            resource_relationship_list_on_conn(&conn, &profile_id).unwrap(),
            vec![saved.clone()]
        );
        assert!(resource_relationship_list_on_conn(&conn, &other_profile_id)
            .unwrap()
            .is_empty());

        let duplicate = ResourceRelationship {
            id: Uuid::new_v4().to_string(),
            profile_id: profile_id.clone(),
            source_resource_id: first.id.clone(),
            target_resource_id: second.id.clone(),
            label: "duplicate pair".into(),
        };
        assert!(resource_relationship_save_on_conn(&mut conn, duplicate).is_err());
        let mut updated = saved.clone();
        updated.label = "documents".into();
        assert_eq!(
            resource_relationship_save_on_conn(&mut conn, updated)
                .unwrap()
                .label,
            "documents"
        );
        assert_eq!(
            resource_relationship_list_on_conn(&conn, &profile_id)
                .unwrap()
                .len(),
            1
        );

        assert!(resource_relationship_save_on_conn(
            &mut conn,
            ResourceRelationship {
                id: Uuid::new_v4().to_string(),
                profile_id: profile_id.clone(),
                source_resource_id: first.id.clone(),
                target_resource_id: first.id.clone(),
                label: "self".into(),
            }
        )
        .is_err());
        let outsider = resource("Outside");
        persist_resource(&mut conn, &outsider, true, Some(&other_profile_id)).unwrap();
        assert!(resource_relationship_save_on_conn(
            &mut conn,
            ResourceRelationship {
                id: Uuid::new_v4().to_string(),
                profile_id: profile_id.clone(),
                source_resource_id: first.id.clone(),
                target_resource_id: outsider.id.clone(),
                label: "cross profile".into(),
            }
        )
        .is_err());
        resource_relationship_delete_on_conn(&conn, &profile_id, &relationship_id).unwrap();
        assert!(resource_relationship_list_on_conn(&conn, &profile_id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn workspace_snapshots_restore_layouts_without_removing_new_cards_or_memberships() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let saved_resource = resource("At checkpoint");
        let later_resource = resource("Added later");
        persist_resource(&mut conn, &saved_resource, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &later_resource, true, Some(&profile_id)).unwrap();
        let checkpoint_layout = DashboardLayout {
            profile_id: profile_id.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: saved_resource.id.clone(),
                x: 2,
                y: 3,
                w: 2,
                h: 1,
                card_type: "compact".into(),
            }],
        };
        layout_save_to_conn(&mut conn, &checkpoint_layout).unwrap();
        let snapshot =
            workspace_snapshot_create_on_conn(&mut conn, &profile_id, "  Planning  ").unwrap();
        assert_eq!(snapshot.name, "Planning");
        assert_eq!(snapshot.resource_ids.len(), 2);
        assert_eq!(snapshot.dashboard.cards[0].x, 2);

        let changed_layout = DashboardLayout {
            profile_id: profile_id.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: later_resource.id.clone(),
                x: 20,
                y: 20,
                w: 1,
                h: 1,
                card_type: "icon".into(),
            }],
        };
        layout_save_to_conn(&mut conn, &changed_layout).unwrap();
        let restored =
            workspace_snapshot_restore_on_conn(&mut conn, &profile_id, &snapshot.id).unwrap();
        assert_eq!(restored.restored_layouts, 2);
        assert!(restored.missing_resource_ids.is_empty());
        let layout = layout_get_from_conn(&conn, &profile_id, "dashboard").unwrap();
        assert_eq!(
            layout.cards.len(),
            2,
            "restore retains resources added after checkpoint"
        );
        let restored_card = layout
            .cards
            .iter()
            .find(|card| card.resource_id == saved_resource.id)
            .unwrap();
        assert_eq!(
            restored_card.x, 2,
            "snapshot positions win for resources it captured"
        );
        assert!(layout
            .cards
            .iter()
            .any(|card| card.resource_id == later_resource.id));
        let membership_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1",
                [&profile_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(membership_count, 2);
    }

    #[test]
    fn workspace_snapshot_restore_reports_removed_profile_resources() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let resource = resource("Will leave profile");
        persist_resource(&mut conn, &resource, true, Some(&profile_id)).unwrap();
        let layout = DashboardLayout {
            profile_id: profile_id.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: resource.id.clone(),
                x: 0,
                y: 0,
                w: 1,
                h: 1,
                card_type: "icon".into(),
            }],
        };
        layout_save_to_conn(&mut conn, &layout).unwrap();
        let snapshot =
            workspace_snapshot_create_on_conn(&mut conn, &profile_id, "Before removal").unwrap();
        conn.execute(
            "DELETE FROM profile_resources WHERE profile_id=?1 AND resource_id=?2",
            params![profile_id, resource.id],
        )
        .unwrap();
        let restored =
            workspace_snapshot_restore_on_conn(&mut conn, &profile_id, &snapshot.id).unwrap();
        assert_eq!(restored.missing_resource_ids, vec![resource.id]);
        assert!(layout_get_from_conn(&conn, &profile_id, "dashboard")
            .unwrap()
            .cards
            .is_empty());
    }

    #[test]
    fn invalid_layout_save_does_not_overwrite_prior_row() {
        let mut conn = database();
        let pid = profile(&conn);
        let resource = resource("Known");
        persist_resource(&mut conn, &resource, true, Some(&pid)).unwrap();
        let prior = DashboardLayout {
            profile_id: pid.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: resource.id.clone(),
                x: 0,
                y: 0,
                w: 2,
                h: 2,
                card_type: "icon".into(),
            }],
        };
        layout_save_to_conn(&mut conn, &prior).unwrap();
        let invalid = DashboardLayout {
            cards: vec![LayoutCard {
                resource_id: Uuid::new_v4().to_string(),
                x: 1,
                y: 3,
                w: 1,
                h: 1,
                card_type: "icon".into(),
            }],
            ..prior.clone()
        };
        assert!(layout_save_to_conn(&mut conn, &invalid).is_err());
        let invalid_profile = DashboardLayout {
            profile_id: "not-a-uuid".into(),
            ..prior.clone()
        };
        assert!(layout_save_to_conn(&mut conn, &invalid_profile).is_err());
        let stored: String = conn
            .query_row(
                "SELECT layout_json FROM layouts WHERE profile_id=?1 AND surface='dashboard'",
                [&pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, serde_json::to_string(&prior).unwrap());
    }

    #[test]
    fn layout_records_are_isolated_by_surface_and_profile() {
        let mut conn = database();
        let profile_a = profile(&conn);
        let profile_b = profile(&conn);
        let resource = resource("Shared resource");
        persist_resource(&mut conn, &resource, true, Some(&profile_a)).unwrap();
        conn.execute(
            "INSERT INTO profile_resources(profile_id,resource_id) VALUES (?1,?2)",
            params![profile_b, resource.id],
        )
        .unwrap();
        let card = LayoutCard {
            resource_id: resource.id.clone(),
            x: 0,
            y: 0,
            w: 2,
            h: 2,
            card_type: "icon".into(),
        };
        let a_dashboard = DashboardLayout {
            profile_id: profile_a.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![card.clone()],
        };
        let a_dock = DashboardLayout {
            profile_id: profile_a.clone(),
            surface: "dock".into(),
            version: 1,
            cards: vec![LayoutCard {
                y: 7,
                ..card.clone()
            }],
        };
        let b_dashboard = DashboardLayout {
            profile_id: profile_b.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard { y: 9, ..card }],
        };
        layout_save_to_conn(&mut conn, &a_dashboard).unwrap();
        layout_save_to_conn(&mut conn, &a_dock).unwrap();
        layout_save_to_conn(&mut conn, &b_dashboard).unwrap();
        assert_eq!(
            layout_get_from_conn(&conn, &profile_a, "dashboard")
                .unwrap()
                .cards[0]
                .y,
            0
        );
        assert_eq!(
            layout_get_from_conn(&conn, &profile_a, "dock")
                .unwrap()
                .cards[0]
                .y,
            7
        );
        assert_eq!(
            layout_get_from_conn(&conn, &profile_b, "dashboard")
                .unwrap()
                .cards[0]
                .y,
            9
        );
    }

    #[test]
    fn layout_rejects_valid_resource_owned_only_by_another_profile() {
        let mut conn = database();
        let owner = profile(&conn);
        let other = profile(&conn);
        let owned = resource("Owned elsewhere");
        persist_resource(&mut conn, &owned, true, Some(&owner)).unwrap();
        let prior = DashboardLayout {
            profile_id: other.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![],
        };
        layout_save_to_conn(&mut conn, &prior).unwrap();
        let invalid = DashboardLayout {
            cards: vec![LayoutCard {
                resource_id: owned.id.clone(),
                x: 0,
                y: 0,
                w: 1,
                h: 1,
                card_type: "icon".into(),
            }],
            ..prior.clone()
        };
        assert!(layout_save_to_conn(&mut conn, &invalid).is_err());
        let stored: String = conn
            .query_row(
                "SELECT layout_json FROM layouts WHERE profile_id=?1 AND surface='dashboard'",
                [&other],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, serde_json::to_string(&prior).unwrap());
    }

    #[test]
    fn layout_rejects_card_count_and_serialized_byte_limits() {
        let pid = Uuid::new_v4().to_string();
        let too_many = DashboardLayout {
            profile_id: pid.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: (0..=10_000)
                .map(|_| LayoutCard {
                    resource_id: Uuid::new_v4().to_string(),
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                    card_type: "icon".into(),
                })
                .collect(),
        };
        assert!(validate_layout(&too_many).is_err());

        let mut conn = database();
        let real_profile = profile(&conn);
        let prior = DashboardLayout {
            profile_id: real_profile.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![],
        };
        layout_save_to_conn(&mut conn, &prior).unwrap();
        let oversized = DashboardLayout {
            profile_id: real_profile.clone(),
            surface: "dashboard".into(),
            version: 1,
            cards: vec![LayoutCard {
                resource_id: Uuid::new_v4().to_string(),
                x: 0,
                y: 0,
                w: 1,
                h: 1,
                card_type: "x".repeat(MAX_LAYOUT_BYTES),
            }],
        };
        assert!(serde_json::to_vec(&oversized).unwrap().len() > MAX_LAYOUT_BYTES);
        assert!(layout_save_to_conn(&mut conn, &oversized).is_err());
        let stored: String = conn
            .query_row(
                "SELECT layout_json FROM layouts WHERE profile_id=?1 AND surface='dashboard'",
                [&real_profile],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, serde_json::to_string(&prior).unwrap());
    }

    #[test]
    fn layout_requires_uuid_known_profile_and_matching_row_identity() {
        let conn = database();
        let pid = profile(&conn);
        assert!(layout_get_from_conn(&conn, "not-a-uuid", "dashboard").is_err());
        assert!(matches!(
            layout_get_from_conn(&conn, &Uuid::new_v4().to_string(), "dashboard"),
            Err(AppError::NotFound)
        ));
        assert!(layout_get_from_conn(&conn, &pid, "window").is_err());
        let other = profile(&conn);
        let wrong_identity = DashboardLayout {
            profile_id: other,
            surface: "dashboard".into(),
            version: 1,
            cards: vec![],
        };
        conn.execute("INSERT INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,'dashboard',1,?2,?3)", params![pid,serde_json::to_string(&wrong_identity).unwrap(),now_utc()]).unwrap();
        assert!(layout_get_from_conn(&conn, &pid, "dashboard").is_err());
    }

    #[test]
    fn layout_checks_card_uuid_surface_and_row_version() {
        let conn = database();
        let pid = profile(&conn);
        let mut invalid = DashboardLayout {
            profile_id: pid.clone(),
            surface: "window".into(),
            version: 1,
            cards: vec![],
        };
        assert!(validate_layout(&invalid).is_err());
        invalid.surface = "dock".into();
        invalid.cards.push(LayoutCard {
            resource_id: "invalid-id".into(),
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            card_type: "icon".into(),
        });
        assert!(validate_layout(&invalid).is_err());
        let valid = DashboardLayout {
            profile_id: pid.clone(),
            surface: "dock".into(),
            version: 1,
            cards: vec![],
        };
        conn.execute("INSERT INTO layouts(profile_id,surface,version,layout_json,updated_utc) VALUES (?1,'dock',2,?2,?3)", params![pid,serde_json::to_string(&valid).unwrap(),now_utc()]).unwrap();
        assert!(layout_get_from_conn(&conn, &valid.profile_id, "dock").is_err());
        let surface_mismatch = DashboardLayout {
            surface: "dashboard".into(),
            ..valid.clone()
        };
        conn.execute(
            "UPDATE layouts SET version=1,layout_json=?2 WHERE profile_id=?1 AND surface='dock'",
            params![pid, serde_json::to_string(&surface_mismatch).unwrap()],
        )
        .unwrap();
        assert!(layout_get_from_conn(&conn, &valid.profile_id, "dock").is_err());
    }

    #[test]
    fn quick_launch_hotkey_uses_alt_space_and_only_toggles_on_press() {
        let shortcut: Shortcut = QUICK_LAUNCH_HOTKEY_SPEC.parse().unwrap();
        assert!(shortcut.matches(Modifiers::ALT, Code::Space));
        let pressed = ShortcutEvent {
            id: shortcut.id(),
            state: ShortcutState::Pressed,
        };
        let released = ShortcutEvent {
            id: shortcut.id(),
            state: ShortcutState::Released,
        };
        assert!(quick_launch_toggle_pressed(&shortcut, &pressed));
        assert!(!quick_launch_toggle_pressed(&shortcut, &released));
        let other = Shortcut::new(Some(Modifiers::ALT), Code::KeyA);
        assert!(quick_launch_toggle_pressed(&other, &pressed)); // Registered shortcut identity is owned by plugin dispatch.
    }

    #[test]
    fn quick_launch_hotkey_status_reports_success_and_conflict() {
        let success = switch_hotkey("alt+space", None, |_| Ok(()), |_| Ok(()));
        assert!(success.registered);
        assert_eq!(success.shortcut, "alt+space");
        assert_eq!(success.active_shortcut.as_deref(), Some("alt+space"));
        assert_eq!(success.error_code, None);
        let conflict = switch_hotkey(
            "ctrl+space",
            Some("alt+space"),
            |_| Err("hotkey already registered by another app".into()),
            |_| Ok(()),
        );
        assert!(!conflict.registered);
        assert_eq!(conflict.shortcut, "ctrl+space");
        assert_eq!(conflict.active_shortcut.as_deref(), Some("alt+space"));
        assert_eq!(conflict.error_code.as_deref(), Some("hotkey_conflict"));
    }

    #[test]
    fn hotkey_conflict_preserves_profile_settings_and_active_registration() {
        let mut conn = database();
        let pid = profile(&conn);
        conn.execute("UPDATE profiles SET settings_json='{\"theme\":\"dark\",\"quickLaunchShortcut\":\"alt+space\"}' WHERE id=?1", [&pid]).unwrap();
        let mut register_calls = Vec::new();
        let mut unregister_calls = Vec::new();
        let status = set_profile_shortcut_with(
            &mut conn,
            &pid,
            "ctrl+space",
            Some("alt+space"),
            |shortcut| {
                register_calls.push(shortcut.to_owned());
                Err("already registered".into())
            },
            |shortcut| {
                unregister_calls.push(shortcut.to_owned());
                Ok(())
            },
        )
        .unwrap();
        assert!(!status.registered);
        assert_eq!(status.active_shortcut.as_deref(), Some("alt+space"));
        assert_eq!(register_calls, vec!["ctrl+space"]);
        assert!(unregister_calls.is_empty());
        let settings: String = conn
            .query_row(
                "SELECT settings_json FROM profiles WHERE id=?1",
                [&pid],
                |row| row.get(0),
            )
            .unwrap();
        let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(settings[QUICK_LAUNCH_SETTING_KEY], "alt+space");
        assert_eq!(settings["theme"], "dark");
    }

    #[test]
    fn inactive_profile_hotkey_is_persisted_without_registration() {
        let conn = database();
        let pid = profile(&conn);
        assert_eq!(
            profile_shortcut(&conn, &pid).unwrap(),
            QUICK_LAUNCH_HOTKEY_SPEC
        );
        let status =
            save_inactive_profile_shortcut(&conn, &pid, "ctrl+space", Some("alt+space".into()))
                .unwrap();
        assert_eq!(profile_shortcut(&conn, &pid).unwrap(), "ctrl+space");
        assert!(!status.registered);
        assert_eq!(status.shortcut, "ctrl+space");
        assert_eq!(status.active_shortcut.as_deref(), Some("alt+space"));
        assert_eq!(status.error_code, None);
    }

    #[test]
    fn profile_shortcut_swap_registers_before_unregistering_old() {
        let calls = std::cell::RefCell::new(Vec::new());
        let status = switch_hotkey(
            "ctrl+space",
            Some("alt+space"),
            |value| {
                calls.borrow_mut().push(format!("register:{value}"));
                Ok(())
            },
            |value| {
                calls.borrow_mut().push(format!("unregister:{value}"));
                Ok(())
            },
        );
        assert!(status.registered);
        assert_eq!(status.active_shortcut.as_deref(), Some("ctrl+space"));
        assert_eq!(
            *calls.borrow(),
            vec!["register:ctrl+space", "unregister:alt+space"]
        );
    }

    #[test]
    fn profile_switch_keeps_selection_when_shortcut_conflicts() {
        let active_profile = ActiveProfileState(Mutex::new("old-profile".into()));
        let conflict = switch_hotkey(
            "ctrl+space",
            Some("alt+space"),
            |_| Err("already registered".into()),
            |_| Ok(()),
        );
        record_active_profile(&active_profile, "requested-profile");
        assert_eq!(*active_profile.0.lock().unwrap(), "requested-profile");
        assert!(!conflict.registered);
        assert_eq!(conflict.active_shortcut.as_deref(), Some("alt+space"));
        assert_eq!(conflict.error_code.as_deref(), Some("hotkey_conflict"));
    }

    #[test]
    fn migration_is_repeatable_and_versioned() {
        let conn = database();
        migrate_database(&conn).unwrap();
        migrate_database(&conn).unwrap();
        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 10);
        let table_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='smart_collections'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(table_exists, 1);
        let shortcuts_exist: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='dynamic_shortcuts'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(shortcuts_exist, 1);
        let relationship_table: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='resource_relationships'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(relationship_table, 1);
        let snapshots_table: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='workspace_snapshots'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(snapshots_table, 1);
        let default_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profiles WHERE name='Default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(default_count, 1);
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(n >= 7);
        let metadata_table: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='metadata_cache'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(metadata_table, 1);
    }

    #[test]
    fn migration_upgrades_legacy_profile_and_launch_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_utc TEXT NOT NULL); CREATE TABLE resources(id TEXT PRIMARY KEY,resource_type TEXT NOT NULL,display_name TEXT NOT NULL,description TEXT,icon TEXT,default_action TEXT NOT NULL,created_utc TEXT NOT NULL,updated_utc TEXT NOT NULL); CREATE TABLE resource_targets(id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,kind TEXT NOT NULL,canonical_uri TEXT NOT NULL,priority INTEGER NOT NULL DEFAULT 0,arguments_json TEXT NOT NULL DEFAULT '[]',working_directory TEXT,active INTEGER NOT NULL DEFAULT 1); CREATE TABLE profiles(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,theme_id TEXT NOT NULL DEFAULT 'system',settings_json TEXT NOT NULL DEFAULT '{}'); CREATE TABLE profile_resources(profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,favorite INTEGER NOT NULL DEFAULT 0,sort_position INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(profile_id,resource_id)); CREATE TABLE launch_events(id TEXT PRIMARY KEY,request_id TEXT NOT NULL UNIQUE,resource_id TEXT NOT NULL,target_id TEXT,profile_id TEXT,source TEXT NOT NULL,launched_at_utc TEXT NOT NULL,status TEXT NOT NULL,error_code TEXT);") .unwrap();
        conn.execute_batch("CREATE TABLE metadata_cache(resource_id TEXT NOT NULL,target_id TEXT NOT NULL,inspected_utc TEXT NOT NULL,file_size_bytes INTEGER,created_utc TEXT,modified_utc TEXT,availability TEXT NOT NULL,provenance TEXT NOT NULL,error_code TEXT,PRIMARY KEY(resource_id,target_id));").unwrap();
        migrate_database(&conn).unwrap();
        let created: String = conn
            .query_row(
                "SELECT created_utc FROM profiles WHERE name='Default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!created.is_empty());
        let cache_status_column: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('metadata_cache') WHERE name='cache_status'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cache_status_column, 1);
        let message_column: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('launch_events') WHERE name='message'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(message_column, 1);
    }

    fn local_path_resource(name: &str, kind: &str, path: &str) -> Resource {
        let mut value = resource(name);
        value.resource_type = kind.into();
        value.targets[0].kind = kind.into();
        value.targets[0].canonical_uri = path.into();
        value
    }

    #[test]
    fn resource_duplicate_is_profile_scoped_and_regenerates_ids_without_history() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let other_profile_id = profile(&conn);
        let source = resource("Source");
        persist_resource(&mut conn, &source, true, Some(&profile_id)).unwrap();
        conn.execute(
            "UPDATE profile_resources SET favorite=1 WHERE profile_id=?1 AND resource_id=?2",
            params![profile_id, source.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status) VALUES (?1,?2,?3,?4,?5,'dashboard',?6,'handed_off')",
            params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), source.id, source.targets[0].id, profile_id, now_utc()],
        )
        .unwrap();

        let duplicate = resource_duplicate_on_conn(
            &mut conn,
            &ResourceDuplicateRequest {
                profile_id: profile_id.clone(),
                resource_id: source.id.clone(),
                display_name: None,
            },
        )
        .unwrap();
        assert_ne!(duplicate.id, source.id);
        assert_ne!(duplicate.targets[0].id, source.targets[0].id);
        assert_eq!(duplicate.display_name, "Source (copy)");
        assert_eq!(duplicate.favorite, true);
        assert_eq!(duplicate.launch_count, 0);
        assert_eq!(duplicate.last_launched_utc, None);
        assert_eq!(
            duplicate.targets[0].canonical_uri,
            source.targets[0].canonical_uri
        );
        assert_eq!(duplicate.tags, source.tags);
        assert_eq!(duplicate.aliases, source.aliases);
        let memberships: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM profile_resources WHERE profile_id=?1 AND resource_id IN (?2,?3)",
                params![profile_id, source.id, duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(memberships, 2);
        assert!(resource_duplicate_on_conn(
            &mut conn,
            &ResourceDuplicateRequest {
                profile_id: other_profile_id,
                resource_id: source.id,
                display_name: Some("A copy".into()),
            }
        )
        .is_err());
    }

    #[test]
    fn relocation_preview_replaces_only_boundary_matched_file_and_folder_targets_in_profile() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let other_profile_id = profile(&conn);
        let file = local_path_resource("File", "file", "C:\\Work\\Project\\plan.xlsx");
        let folder = local_path_resource("Folder", "folder", "c:/work/project/assets/");
        let sibling = local_path_resource("Sibling", "file", "C:\\Work\\Project-old\\plan.xlsx");
        let app = local_path_resource("Application", "app", "C:\\Work\\Project\\tool.exe");
        let other_profile =
            local_path_resource("Other profile", "file", "C:\\Work\\Project\\secret.txt");
        for item in [&file, &folder, &sibling, &app] {
            persist_resource(&mut conn, item, true, Some(&profile_id)).unwrap();
        }
        persist_resource(&mut conn, &other_profile, true, Some(&other_profile_id)).unwrap();
        conn.execute(
            "UPDATE resources SET updated_utc='before-relocation' WHERE id IN (?1,?2)",
            params![file.id, folder.id],
        )
        .unwrap();

        let preview = resource_relocation_preview_on_conn(
            &conn,
            &ResourceRelocationPreviewRequest {
                profile_id: profile_id.clone(),
                source_prefix: "c:/work/project/".into(),
                destination_prefix: "D:/Archive/Project/".into(),
                resource_ids: None,
            },
        )
        .unwrap();
        assert_eq!(preview.changes.len(), 2);
        assert!(preview.changes.iter().any(|change| {
            change.resource_id == file.id && change.to_uri == "D:\\Archive\\Project\\plan.xlsx"
        }));
        assert!(preview.changes.iter().any(|change| {
            change.resource_id == folder.id && change.to_uri == "D:\\Archive\\Project\\assets"
        }));
        assert!(!preview.changes.iter().any(|change| {
            change.resource_id == sibling.id
                || change.resource_id == app.id
                || change.resource_id == other_profile.id
        }));

        let applied = resource_relocation_commit_on_conn(&mut conn, &preview).unwrap();
        assert_eq!(applied.updated_target_count, 2);
        assert_eq!(applied.updated_resource_ids.len(), 2);
        let timestamps = applied
            .updated_resource_ids
            .iter()
            .map(|resource_id| {
                conn.query_row(
                    "SELECT updated_utc FROM resources WHERE id=?1",
                    [resource_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(timestamps.len(), 2);
        assert!(timestamps
            .iter()
            .all(|timestamp| timestamp != "before-relocation"));
        assert_eq!(timestamps[0], timestamps[1]);
        let stored: String = conn
            .query_row(
                "SELECT canonical_uri FROM resource_targets WHERE id=?1",
                [&file.targets[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, "D:\\Archive\\Project\\plan.xlsx");
        // All paths are nonexistent fixture strings: relocation only updates SQLite.
        assert!(!std::path::Path::new("D:\\Archive\\Project\\plan.xlsx").exists());
    }

    #[test]
    fn relocation_rejects_invalid_prefixes_scope_and_duplicate_selection() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let other_profile_id = profile(&conn);
        let file = local_path_resource("File", "file", "C:\\Work\\Project\\plan.xlsx");
        persist_resource(&mut conn, &file, true, Some(&profile_id)).unwrap();
        let other = local_path_resource("Other", "file", "C:\\Work\\Project\\other.xlsx");
        persist_resource(&mut conn, &other, true, Some(&other_profile_id)).unwrap();

        let request = |source: &str, destination: &str, ids: Option<Vec<String>>| {
            ResourceRelocationPreviewRequest {
                profile_id: profile_id.clone(),
                source_prefix: source.into(),
                destination_prefix: destination.into(),
                resource_ids: ids,
            }
        };
        assert!(
            resource_relocation_preview_on_conn(&conn, &request("relative", "D:\\New", None))
                .is_err()
        );
        assert!(resource_relocation_preview_on_conn(
            &conn,
            &request("C:\\Work\\..\\", "D:\\New", None)
        )
        .is_err());
        assert!(resource_relocation_preview_on_conn(
            &conn,
            &request("C:\\Work", "C:\\Work\\New", None)
        )
        .is_err());
        assert!(resource_relocation_preview_on_conn(
            &conn,
            &request("C:\\Work", "D:\\New", Some(vec![other.id.clone()])),
        )
        .is_err());
        assert!(resource_relocation_preview_on_conn(
            &conn,
            &request(
                "C:\\Work",
                "D:\\New",
                Some(vec![file.id.clone(), file.id.clone()])
            ),
        )
        .is_err());
    }

    #[test]
    fn relocation_preview_is_bound_to_state_and_rolls_back_all_target_updates() {
        let mut conn = database();
        let profile_id = profile(&conn);
        let first = local_path_resource("First", "file", "C:\\Work\\Project\\first.txt");
        let second = local_path_resource("Second", "file", "C:\\Work\\Project\\second.txt");
        persist_resource(&mut conn, &first, true, Some(&profile_id)).unwrap();
        persist_resource(&mut conn, &second, true, Some(&profile_id)).unwrap();
        let request = ResourceRelocationPreviewRequest {
            profile_id: profile_id.clone(),
            source_prefix: "C:\\Work\\Project".into(),
            destination_prefix: "D:\\Relocated".into(),
            resource_ids: None,
        };
        let preview = resource_relocation_preview_on_conn(&conn, &request).unwrap();

        conn.execute(
            "UPDATE resource_targets SET canonical_uri='C:\\Elsewhere\\first.txt' WHERE id=?1",
            [&first.targets[0].id],
        )
        .unwrap();
        assert!(resource_relocation_commit_on_conn(&mut conn, &preview).is_err());
        let unchanged: String = conn
            .query_row(
                "SELECT canonical_uri FROM resource_targets WHERE id=?1",
                [&second.targets[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, "C:\\Work\\Project\\second.txt");

        let fresh = resource_relocation_preview_on_conn(&conn, &request).unwrap();
        conn.execute_batch(&format!(
            "CREATE TRIGGER reject_relocation BEFORE UPDATE OF canonical_uri ON resource_targets WHEN OLD.id='{}' BEGIN SELECT RAISE(ABORT,'injected relocation failure'); END;",
            second.targets[0].id
        ))
        .unwrap();
        assert!(resource_relocation_commit_on_conn(&mut conn, &fresh).is_err());
        for (target_id, path) in [
            (&first.targets[0].id, "C:\\Elsewhere\\first.txt"),
            (&second.targets[0].id, "C:\\Work\\Project\\second.txt"),
        ] {
            let stored: String = conn
                .query_row(
                    "SELECT canonical_uri FROM resource_targets WHERE id=?1",
                    [target_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                stored, path,
                "failed relocation rolls back every target update"
            );
        }
    }

    fn sample_target() -> ResourceTarget {
        ResourceTarget {
            id: Uuid::new_v4().to_string(),
            kind: "file".into(),
            canonical_uri: "C:\\example.txt".into(),
            priority: 0,
            arguments: vec![],
            working_directory: None,
            active: true,
        }
    }
}
