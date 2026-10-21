use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use tauri::{Emitter, Manager, State};
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
    where S: serde::Serializer {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInput {
    pub resource_type: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_action: String,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<ResourceTarget>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePatch {
    pub resource_type: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub default_action: Option<String>,
    pub targets: Option<Vec<ResourceTarget>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub request_id: String,
    pub resource_id: String,
    pub target_id: Option<String>,
    pub profile_id: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub theme_id: String,
    pub settings_json: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub request_id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub launched_at_utc: String,
}

pub struct AppState(pub Mutex<Connection>);

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
 settings_json TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS profile_resources (
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
 resource_id TEXT NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
 favorite INTEGER NOT NULL DEFAULT 0, sort_position INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(profile_id, resource_id)
);
CREATE TABLE IF NOT EXISTS launch_events (
 id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, resource_id TEXT NOT NULL,
 target_id TEXT, profile_id TEXT, source TEXT NOT NULL, launched_at_utc TEXT NOT NULL,
 status TEXT NOT NULL, error_code TEXT
);
CREATE INDEX IF NOT EXISTS idx_launch_resource_time ON launch_events(resource_id, launched_at_utc DESC);
"#;

fn now_utc() -> String { Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true) }

pub fn open_database(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| AppError::Invalid(e.to_string()))?; }
    let conn = Connection::open(path)?;
    conn.execute_batch(MIGRATION)?;
    Ok(conn)
}

fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
    state.0.lock().map_err(|_| AppError::Invalid("database lock poisoned".into()))
}

fn resource_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Resource> {
    Ok(Resource { id: row.get(0)?, resource_type: row.get(1)?, display_name: row.get(2)?, description: row.get(3)?, icon: row.get(4)?, default_action: row.get(5)?, created_utc: row.get(6)?, updated_utc: row.get(7)?, targets: Vec::new() })
}

fn load_targets(conn: &Connection, resource: &mut Resource) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT id,kind,canonical_uri,priority,arguments_json,working_directory,active FROM resource_targets WHERE resource_id=?1 ORDER BY priority DESC")?;
    let rows = stmt.query_map([&resource.id], |r| Ok(ResourceTarget { id:r.get(0)?, kind:r.get(1)?, canonical_uri:r.get(2)?, priority:r.get(3)?, arguments:serde_json::from_str(&r.get::<_,String>(4)?).unwrap_or_default(), working_directory:r.get(5)?, active:r.get::<_,i64>(6)? != 0 }))?;
    resource.targets = rows.collect::<Result<Vec<_>,_>>()?;
    Ok(())
}

#[tauri::command]
pub fn resource_list(state: State<'_, AppState>, profile_id: Option<String>) -> Result<Vec<Resource>, AppError> {
    let conn = lock(&state)?;
    let mut stmt = conn.prepare("SELECT r.id,r.resource_type,r.display_name,r.description,r.icon,r.default_action,r.created_utc,r.updated_utc FROM resources r WHERE (?1 IS NULL OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND pr.profile_id=?1)) ORDER BY r.display_name COLLATE NOCASE")?;
    let mut rows = stmt.query_map([profile_id], resource_from_row)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? { let mut resource = row?; load_targets(&conn, &mut resource)?; result.push(resource); }
    Ok(result)
}

fn validate_resource(resource: &Resource) -> Result<(), AppError> {
    Uuid::parse_str(&resource.id).map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    if resource.display_name.trim().is_empty() { return Err(AppError::Invalid("display name is required".into())); }
    for target in &resource.targets {
        Uuid::parse_str(&target.id).map_err(|_| AppError::Invalid("target id must be a UUID".into()))?;
        validate_target(target)?;
    }
    Ok(())
}

fn validate_target(target: &ResourceTarget) -> Result<(), AppError> {
    match target.kind.as_str() {
        "url" => {
            let parsed = Url::parse(&target.canonical_uri).map_err(|_| AppError::Invalid("URL is invalid".into()))?;
            if parsed.scheme() != "https" { return Err(AppError::Invalid("only HTTPS URLs may be opened".into())); }
            if !target.arguments.is_empty() { return Err(AppError::Invalid("arguments are only allowed for executable targets".into())); }
        }
        "file" | "folder" | "executable" | "shortcut" => {
            let path = target.canonical_uri.trim();
            let drive_path = path.as_bytes().get(1) == Some(&b':') && path.as_bytes().get(2).is_some_and(|b| *b == b'\\' || *b == b'/');
            let unc_path = path.starts_with("\\\\") && path.len() > 2;
            if !(drive_path || unc_path) || path.starts_with("\\\\?\\") { return Err(AppError::Invalid("target must be an absolute Windows path or UNC path".into())); }
            if target.kind != "executable" && !target.arguments.is_empty() { return Err(AppError::Invalid("arguments are only allowed for executable targets".into())); }
        }
        _ => return Err(AppError::Invalid("unsupported target kind".into())),
    }
    if target.arguments.iter().any(|a| a.contains('\0')) { return Err(AppError::Invalid("arguments cannot contain NUL".into())); }
    Ok(())
}

