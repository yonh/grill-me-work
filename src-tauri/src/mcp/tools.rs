//! MCP tool registry. Add new tools here; handlers receive [`McpContext`].

use serde_json::{json, Value};

use crate::active_session;
use crate::workspace;

/// Shared state available to every tool.
pub struct McpContext {
    pub store: super::SharedStore,
    pub scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::SchedulerMsg>,
    /// Optional — used to push UI events (e.g. active session change).
    pub app: Option<tauri::AppHandle>,
    #[allow(dead_code)]
    pub port: u16,
}

/// JSON Schema fragment for an object with no required props.
fn empty_object_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn session_id_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "要设为激活的项目/会话 id"
            }
        },
        "required": ["session_id"],
        "additionalProperties": false
    })
}

/// All tool names (for /health).
pub fn tool_names() -> Vec<&'static str> {
    vec![
        "list_sessions",
        "get_active_session",
        "set_active_session",
    ]
}

/// MCP `tools/list` payload.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_sessions",
            "description": "列出 Grill-Me 中的全部项目/会话。返回 id、标题、角色、状态、原型版本、工作区路径，以及 is_active（当前打开的项目）。",
            "inputSchema": empty_object_schema(),
        }),
        json!({
            "name": "get_active_session",
            "description": "获取当前打开（激活）的项目。若未打开任何项目，session_id 为 null。",
            "inputSchema": empty_object_schema(),
        }),
        json!({
            "name": "set_active_session",
            "description": "切换 UI 当前打开的项目。传入 session_id；之后生成原型、答题、蓝图等默认作用于该项目。",
            "inputSchema": session_id_schema(),
        }),
    ]
}

/// Dispatch `tools/call`.
pub fn call_tool(ctx: &McpContext, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_sessions" => list_sessions(ctx, args),
        "get_active_session" => get_active_session(ctx, args),
        "set_active_session" => set_active_session(ctx, args),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn session_row(s: &crate::model::Session, active_id: Option<&str>) -> Value {
    let ws = workspace::session_workspace_dir(&s.id);
    let proto = workspace::session_prototype_dir(&s.id);
    json!({
        "id": s.id,
        "title": s.title,
        "role": s.role,
        "status": s.status.as_str(),
        "prototype_version": s.prototype_version,
        "has_summary": s.summary.as_ref().map(|x| !x.is_empty()).unwrap_or(false),
        "created_at": s.created_at,
        "updated_at": s.updated_at,
        "workspace_path": ws.to_string_lossy(),
        "prototype_path": proto.to_string_lossy(),
        "has_prototype": proto.join("index.html").exists(),
        "is_active": active_id == Some(s.id.as_str()),
    })
}

fn list_sessions(ctx: &McpContext, _args: &Value) -> Result<Value, String> {
    let sessions = ctx.store.get_sessions().map_err(|e| e.to_string())?;
    let active = active_session::get_active_id(ctx.store.as_ref());
    let items: Vec<Value> = sessions
        .iter()
        .map(|s| session_row(s, active.as_deref()))
        .collect();

    Ok(json!({
        "count": items.len(),
        "active_session_id": active,
        "sessions": items,
    }))
}

fn get_active_session(ctx: &McpContext, _args: &Value) -> Result<Value, String> {
    let info = active_session::describe(ctx.store.as_ref());
    Ok(serde_json::to_value(&info).map_err(|e| e.to_string())?)
}

fn set_active_session(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or("missing session_id")?;
    let info = active_session::set_active(ctx.store.as_ref(), Some(id), ctx.app.as_ref())?;
    log::info!("[mcp] active session → {id}");
    Ok(serde_json::to_value(&info).map_err(|e| e.to_string())?)
}