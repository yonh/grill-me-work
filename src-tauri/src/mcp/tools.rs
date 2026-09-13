//! MCP tool registry. Add new tools here; handlers receive [`McpContext`].

use serde_json::{json, Value};

use crate::workspace;

/// Shared state available to every tool.
pub struct McpContext {
    pub store: super::SharedStore,
    pub scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::SchedulerMsg>,
    #[allow(dead_code)]
    pub port: u16,
}

/// JSON Schema fragment for an object with no required props.
fn empty_object_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

/// All tool names (for /health).
pub fn tool_names() -> Vec<&'static str> {
    vec!["list_sessions"]
}

/// MCP `tools/list` payload.
pub fn tool_definitions() -> Vec<Value> {
    vec![json!({
        "name": "list_sessions",
        "description": "列出 Grill-Me 中的全部项目/会话（访谈项目）。返回 id、标题、角色、状态、原型版本号、工作区路径。",
        "inputSchema": empty_object_schema(),
    })]
}

/// Dispatch `tools/call`.
pub fn call_tool(ctx: &McpContext, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_sessions" => list_sessions(ctx, args),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn list_sessions(ctx: &McpContext, _args: &Value) -> Result<Value, String> {
    let sessions = ctx.store.get_sessions().map_err(|e| e.to_string())?;
    let items: Vec<Value> = sessions
        .iter()
        .map(|s| {
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
            })
        })
        .collect();

    Ok(json!({
        "count": items.len(),
        "sessions": items,
    }))
}
