//! MCP JSON-RPC protocol helpers (subset used by the server).

use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "grill-me";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

fn envelope(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

pub fn error_response(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

pub fn initialize_result(id: Option<Value>) -> Value {
    envelope(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            },
            "instructions": "Grill-Me 需求访谈与原型助手。用 list_sessions 查看项目，后续工具可答题、生成原型、操作迭代蓝图与时间线。",
        }),
    )
}

pub fn pong_result(id: Option<Value>) -> Value {
    envelope(id, json!({}))
}

pub fn tools_list_result(id: Option<Value>, tools: &[Value]) -> Value {
    envelope(id, json!({ "tools": tools }))
}

/// MCP tool result: content array + optional structuredContent.
pub fn tool_call_result(id: Option<Value>, result: Value) -> Value {
    envelope(
        id,
        json!({
            "content": [
                { "type": "text", "text": result.to_string() }
            ],
            "structuredContent": result,
            "isError": false,
        }),
    )
}
