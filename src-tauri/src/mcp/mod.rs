//! Minimal MCP (Model Context Protocol) server for Grill-Me.
//!
//! Exposes app capabilities as MCP tools over local HTTP JSON-RPC so external
//! AI agents (Claude Code, MiMo, Cursor, …) can drive interviews and prototypes.
//!
//! Endpoints (127.0.0.1 only):
//! - `POST /mcp` — JSON-RPC 2.0: initialize / tools/list / tools/call
//! - `GET  /health` — liveness + port info
//!
//! Tools register in `tools.rs`. Session tools cover the full interview loop:
//! create → outline review/confirm → answer/skip → finish, so an external
//! agent can run an interview given only a high-level direction.

pub mod protocol;
pub mod tools;

use std::sync::Arc;

use crate::store::Store;
pub use tools::McpContext;

pub const DEFAULT_MCP_PORT: u16 = 8787;

/// Shared handle used by tool handlers.
pub type SharedStore = Arc<dyn Store>;

/// Start the MCP HTTP server on 127.0.0.1. Non-blocking (background thread).
pub fn start_mcp_server(
    store: SharedStore,
    scheduler_tx: tokio::sync::mpsc::Sender<crate::scheduler::SchedulerMsg>,
    app: Option<tauri::AppHandle>,
) -> u16 {
    let port = pick_port(DEFAULT_MCP_PORT);
    let ctx = Arc::new(McpContext {
        store,
        scheduler_tx,
        app,
        port,
    });

    std::thread::Builder::new()
        .name("grill-me-mcp".into())
        .spawn(move || {
            if let Err(e) = serve_loop(port, ctx) {
                log::error!("[mcp] server exited: {e}");
            }
        })
        .ok();

    log::info!("[mcp] listening on http://127.0.0.1:{port}/mcp");
    port
}