fn persist_resource(conn: &mut Connection, resource: &Resource, creating: bool, profile_id: Option<&str>) -> Result<(), AppError> {
    validate_resource(resource)?;
    let tx = conn.transaction()?;
    if creating {
        tx.execute("INSERT INTO resources VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![resource.id,resource.resource_type,resource.display_name,resource.description,resource.icon,resource.default_action,resource.created_utc,resource.updated_utc])?;
    } else {
        let changed = tx.execute("UPDATE resources SET resource_type=?2,display_name=?3,description=?4,icon=?5,default_action=?6,updated_utc=?7 WHERE id=?1", params![resource.id,resource.resource_type,resource.display_name,resource.description,resource.icon,resource.default_action,resource.updated_utc])?;
        if changed == 0 { return Err(AppError::NotFound); }
        tx.execute("DELETE FROM resource_targets WHERE resource_id=?1", [&resource.id])?;
    }
    for target in &resource.targets {
        tx.execute("INSERT INTO resource_targets VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![target.id,resource.id,target.kind,target.canonical_uri,target.priority,serde_json::to_string(&target.arguments)?,target.working_directory,if target.active {1i64} else {0i64}])?;
    }
    if creating {
        if let Some(profile_id)=profile_id {
            Uuid::parse_str(profile_id).map_err(|_|AppError::Invalid("profile id must be a UUID".into()))?;
            tx.execute("INSERT INTO profile_resources(profile_id,resource_id) VALUES (?1,?2)",params![profile_id,resource.id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn resource_create(state: State<'_, AppState>, resource: ResourceInput) -> Result<Resource, AppError> {
    let ts = now_utc();
    let profile_id=resource.profile_id;
    let resource = Resource { id:Uuid::new_v4().to_string(), resource_type:resource.resource_type, display_name:resource.display_name, description:resource.description, icon:resource.icon, default_action:resource.default_action, created_utc:ts.clone(), updated_utc:ts, targets:resource.targets };
    persist_resource(&mut lock(&state)?, &resource, true, profile_id.as_deref())?; Ok(resource)
}

#[tauri::command]
pub fn resource_update(state: State<'_, AppState>, id: String, patch: ResourcePatch) -> Result<Resource, AppError> {
    Uuid::parse_str(&id).map_err(|_|AppError::Invalid("resource id must be a UUID".into()))?;
    let conn=lock(&state)?;
    let mut resource=conn.query_row("SELECT id,resource_type,display_name,description,icon,default_action,created_utc,updated_utc FROM resources WHERE id=?1",[&id],resource_from_row).optional()?.ok_or(AppError::NotFound)?;
    load_targets(&conn,&mut resource)?;
    drop(conn);
    if let Some(v)=patch.resource_type {resource.resource_type=v;} if let Some(v)=patch.display_name {resource.display_name=v;}
    if patch.description.is_some() {resource.description=patch.description;} if patch.icon.is_some() {resource.icon=patch.icon;}
    if let Some(v)=patch.default_action {resource.default_action=v;} if let Some(v)=patch.targets {resource.targets=v;}
    resource.updated_utc=now_utc(); persist_resource(&mut lock(&state)?, &resource, false, None)?; Ok(resource)
}

#[tauri::command]
pub fn resource_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    Uuid::parse_str(&id).map_err(|_| AppError::Invalid("resource id must be a UUID".into()))?;
    let changed = lock(&state)?.execute("DELETE FROM resources WHERE id=?1", [id])?; if changed == 0 { return Err(AppError::NotFound); } Ok(())
}

#[tauri::command]
pub fn resource_search(state: State<'_, AppState>, query: String, profile_id: Option<String>) -> Result<Vec<Resource>, AppError> {
    let q = query.trim(); if q.is_empty() { return resource_list(state, profile_id); }
    let conn = lock(&state)?;
    let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare("SELECT r.id,r.resource_type,r.display_name,r.description,r.icon,r.default_action,r.created_utc,r.updated_utc FROM resources r WHERE (r.display_name LIKE ?1 ESCAPE '\\' OR COALESCE(r.description,'') LIKE ?1 ESCAPE '\\') AND (?4 IS NULL OR EXISTS(SELECT 1 FROM profile_resources pr WHERE pr.resource_id=r.id AND pr.profile_id=?4)) ORDER BY CASE WHEN r.display_name=?2 COLLATE NOCASE THEN 0 WHEN r.display_name LIKE ?3 ESCAPE '\\' COLLATE NOCASE THEN 1 ELSE 2 END,r.display_name COLLATE NOCASE")?;
    let rows = stmt.query_map(params![like,q,format!("{}%",q),profile_id], resource_from_row)?;
    let mut result = Vec::new(); for row in rows { let mut r=row?; load_targets(&conn,&mut r)?; result.push(r); } Ok(result)
}

#[tauri::command]
pub fn profile_list(state: State<'_, AppState>) -> Result<Vec<Profile>, AppError> {
    let conn=lock(&state)?; let mut stmt=conn.prepare("SELECT id,name,theme_id,settings_json FROM profiles ORDER BY name COLLATE NOCASE")?;
    Ok(stmt.query_map([],|r|Ok(Profile{id:r.get(0)?,name:r.get(1)?,theme_id:r.get(2)?,settings_json:r.get(3)?}))?.collect::<Result<Vec<_>,_>>()?)
}

#[tauri::command]
pub fn profile_create(state: State<'_, AppState>, name: String) -> Result<Profile, AppError> {
    if name.trim().is_empty() { return Err(AppError::Invalid("profile name is required".into())); }
    let profile=Profile{id:Uuid::new_v4().to_string(),name:name.trim().into(),theme_id:"system".into(),settings_json:"{}".into()};
    lock(&state)?.execute("INSERT INTO profiles VALUES (?1,?2,?3,?4)",params![profile.id,profile.name,profile.theme_id,profile.settings_json])?; Ok(profile)
}

#[tauri::command]
pub fn profile_switch(state: State<'_, AppState>, profile_id: String) -> Result<Profile, AppError> {
    Uuid::parse_str(&profile_id).map_err(|_|AppError::Invalid("profile id must be a UUID".into()))?;
    let conn=lock(&state)?;
    conn.query_row("SELECT id,name,theme_id,settings_json FROM profiles WHERE id=?1",[profile_id],|r|Ok(Profile{id:r.get(0)?,name:r.get(1)?,theme_id:r.get(2)?,settings_json:r.get(3)?})).optional()?.ok_or(AppError::NotFound)
}

#[tauri::command]
pub fn resource_launch(app: tauri::AppHandle, state: State<'_, AppState>, request: LaunchRequest) -> Result<LaunchResult, AppError> {
    let LaunchRequest{request_id,resource_id,target_id,profile_id,source}=request;
    Uuid::parse_str(&request_id).map_err(|_|AppError::Invalid("request id must be a UUID".into()))?;
    Uuid::parse_str(&resource_id).map_err(|_|AppError::Invalid("resource id must be a UUID".into()))?;
    let launched_at_utc=now_utc();
    let target = {
        let conn=lock(&state)?;
        if let Some(existing) = conn.query_row("SELECT status,error_code,launched_at_utc FROM launch_events WHERE request_id=?1",[&request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?))).optional()? {
            return Ok(LaunchResult { request_id, status: existing.0, error_code: existing.1, launched_at_utc: existing.2 });
        }
        let target: Option<ResourceTarget> = conn.query_row("SELECT id,kind,canonical_uri,priority,arguments_json,working_directory,active FROM resource_targets WHERE resource_id=?1 AND active=1 AND (?2 IS NULL OR id=?2) ORDER BY priority DESC LIMIT 1",params![resource_id,target_id],|r|Ok(ResourceTarget{id:r.get(0)?,kind:r.get(1)?,canonical_uri:r.get(2)?,priority:r.get(3)?,arguments:serde_json::from_str(&r.get::<_,String>(4)?).unwrap_or_default(),working_directory:r.get(5)?,active:r.get::<_,i64>(6)? != 0})).optional()?;
        // Reserve the request before crossing the OS boundary. A duplicate request
        // can never start a second process, even when IPC calls race.
        let chosen_id=target.as_ref().map(|t|t.id.clone());
        let inserted=conn.execute("INSERT OR IGNORE INTO launch_events(id,request_id,resource_id,target_id,profile_id,source,launched_at_utc,status,error_code) VALUES (?1,?2,?3,?4,?5,?6,?7,'requested',NULL)",params![Uuid::new_v4().to_string(),request_id,resource_id,chosen_id,profile_id,source.unwrap_or_else(||"dashboard".into()),launched_at_utc])?;
        if inserted == 0 {
            let existing=conn.query_row("SELECT status,error_code,launched_at_utc FROM launch_events WHERE request_id=?1",[&request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?)))?;
            return Ok(LaunchResult { request_id, status:existing.0,error_code:existing.1,launched_at_utc:existing.2 });
        }
        target
    };
    let outcome = match target {
        None => Err("target_not_found".to_string()),
        Some(t) => launch_target(&app, &t).map_err(|e| e.to_string()),
    };
    let (status,error_code)=match outcome { Ok(())=>("handed_off".to_string(),None),Err(e)=>("failed".to_string(),Some(e)) };
    let result=LaunchResult{request_id:request_id.clone(),status:status.clone(),error_code:error_code.clone(),launched_at_utc:launched_at_utc.clone()};
    let conn=lock(&state)?;
    conn.execute("UPDATE launch_events SET status=?2,error_code=?3 WHERE request_id=?1",params![request_id,status,error_code])?;
    drop(conn);
    let _ = app.emit("launch-result", &result);
    Ok(result)
}

#[cfg(windows)]
fn launch_target(_app: &tauri::AppHandle, target: &ResourceTarget) -> Result<(), AppError> {
    use windows::{core::PCWSTR, Win32::UI::Shell::ShellExecuteW, Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL};
    validate_target(target)?;
    let verb: Vec<u16>="open\0".encode_utf16().collect(); let path:Vec<u16>=format!("{}\0",target.canonical_uri).encode_utf16().collect();
    let params=if target.arguments.is_empty(){None}else{Some(target.arguments.iter().map(|arg| quote_windows_argument(arg)).collect::<Vec<_>>().join(" "))};
    let params_w=params.as_ref().map(|p|format!("{}\0",p).encode_utf16().collect::<Vec<_>>());
    let work_w=target.working_directory.as_ref().map(|p|format!("{}\0",p).encode_utf16().collect::<Vec<_>>());
    let code=unsafe { ShellExecuteW(None,PCWSTR(verb.as_ptr()),PCWSTR(path.as_ptr()),params_w.as_ref().map_or(PCWSTR::null(),|p|PCWSTR(p.as_ptr())),work_w.as_ref().map_or(PCWSTR::null(),|p|PCWSTR(p.as_ptr())),SW_SHOWNORMAL) };
    if code.0 as isize <= 32 { return Err(AppError::Launch(format!("windows_shell_error_{}",code.0))); } Ok(())
}

#[cfg(windows)]
fn quote_windows_argument(arg: &str) -> String {
    // Windows command-line escaping: quote every argument and double backslashes
    // before a quote or the closing quote, rather than concatenating raw strings.
    let mut out = String::from("\"");
    let mut slashes = 0;
    for ch in arg.chars() {
        if ch == '\\' { slashes += 1; continue; }
        if ch == '"' { out.push_str(&"\\".repeat(slashes * 2 + 1)); out.push('"'); }
        else { out.push_str(&"\\".repeat(slashes)); out.push(ch); }
        slashes = 0;
    }
    out.push_str(&"\\".repeat(slashes * 2)); out.push('"'); out
}

#[cfg(not(windows))]
fn launch_target(_app: &tauri::AppHandle, _target: &ResourceTarget) -> Result<(), AppError> {
    Err(AppError::Launch("windows_shell_unavailable_on_this_platform".into()))
}

pub fn run() {
    tauri::Builder::default().setup(|app| {
        let path=app.path().app_local_data_dir()?.join("launcher.sqlite3");
        let conn=open_database(&path).map_err(|e|std::io::Error::other(e.to_string()))?;
        app.manage(AppState(Mutex::new(conn)));
        Ok(())
    }).invoke_handler(tauri::generate_handler![resource_list,resource_create,resource_update,resource_delete,resource_search,resource_launch,profile_list,profile_create,profile_switch]).run(tauri::generate_context!()).expect("failed to run Smart Workspace Launcher");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(kind:&str,uri:&str)->ResourceTarget { ResourceTarget{id:Uuid::new_v4().to_string(),kind:kind.into(),canonical_uri:uri.into(),priority:0,arguments:vec![],working_directory:None,active:true} }
    #[test] fn rejects_non_https_urls() { assert!(validate_target(&sample("url","http://example.com")).is_err()); assert!(validate_target(&sample("url","javascript:alert(1)")).is_err()); }
    #[test] fn accepts_https_urls() { assert!(validate_target(&sample("url","https://example.com/path")).is_ok()); }
    #[test] fn rejects_unknown_target_kinds() { assert!(validate_target(&sample("script","powershell.exe")).is_err()); }
    #[test] fn resource_ids_must_be_uuid() { let r=Resource{id:"path-as-id".into(),resource_type:"file".into(),display_name:"x".into(),description:None,icon:None,default_action:"open".into(),created_utc:now_utc(),updated_utc:now_utc(),targets:vec![]}; assert!(validate_resource(&r).is_err()); }
    #[test] fn migration_creates_core_tables() { let c=Connection::open_in_memory().unwrap(); c.execute_batch(MIGRATION).unwrap(); let n:i64=c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table'",[],|r|r.get(0)).unwrap(); assert!(n>=4); }
}
