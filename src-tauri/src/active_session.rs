//! Active (currently open) session — shared by UI and MCP so AI can target "the open project".

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::events::{ActiveSessionPayload, EVENT_ACTIVE_SESSION};
use crate::store::Store;
use crate::workspace;

pub const SETTING_KEY: &str = "active_session_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSessionInfo {
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub prototype_version: Option<i32>,
    pub workspace_path: Option<String>,
    pub prototype_path: Option<String>,
    pub has_prototype: bool,
}

impl ActiveSessionInfo {
    pub fn none() -> Self {
        ActiveSessionInfo {
            session_id: None,
            title: None,
            role: None,
            status: None,
            prototype_version: None,
            workspace_path: None,
            prototype_path: None,
            has_prototype: false,
        }
    }
}

pub fn get_active_id(store: &dyn Store) -> Option<String> {
    store
        .get_setting(SETTING_KEY)
        .ok()
        .flatten()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn describe(store: &dyn Store) -> ActiveSessionInfo {
    let Some(id) = get_active_id(store) else {
        return ActiveSessionInfo::none();
    };
    let Ok(Some(s)) = store.get_session(&id) else {
        return ActiveSessionInfo::none();
    };
    let ws = workspace::session_workspace_dir(&s.id);
    let proto = workspace::session_prototype_dir(&s.id);
    ActiveSessionInfo {
        session_id: Some(s.id),
        title: Some(s.title),
        role: Some(s.role),
        status: Some(s.status.as_str().to_string()),
        prototype_version: Some(s.prototype_version),
        workspace_path: Some(ws.to_string_lossy().to_string()),
        prototype_path: Some(proto.to_string_lossy().to_string()),
        has_prototype: proto.join("index.html").exists(),
    }
}

/// Persist active session and optionally notify the UI.
pub fn set_active(
    store: &dyn Store,
    session_id: Option<&str>,
    app: Option<&AppHandle>,
) -> Result<ActiveSessionInfo, String> {
    match session_id {
        Some(id) => {
            let id = id.trim();
            if id.is_empty() {
                return Err("session_id is empty".into());
            }
            store
                .get_session(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("session not found: {id}"))?;
            store
                .set_setting(SETTING_KEY, id)
                .map_err(|e| e.to_string())?;
        }
        None => {
            store
                .set_setting(SETTING_KEY, "")
                .map_err(|e| e.to_string())?;
        }
    }

    let info = describe(store);
    if let Some(app) = app {
        let _ = app.emit(
            EVENT_ACTIVE_SESSION,
            ActiveSessionPayload {
                session_id: info.session_id.clone(),
                title: info.title.clone(),
            },
        );
    }
    Ok(info)
}

/// Clear active session if the id was deleted.
pub fn clear_if_matches(store: &dyn Store, session_id: &str, app: Option<&AppHandle>) {
    if get_active_id(store).as_deref() == Some(session_id) {
        let _ = set_active(store, None, app);
    }
}
