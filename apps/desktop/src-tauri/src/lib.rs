use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

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

pub struct AppState(pub Mutex<Connection>);

pub struct HotkeyStatusState(pub Mutex<QuickLaunchHotkeyStatus>);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickLaunchHotkeyStatus {
    pub registered: bool,
    pub shortcut: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

const QUICK_LAUNCH_HOTKEY_SPEC: &str = "ALT+SPACE";
const QUICK_LAUNCH_HOTKEY_LABEL: &str = "Alt+Space";

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

fn quick_launch_toggle_pressed(shortcut: &Shortcut, event: &ShortcutEvent) -> bool {
    shortcut.matches(Modifiers::ALT, Code::Space) && event.state() == ShortcutState::Pressed
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

fn hotkey_status(result: Result<(), String>) -> QuickLaunchHotkeyStatus {
    match result {
        Ok(()) => QuickLaunchHotkeyStatus {
            registered: true,
            shortcut: QUICK_LAUNCH_HOTKEY_LABEL.into(),
            error_code: None,
        },
        Err(message) => QuickLaunchHotkeyStatus {
            registered: false,
            shortcut: QUICK_LAUNCH_HOTKEY_LABEL.into(),
            error_code: Some(hotkey_error_code(&message).into()),
        },
    }
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
    load_targets(conn, resource)?;
    let mut stmt=conn.prepare("SELECT kind,value FROM resource_terms WHERE resource_id=?1 ORDER BY kind,value COLLATE NOCASE")?;
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
    if resource.display_name.trim().is_empty() {
        return Err(AppError::Invalid("display name is required".into()));
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
    match target.kind.as_str() {
        "url" => {
            let parsed = Url::parse(&target.canonical_uri)
                .map_err(|_| AppError::Invalid("URL is invalid".into()))?;
            if parsed.scheme() != "https" {
                return Err(AppError::Invalid("only HTTPS URLs may be opened".into()));
            }
            if !target.arguments.is_empty() {
                return Err(AppError::Invalid(
                    "arguments are only allowed for app targets".into(),
                ));
            }
        }
        "file" | "folder" | "app" | "shortcut" => {
            let path = target.canonical_uri.trim();
            let drive_path = path.as_bytes().get(1) == Some(&b':')
                && path
                    .as_bytes()
                    .get(2)
                    .is_some_and(|b| *b == b'\\' || *b == b'/');
            let unc_path = path.starts_with("\\\\") && path.len() > 2;
            if !(drive_path || unc_path) || path.starts_with("\\\\?\\") {
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
    if target.arguments.iter().any(|a| a.contains('\0')) {
        return Err(AppError::Invalid("arguments cannot contain NUL".into()));
    }
    Ok(())
}

fn persist_resource(
    conn: &mut Connection,
    resource: &Resource,
    creating: bool,
    profile_id: Option<&str>,
) -> Result<(), AppError> {
    validate_resource(resource)?;
    let tx = conn.transaction()?;
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
    tx.commit()?;
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
    Uuid::parse_str(&id).map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    let conn = lock(&state)?;
    let mut resource=conn.query_row("SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",[&id],resource_from_row).optional()?.ok_or(AppError::NotFound)?;
    load_resource_details(&conn, &mut resource, patch.profile_id.as_deref())?;
    drop(conn);
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
    let mut conn = lock(&state)?;
    persist_resource(&mut conn, &resource, false, None)?;
    if let Some(pid) = profile_id {
        Uuid::parse_str(&pid).map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
        conn.execute("INSERT INTO profile_resources(profile_id,resource_id,favorite,category) VALUES (?1,?2,?3,?4) ON CONFLICT(profile_id,resource_id) DO UPDATE SET favorite=excluded.favorite,category=excluded.category",params![pid,resource.id,if resource.favorite {1i64}else{0i64},resource.category])?;
    }
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

fn search_resources(
    conn: &Connection,
    q: &str,
    profile_id: Option<&str>,
    is_favorite: bool,
    limit: usize,
) -> Result<Vec<Resource>, AppError> {
    let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let starts = format!("{}%", q);
    let sql="SELECT r.id,r.resource_type,r.display_name,r.description,r.icon,r.default_action,r.created_utc,r.updated_utc FROM resources r WHERE (?1='' OR r.display_name LIKE ?2 ESCAPE '\\' OR COALESCE(r.description,'') LIKE ?2 ESCAPE '\\' OR EXISTS(SELECT 1 FROM resource_terms t WHERE t.resource_id=r.id AND t.value LIKE ?2 ESCAPE '\\')) AND (?3 IS NULL OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND pr.profile_id=?3)) AND (?4=0 OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND (?3 IS NULL OR pr.profile_id=?3) AND pr.favorite=1)) ORDER BY CASE WHEN r.display_name=?1 COLLATE NOCASE THEN 0 WHEN r.display_name LIKE ?5 ESCAPE '\\' COLLATE NOCASE THEN 1 ELSE 2 END,r.display_name COLLATE NOCASE LIMIT ?6";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(
        params![
            q,
            like,
            profile_id,
            if is_favorite { 1i64 } else { 0i64 },
            starts,
            limit.clamp(1, 500) as i64
        ],
        resource_from_row,
    )?;
    let mut result = Vec::new();
    for row in rows {
        let mut resource = row?;
        load_resource_details(conn, &mut resource, profile_id)?;
        result.push(resource);
    }
    Ok(result)
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
    if name.trim().is_empty() {
        return Err(AppError::Invalid("profile name is required".into()));
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

#[tauri::command]
fn profile_switch(state: State<'_, AppState>, profile_id: String) -> Result<Profile, AppError> {
    Uuid::parse_str(&profile_id)
        .map_err(|_| AppError::Invalid("profile id must be a UUID".into()))?;
    let conn = lock(&state)?;
    conn.query_row(
        "SELECT id,name,theme_id,settings_json,created_utc,updated_utc FROM profiles WHERE id=?1",
        [profile_id],
        profile_from_row,
    )
    .optional()?
    .ok_or(AppError::NotFound)
}

#[tauri::command]
fn resource_launch(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: LaunchRequest,
) -> Result<LaunchResult, AppError> {
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
    if !["dashboard", "quick_launch", "dock", "explorer", "other"]
        .contains(&request.source.as_str())
    {
        return Err(AppError::Invalid("unsupported launch source".into()));
    }
    if let Some(existing)=conn.query_row("SELECT request_id,resource_id,target_id,profile_id,source,status,launched_at_utc,error_code,message,duration_ms FROM launch_events WHERE request_id=?1",[&request.request_id],launch_event_from_row).optional()? {return Ok(existing);}
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
            app.manage(AppState(Mutex::new(conn)));

            let registration = app.global_shortcut().register(QUICK_LAUNCH_HOTKEY_SPEC);
            let status = hotkey_status(registration.map_err(|error| error.to_string()));
            if !status.registered {
                eprintln!(
                    "Global shortcut {} failed: {}",
                    status.shortcut,
                    status.error_code.as_deref().unwrap_or("unknown")
                );
            }
            app.manage(HotkeyStatusState(Mutex::new(status.clone())));
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
            profile_list,
            profile_create,
            profile_switch,
            layout_get,
            layout_save,
            quick_launch_hotkey_status,
            analytics_query
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

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate_database(&conn).unwrap();
        conn
    }

    fn profile(conn: &Connection) -> String {
        let id = Uuid::new_v4().to_string();
        let ts = now_utc();
        conn.execute("INSERT INTO profiles(id,name,theme_id,settings_json,created_utc,updated_utc) VALUES (?1,'Work','system','{}',?2,?2)",params![id,ts]).unwrap();
        id
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
        assert!(!quick_launch_toggle_pressed(&other, &pressed));
    }

    #[test]
    fn quick_launch_hotkey_status_reports_success_and_conflict() {
        let success = hotkey_status(Ok(()));
        assert!(success.registered);
        assert_eq!(success.shortcut, "Alt+Space");
        assert_eq!(success.error_code, None);
        let conflict = hotkey_status(Err("hotkey already registered by another app".into()));
        assert!(!conflict.registered);
        assert_eq!(conflict.error_code.as_deref(), Some("hotkey_conflict"));
        let failure = hotkey_status(Err("native backend failed".into()));
        assert_eq!(
            failure.error_code.as_deref(),
            Some("hotkey_registration_failed")
        );
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
        assert_eq!(version, 3);
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
    }

    #[test]
    fn migration_upgrades_legacy_profile_and_launch_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,checksum TEXT NOT NULL,applied_utc TEXT NOT NULL); CREATE TABLE resources(id TEXT PRIMARY KEY,resource_type TEXT NOT NULL,display_name TEXT NOT NULL,description TEXT,icon TEXT,default_action TEXT NOT NULL,created_utc TEXT NOT NULL,updated_utc TEXT NOT NULL); CREATE TABLE resource_targets(id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,kind TEXT NOT NULL,canonical_uri TEXT NOT NULL,priority INTEGER NOT NULL DEFAULT 0,arguments_json TEXT NOT NULL DEFAULT '[]',working_directory TEXT,active INTEGER NOT NULL DEFAULT 1); CREATE TABLE profiles(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,theme_id TEXT NOT NULL DEFAULT 'system',settings_json TEXT NOT NULL DEFAULT '{}'); CREATE TABLE profile_resources(profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,favorite INTEGER NOT NULL DEFAULT 0,sort_position INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(profile_id,resource_id)); CREATE TABLE launch_events(id TEXT PRIMARY KEY,request_id TEXT NOT NULL UNIQUE,resource_id TEXT NOT NULL,target_id TEXT,profile_id TEXT,source TEXT NOT NULL,launched_at_utc TEXT NOT NULL,status TEXT NOT NULL,error_code TEXT);") .unwrap();
        migrate_database(&conn).unwrap();
        let created: String = conn
            .query_row(
                "SELECT created_utc FROM profiles WHERE name='Default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!created.is_empty());
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
