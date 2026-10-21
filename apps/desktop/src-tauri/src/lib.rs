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
    pub resource_count: usize,
    pub layout_count: usize,
    pub history_event_count: usize,
    pub profile_id_collision: bool,
    pub profile_name_collision: bool,
    pub resource_id_collisions: Vec<String>,
    pub target_id_collisions: Vec<String>,
    pub resource_conflicts: Vec<ProfileResourceConflictPreview>,
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
                || !["dashboard", "quick_launch", "dock", "explorer", "other"]
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
) -> Result<ProfileImportPreview, AppError> {
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
    let mut preview = ProfileImportPreview {
        package_sha256: package_sha256.into(),
        preview_token: String::new(),
        profile_id: package.profile.id.clone(),
        profile_name: package.profile.name.clone(),
        resource_count: package.resources.resources.len(),
        layout_count: package.layouts.layouts.len(),
        history_event_count: package
            .history
            .as_ref()
            .map_or(0, |history| history.events.len()),
        profile_id_collision,
        profile_name_collision,
        resource_id_collisions,
        target_id_collisions,
        resource_conflicts,
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
    let mut imported_history_events = 0;
    if let Some(history) = &package.history {
        for event in &history.events {
            let Some(resource_id) = resource_map.get(&event.resource_id) else {
                continue;
            };
            let target_id_mapped = event
                .target_id
                .as_ref()
                .and_then(|target_id| target_map.get(target_id));
            tx.execute("INSERT INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status,error_code,message,duration_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), resource_id, target_id_mapped, target_id, event.source, event.launched_at_utc, event.status, event.error_code, event.message, event.duration_ms])?;
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
    let (package, package_sha256) = read_profile_package_with_digest(package_path)?;
    if package_sha256 != expected_package_sha256 {
        return Err(package_error(
            "profile package changed after preview; preview it again",
        ));
    }
    let preview = profile_import_preview_from_conn(conn, &package, &package_sha256)?;
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
    let stats = conn.query_row(
        "SELECT COUNT(*),MAX(launched_at_utc) FROM launch_events WHERE resource_id=?1",
        [&resource.id],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
    )?;
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

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}

fn validate_layout(layout: &DashboardLayout) -> Result<(), AppError> {
    if !valid_uuid(&layout.profile_id) {
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
        if !valid_uuid(&card.resource_id)
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
    if !valid_uuid(profile_id) {
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
) -> Result<ProfileImportPreview, AppError> {
    let (package, package_sha256) = read_profile_package_with_digest(Path::new(&package_path))?;
    let conn = lock(&state)?;
    profile_import_preview_from_conn(&conn, &package, &package_sha256)
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
    }
    if !["dashboard", "quick_launch", "dock", "explorer", "other"]
        .contains(&request.source.as_str())
    {
        return Err(AppError::Invalid("unsupported launch source".into()));
    }
    if let Some(existing) = conn
        .query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE request_id=?1", [&request.request_id], launch_event_from_row)
        .optional()?
    {
        if existing.resource_id != request.resource_id
            || existing.profile_id != request.profile_id
            || existing.source != request.source
        {
            return Err(AppError::Invalid("launch request ID was already used for a different action".into()));
        }
        let resolved_target: Option<String> = conn
            .query_row(
                "SELECT id FROM resource_targets WHERE resource_id=?1 AND active=1 AND (?2 IS NULL OR id=?2) ORDER BY priority DESC LIMIT 1",
                params![request.resource_id, request.target_id],
                |row| row.get(0),
            )
            .optional()?;
        if existing.target_id != resolved_target {
            return Err(AppError::Invalid("launch request ID was already used for a different target".into()));
        }
        return Ok(existing);
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
    let event_id = Uuid::new_v4().to_string();
    // Caller holds the DB mutex across reservation and handoff, preventing duplicate OS calls.
    conn.execute("INSERT INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status) VALUES (?1,?2,?3,?4,?5,?6,?7,'requested')",params![event_id,request.request_id,request.resource_id,target_id,request.profile_id,request.source,launched_at_utc])?;
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
    conn.execute(
        "UPDATE launch_events SET status=?2,error_code=?3,message=?4,duration_ms=?5 WHERE id=?1",
        params![event_id, status, error_code, message, duration_ms],
    )?;
    Ok(conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE id=?1",[event_id],launch_event_from_row)?)
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
    profile_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<LaunchEvent>, AppError> {
    if let Some(pid) = profile_id.as_ref() {
        Uuid::parse_str(pid).map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    }
    let conn = lock(&state)?;
    let mut stmt=conn.prepare("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE (?1 IS NULL OR profile_id=?1) ORDER BY launched_at_utc DESC LIMIT ?2")?;
    let events = stmt
        .query_map(
            params![profile_id, limit.unwrap_or(100).clamp(1, 1000) as i64],
            launch_event_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
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
    profile_id: Option<String>,
    since_utc: Option<String>,
) -> Result<AnalyticsSummary, AppError> {
    if let Some(pid) = profile_id.as_ref() {
        Uuid::parse_str(pid).map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    }
    let since_utc = since_utc
        .map(|value| {
            if value.len() > 64 {
                return Err(AppError::Invalid("sinceUtc is too long".into()));
            }
            chrono::DateTime::parse_from_rfc3339(&value)
                .map(|parsed| {
                    parsed
                        .with_timezone(&Utc)
                        .to_rfc3339_opts(SecondsFormat::Millis, true)
                })
                .map_err(|_| AppError::Invalid("sinceUtc must be an RFC 3339 timestamp".into()))
        })
        .transpose()?;
    let conn = lock(&state)?;
    analytics_summary_for(&conn, profile_id.as_deref(), since_utc.as_deref())
}

fn analytics_summary_for(
    conn: &Connection,
    profile_id: Option<&str>,
    since_utc: Option<&str>,
) -> Result<AnalyticsSummary, AppError> {
    let mut stmt = conn.prepare("SELECT source,status,COUNT(*),MIN(launched_at_utc),MAX(launched_at_utc) FROM launch_events WHERE (?1 IS NULL OR profile_id=?1) AND (?2 IS NULL OR launched_at_utc>=?2) GROUP BY source,status ORDER BY source,status")?;
    let mut summary = AnalyticsSummary {
        total_launches: 0,
        by_source: BTreeMap::new(),
        by_status: BTreeMap::new(),
        first_event_utc: None,
        last_event_utc: None,
    };
    for row in stmt.query_map(params![profile_id, since_utc], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, u64>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })? {
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
            resource_delete,
            resource_search,
            resource_launch,
            resource_inspect,
            profile_list,
            profile_create,
            profile_theme_set,
            profile_export,
            profile_import_preview,
            profile_import_commit,
            profile_switch,
            layout_get,
            layout_save,
            quick_launch_hotkey_status,
            quick_launch_hotkey_set,
            analytics_query,
            analytics_summary
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
            profile_import_preview_from_conn(&conn, &package, "a".repeat(64).as_str()).unwrap();
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
        let preview = profile_import_preview_from_conn(&conn, &parsed, &digest).unwrap();

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
            profile_import_preview_from_conn(&conn, &parsed, &digest).unwrap();
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
        let summary = analytics_summary_for(&conn, Some(&pid), None).unwrap();
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
        let after =
            analytics_summary_for(&conn, Some(&pid), Some("2026-10-20T00:00:00.000Z")).unwrap();
        assert_eq!(after.total_launches, 1);
        assert_eq!(after.by_source.get("quick_launch"), Some(&1));
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
        assert_eq!(version, 4);
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