fn pick_port(preferred: u16) -> u16 {
    for p in preferred..preferred + 20 {
        if std::net::TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    preferred
}

fn serve_loop(port: u16, ctx: Arc<McpContext>) -> Result<(), String> {
    let server = tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().clone();

        if method == tiny_http::Method::Get && (url == "/health" || url == "/") {
            let body = serde_json::json!({
                "ok": true,
                "service": "grill-me-mcp",
                "protocol": "mcp",
                "port": ctx.port,
                "endpoint": format!("http://127.0.0.1:{}/mcp", ctx.port),
                "tools": tools::tool_names(),
            })
            .to_string();
            let _ = request.respond(json_response(200, &body));
            continue;
        }

        if url != "/mcp" {
            let _ = request.respond(json_response(404, r#"{"error":"not found"}"#));
            continue;
        }

        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let _ = request.respond(json_response(400, r#"{"error":"bad body"}"#));
            continue;
        }

        let response = handle_rpc(&ctx, &body);
        let _ = request.respond(json_response(200, &response));
    }
    Ok(())
}

fn json_response(status: u16, body: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
        .expect("header");
    tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(header)
        .with_header(
            tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
        )
}

/// Handle one JSON-RPC payload (single message or batch array).
pub fn handle_rpc(ctx: &McpContext, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return protocol::error_response(None, -32700, "parse error").to_string();
    }

    // Batch
    if trimmed.starts_with('[') {
        let Ok(batch) = serde_json::from_str::<Vec<serde_json::Value>>(trimmed) else {
            return protocol::error_response(None, -32700, "parse error").to_string();
        };
        let out: Vec<serde_json::Value> = batch.iter().map(|msg| dispatch(ctx, msg)).collect();
        return serde_json::Value::Array(out).to_string();
    }

    let Ok(msg) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return protocol::error_response(None, -32700, "parse error").to_string();
    };
    dispatch(ctx, &msg).to_string()
}

fn dispatch(ctx: &McpContext, msg: &serde_json::Value) -> serde_json::Value {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

    // Notifications have no id and no response
    if method.starts_with("notifications/") {
        return serde_json::Value::Null;
    }

    match method {
        "initialize" => protocol::initialize_result(id),
        "ping" => protocol::pong_result(id),
        "tools/list" => protocol::tools_list_result(id, &tools::tool_definitions()),
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or(serde_json::json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let started = std::time::Instant::now();
            let outcome = tools::call_tool(ctx, name, &args);
            record_mcp_activity(ctx, name, &args, started.elapsed(), &outcome);
            match outcome {
                Ok(result) => protocol::tool_call_result(id, result),
                Err(e) => protocol::error_response(id, -32000, &e),
            }
        }
        "" => protocol::error_response(id, -32600, "invalid request"),
        other => protocol::error_response(id, -32601, &format!("method not found: {other}")),
    }
}

/// Read-only/polling tools — skipped in the activity feed so the timeline
/// shows only operations that change state.
const QUIET_TOOLS: &[&str] = &[
    "list_sessions", "get_active_session", "get_session_state", "get_outline",
    "list_questions", "get_messages", "get_prototype_versions", "get_timeline",
    "get_agent_log", "get_app_log", "get_pipeline", "list_rounds",
    "get_round_archive", "get_activity",
];

/// Persist one activity row per mutating MCP call so the user can see exactly
/// what an external agent triggered. Detail = compact args + duration + result.
fn record_mcp_activity(
    ctx: &McpContext,
    name: &str,
    args: &serde_json::Value,
    elapsed: std::time::Duration,
    outcome: &Result<serde_json::Value, String>,
) {
    if QUIET_TOOLS.contains(&name) {
        return;
    }
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // For tools like create_session the id is in the result.
            outcome
                .as_ref()
                .ok()
                .and_then(|v| v.get("session_id").and_then(|s| s.as_str()))
                .map(|s| s.to_string())
        })
        .or_else(|| {
            outcome
                .as_ref()
                .ok()
                .and_then(|v| {
                    v.get("session")
                        .and_then(|s| s.get("id"))
                        .and_then(|s| s.as_str())
                })
                .map(|s| s.to_string())
        });
    let Some(sid) = session_id else { return };

    // Compact args: keep meaningful fields, drop the session id and long blobs.
    let mut parts: Vec<String> = Vec::new();
    if let Some(obj) = args.as_object() {
        for (k, v) in obj {
            if k == "session_id" {
                continue;
            }
            let vs = match v {
                serde_json::Value::String(s) => {
                    let t: String = s.chars().take(50).collect();
                    format!("{k}={t}")
                }
                serde_json::Value::Null => continue,
                other => format!("{k}={}", other.to_string().chars().take(50).collect::<String>()),
            };
            parts.push(vs);
        }
    }
    let ms = elapsed.as_millis();
    let (level, tail) = match outcome {
        Ok(_) => ("ok", String::new()),
        Err(e) => {
            let e: String = e.chars().take(80).collect();
            ("err", format!(" · {e}"))
        }
    };
    let detail = format!("{} · {}ms{}", parts.join(" "), ms, tail);
    crate::scheduler::record_activity(
        ctx.store.as_ref(),
        ctx.app.as_ref(),
        &sid,
        "mcp",
        &format!("MCP: {name}"),
        Some(&detail),
        level,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::sqlite::SqliteStore;

    fn test_ctx(dir: &std::path::Path) -> Arc<McpContext> {
        let db = dir.join("t.db");
        let store = SqliteStore::new(db.to_str().unwrap()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        Arc::new(McpContext {
            store: Arc::new(store),
            scheduler_tx: tx,
            app: None,
            port: 0,
        })
    }

    #[test]
    fn initialize_and_list_sessions() {
        let tmp = std::env::temp_dir().join(format!("grill-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let ctx = test_ctx(&tmp);

        let init = handle_rpc(
            &ctx,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        );
        assert!(init.contains("grill-me"));

        let tools = handle_rpc(&ctx, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        assert!(tools.contains("list_sessions"));
        assert!(tools.contains("create_session"));
        assert!(tools.contains("answer_question"));
        assert!(tools.contains("confirm_outline"));

        let call = handle_rpc(
            &ctx,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_sessions","arguments":{}}}"#,
        );
        assert!(call.contains("sessions"), "got: {call}");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
