//! MCP tool registry. Add new tools here; handlers receive [`McpContext`].
//!
//! The MCP server runs on a plain thread (no tokio runtime), so scheduler
//! calls go through `mpsc::blocking_send` + `oneshot::blocking_recv`.

use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::active_session;
use crate::model::{AnswerValue, OutlineNode, OutlineNodeStatus};
use crate::scheduler::SchedulerMsg;
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
                "description": "目标项目/会话 id"
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
        // --- interview loop ---
        "create_session",
        "get_session_state",
        "get_outline",
        "generate_outline",
        "save_outline",
        "confirm_outline",
        "dismiss_outline",
        "list_questions",
        "answer_question",
        "skip_question",
        "send_message",
        "request_batch",
        "finish_session",
        "generate_prototype",
        "get_messages",
        "get_prototype_versions",
        "regenerate_stale",
        "dismiss_stale",
        // --- timeline / branch (gacha-mode development) ---
        "get_timeline",
        "checkout_ref",
        "fork_branch",
        "run_graph",
        "cancel_graph",
        "cancel_prototype",
        "get_agent_log",
        "get_app_log",
        // --- pipeline: spec → tickets → branch-map development ---
        "get_pipeline",
        "generate_spec",
        "save_spec",
        "confirm_spec",
        "generate_tickets",
        "save_tickets",
        "confirm_tickets",
        "set_ticket_status",
        "run_ticket",
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
        // --- interview loop ---
        json!({
            "name": "create_session",
            "description": "新建访谈会话并自动生成访谈大纲。role 可选：pm（需求方）/dev（技术）/designer（设计）/manager（项目管理），默认 pm。initial_context 传入人类给的大方向。创建后调用 get_outline 轮询大纲（draft 状态时可编辑或直接确认）。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "项目/会话标题" },
                    "role": { "type": "string", "description": "访谈角色视角：pm|dev|designer|manager，默认 pm" },
                    "initial_context": { "type": "string", "description": "初始上下文/大方向描述" }
                },
                "required": ["title"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "get_session_state",
            "description": "获取会话当前状态快照：会话信息、大纲状态与各节点覆盖情况、各状态问题数量。适合作为轮询入口判断下一步该做什么（等大纲/答题/确认完成）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "get_outline",
            "description": "获取访谈大纲（status + 节点列表，含每节点已答/待答数）。status: none|generating|draft|confirmed。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "generate_outline",
            "description": "（重新）生成访谈大纲。已有大纲会话慎用——会覆盖为新的 draft。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "save_outline",
            "description": "保存编辑后的大纲节点数组（顺序即优先级）。每项 {id?, title, description?, excluded?}：id 省略则新建节点；excluded=true 排除该方向。对已有节点不传 excluded 时保留其原状态。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "nodes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "已有节点 id；省略则新建" },
                                "title": { "type": "string" },
                                "description": { "type": "string" },
                                "excluded": { "type": "boolean", "description": "true 表示排除，不再就它出题" }
                            },
                            "required": ["title"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["session_id", "nodes"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "confirm_outline",
            "description": "确认大纲，开始按节点出题。确认前不会生成任何问题。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "dismiss_outline",
            "description": "放弃大纲进入自由访谈模式（不限定范围，慎用——可能跑题）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "list_questions",
            "description": "列出会话的问题。可选 status 过滤：ready（待答）|answered|skipped|stale|generating；默认返回全部。每题含 options 和所属大纲节点标题。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "status": { "type": "string", "description": "可选过滤：ready|answered|skipped|stale|generating" }
                },
                "required": ["session_id"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "answer_question",
            "description": "回答一道题。answer 可以是字符串（开放回答），或 {kind:\"choice\",option:\"选项label\"} / {kind:\"multi\",options:[\"label\"]} / {kind:\"open\",text:\"...\"}。答题会自动触发大纲覆盖刷新与下一批出题。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "question_id": { "type": "string" },
                    "answer": { "description": "字符串或 AnswerValue 对象" }
                },
                "required": ["session_id", "question_id", "answer"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "skip_question",
            "description": "跳过一道题。跳过会作为负面信号，系统不会换着说法重问该方向。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "question_id": { "type": "string" }
                },
                "required": ["session_id", "question_id"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "send_message",
            "description": "向会话发送一条用户消息（访谈对话）。用于表达方向、约束或偏好，会影响后续出题。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "content": { "type": "string", "description": "消息文本" }
                },
                "required": ["session_id", "content"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "request_batch",
            "description": "手动请求再生成一批问题（会清除“已饱和”状态）。一般不需要调用——答题后会自动补题。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "finish_session",
            "description": "结束会话，返回全部决策与总结。大纲节点全部覆盖后调用，或人类明确要求结束时调用。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "generate_prototype",
            "description": "触发一次原型生成/更新（同步阻塞直到 agent 跑完，可能几分钟；返回 version/changelog/file_count 可直接用于验收）。feedback 可选，给 coding agent 的修改要求。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "feedback": { "type": "string", "description": "可选：给原型 agent 的反馈，如“增加火球术技能”" }
                },
                "required": ["session_id"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "get_messages",
            "description": "读取会话的访谈对话记录（含 assistant 回复）。send_message 之后用它看系统如何回应你的方向输入。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "get_prototype_versions",
            "description": "列出原型版本历史（版本号、changelog、commit、文件数）。用于验收/回溯。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "regenerate_stale",
            "description": "重新生成一批 stale（已过期）问题——答题会使依赖题失效，用它让系统基于新答案重出。传 question_ids 数组。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "question_ids": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["session_id", "question_ids"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "dismiss_stale",
            "description": "废弃一道 stale 题（不再重出，也不再问）。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "question_id": { "type": "string" }
                },
                "required": ["question_id"],
                "additionalProperties": false
            }),
        }),
        // --- timeline / branch (gacha-mode development) ---
        json!({
            "name": "get_timeline",
            "description": "获取 git 决策树状态（当前分支、HEAD、分支列表）+ 提交图（含分支归属）。抽卡式开发前用它找基点 commit。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "checkout_ref",
            "description": "切换工作区到某个分支或 commit（prototype/ 内容随之切换，预览即变）。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "ref": { "type": "string", "description": "分支名或 commit sha" }
                },
                "required": ["session_id", "ref"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "fork_branch",
            "description": "从指定 commit（默认当前 HEAD）检出新分支——抽卡模式的核心：同一基点开多条线各自迭代。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "branch_name": { "type": "string", "description": "新分支名，如 fireball-try1" },
                    "from_sha": { "type": "string", "description": "基点 commit sha，省略=当前 HEAD" }
                },
                "required": ["session_id", "branch_name"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "run_graph",
            "description": "运行该会话的迭代蓝图（蓝图定义的 agent 步骤串行执行，每步成功自动 commit）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "cancel_graph",
            "description": "取消正在运行的迭代蓝图。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "cancel_prototype",
            "description": "中止该会话当前正在跑的原型 agent 进程（kill 子进程）。返回 was_running 表示当时是否真有任务在跑。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "get_agent_log",
            "description": "读取该会话最近一次原型 agent 运行的输出日志（环形缓冲，最多 400 行）。tail 可选，只取最后 N 行。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "tail": { "type": "integer", "description": "只返回最后 N 行" }
                },
                "required": ["session_id"],
                "additionalProperties": false
            }),
        }),
        // --- pipeline: spec → tickets → branch-map development ---
        json!({
            "name": "get_pipeline",
            "description": "获取流水线快照：stage（interviewing|spec_draft|tickets_draft|developing|none=旧会话自由模式）、spec 全文、tickets 列表（状态/依赖/分支）。新流程的轮询入口。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "generate_spec",
            "description": "基于访谈决策+大纲+对话生成 spec 草稿（异步，轮询 get_pipeline 直到 stage=spec_draft）。访谈完成后调用。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "save_spec",
            "description": "保存对 spec 草稿的编辑（仅 spec_draft 阶段）。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "spec": { "type": "string", "description": "编辑后的 spec markdown 全文" }
                },
                "required": ["session_id", "spec"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "confirm_spec",
            "description": "确认 spec——写入 docs/spec.md 并自动开始生成 tickets（stage → tickets_draft）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "generate_tickets",
            "description": "基于已确认 spec 重新生成 tickets 草稿（异步）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "save_tickets",
            "description": "保存对 tickets 的编辑（仅 tickets_draft 阶段）：可改标题/描述/依赖/顺序/增删。传完整 tickets 数组。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "tickets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "title": { "type": "string" },
                                "description": { "type": "string" },
                                "status": { "type": "string" },
                                "depends_on": { "type": "array", "items": { "type": "string" } },
                                "display_order": { "type": "integer" }
                            },
                            "required": ["id", "title"]
                        }
                    }
                },
                "required": ["session_id", "tickets"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "confirm_tickets",
            "description": "确认 tickets——stage → developing，之后才能 run_ticket / generate_prototype（强制门控）。",
            "inputSchema": session_id_schema(),
        }),
        json!({
            "name": "set_ticket_status",
            "description": "设置 ticket 状态：pending|in_progress|done|rejected。验收后标 done，废弃标 rejected。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "string" },
                    "status": { "type": "string" }
                },
                "required": ["ticket_id", "status"],
                "additionalProperties": false
            }),
        }),
        json!({
            "name": "run_ticket",
            "description": "为某条 ticket 开一条开发线：从基点 commit（默认 HEAD）fork 分支并自动跑 N 轮 agent 迭代。异步——立即返回分支名，进度看 timeline_updated / get_timeline。同一 ticket 可多次调用开多条抽卡分支。",
            "inputSchema": json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "ticket_id": { "type": "string" },
                    "rounds": { "type": "integer", "description": "迭代轮数，默认 3，上限 10" },
                    "branch_name": { "type": "string", "description": "自定义分支名，默认 t<序号>-<标题>" },
                    "from_sha": { "type": "string", "description": "基点 commit，默认当前 HEAD" },
                    "feedback": { "type": "string", "description": "给 agent 的补充要求/实现思路" }
                },
                "required": ["session_id", "ticket_id"],
                "additionalProperties": false
            }),
        }),
    ]
}

/// Dispatch `tools/call`.
pub fn call_tool(ctx: &McpContext, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_sessions" => list_sessions(ctx, args),
        "get_active_session" => get_active_session(ctx, args),
        "set_active_session" => set_active_session(ctx, args),
        "create_session" => create_session(ctx, args),
        "get_session_state" => get_session_state(ctx, args),
        "get_outline" => get_outline(ctx, args),
        "generate_outline" => generate_outline(ctx, args),
        "save_outline" => save_outline(ctx, args),
        "confirm_outline" => confirm_outline(ctx, args),
        "dismiss_outline" => dismiss_outline(ctx, args),
        "list_questions" => list_questions(ctx, args),
        "answer_question" => answer_question(ctx, args),
        "skip_question" => skip_question(ctx, args),
        "send_message" => send_message(ctx, args),
        "request_batch" => request_batch(ctx, args),
        "finish_session" => finish_session(ctx, args),
        "generate_prototype" => generate_prototype(ctx, args),
        "get_messages" => get_messages(ctx, args),
        "get_prototype_versions" => get_prototype_versions(ctx, args),
        "regenerate_stale" => regenerate_stale(ctx, args),
        "dismiss_stale" => dismiss_stale(ctx, args),
        "get_timeline" => get_timeline(ctx, args),
        "checkout_ref" => checkout_ref(ctx, args),
        "fork_branch" => fork_branch(ctx, args),
        "run_graph" => run_graph(ctx, args),
        "cancel_graph" => cancel_graph(ctx, args),
        "cancel_prototype" => cancel_prototype(ctx, args),
        "get_agent_log" => get_agent_log(ctx, args),
        "get_app_log" => get_app_log(ctx, args),
        "get_pipeline" => get_pipeline(ctx, args),
        "generate_spec" => generate_spec(ctx, args),
        "save_spec" => save_spec(ctx, args),
        "confirm_spec" => confirm_spec(ctx, args),
        "generate_tickets" => generate_tickets(ctx, args),
        "save_tickets" => save_tickets(ctx, args),
        "confirm_tickets" => confirm_tickets(ctx, args),
        "set_ticket_status" => set_ticket_status(ctx, args),
        "run_ticket" => run_ticket(ctx, args),
        other => Err(format!("unknown tool: {other}")),
    }
}

// ==================== helpers ====================

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing {key}"))
}

fn session_id(args: &Value) -> Result<String, String> {
    arg_str(args, "session_id").map(|s| s.to_string())
}

fn app(ctx: &McpContext) -> Result<tauri::AppHandle, String> {
    ctx.app
        .clone()
        .ok_or_else(|| "app handle unavailable (MCP running headless)".to_string())
}

/// Send a scheduler message carrying a oneshot reply channel and block for it.
/// Safe here because the MCP server thread is not inside a tokio runtime.
fn sched_call<T>(
    ctx: &McpContext,
    build: impl FnOnce(oneshot::Sender<Result<T, String>>) -> SchedulerMsg,
) -> Result<T, String> {
    let (tx, rx) = oneshot::channel();
    ctx.scheduler_tx
        .blocking_send(build(tx))
        .map_err(|_| "scheduler unavailable".to_string())?;
    rx.blocking_recv()
        .map_err(|_| "scheduler dropped reply".to_string())?
}

/// Fire-and-forget scheduler message (no reply channel).
fn sched_send(ctx: &McpContext, msg: SchedulerMsg) -> Result<(), String> {
    ctx.scheduler_tx
        .blocking_send(msg)
        .map_err(|_| "scheduler unavailable".to_string())
}

fn session_row(s: &crate::model::Session, active_id: Option<&str>) -> Value {
    let ws = workspace::session_workspace_dir(&s.id);
    let proto = workspace::session_prototype_dir(&s.id);
    json!({
        "id": s.id,
        "title": s.title,
        "role": s.role,
        "status": s.status.as_str(),
        "outline_status": s.outline_status.as_str(),
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

fn question_row(q: &crate::model::Question, node_titles: &std::collections::HashMap<String, String>) -> Value {
    json!({
        "id": q.id,
        "status": q.status.as_str(),
        "q_type": q.q_type.as_str(),
        "category": q.category.as_str(),
        "question": q.question,
        "context": q.context,
        "options": q.options.iter().map(|o| json!({
            "label": o.label,
            "description": o.description,
        })).collect::<Vec<_>>(),
        "recommended_option": q.recommended_option,
        "rationale": q.rationale,
        "answer": q.answer,
        "outline_node_id": q.outline_node_id,
        "outline_node_title": q.outline_node_id.as_ref().and_then(|id| node_titles.get(id)),
    })
}

/// Accept a plain string (open answer) or a tagged AnswerValue object.
fn parse_answer(v: &Value) -> Result<AnswerValue, String> {
    if let Some(s) = v.as_str() {
        return Ok(AnswerValue::Open { text: s.to_string() });
    }
    serde_json::from_value(v.clone()).map_err(|e| {
        format!("invalid answer: {e}; use a string or {{kind:\"choice\"|\"multi\"|\"open\", ...}}")
    })
}

// ==================== session tools ====================

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
    let id = session_id(args)?;
    let info = active_session::set_active(ctx.store.as_ref(), Some(&id), ctx.app.as_ref())?;
    log::info!("[mcp] active session → {id}");
    Ok(serde_json::to_value(&info).map_err(|e| e.to_string())?)
}

// ==================== interview loop ====================

fn create_session(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let title = arg_str(args, "title")?.to_string();
    let role = args
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("pm")
        .to_string();
    let initial_context = args
        .get("initial_context")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let app_handle = app(ctx)?;
    let session = sched_call(ctx, |reply| SchedulerMsg::CreateSession {
        title,
        role,
        initial_context,
        app_handle: app_handle.clone(),
        reply,
    })?;

    // Kick off the interview pipeline — for a fresh session this generates
    // the outline first (status → generating → draft) and waits for confirmation.
    sched_send(ctx, SchedulerMsg::StartInitialBatch {
        session_id: session.id.clone(),
        app_handle,
    })?;

    log::info!("[mcp] create_session → {}", session.id);
    Ok(json!({
        "session": session_row(&session, active_session::get_active_id(ctx.store.as_ref()).as_deref()),
        "next": "poll get_outline until status == 'draft', then save_outline (optional edits) + confirm_outline",
    }))
}

fn get_session_state(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let session = ctx
        .store
        .get_session(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session not found: {id}"))?;

    let nodes = ctx.store.get_outline_nodes(&id).unwrap_or_default();
    let questions = ctx.store.get_questions(&id).unwrap_or_default();

    let mut counts_map: std::collections::HashMap<&str, i64> = Default::default();
    for q in &questions {
        *counts_map.entry(q.status.as_str()).or_insert(0) += 1;
    }
    let counts: serde_json::Map<String, Value> = counts_map
        .into_iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect();

    let node_rows: Vec<Value> = nodes
        .iter()
        .map(|n| {
            let answered = questions
                .iter()
                .filter(|q| q.outline_node_id.as_deref() == Some(n.id.as_str())
                    && q.status == crate::model::QuestionStatus::Answered)
                .count();
            let pending = questions
                .iter()
                .filter(|q| q.outline_node_id.as_deref() == Some(n.id.as_str())
                    && matches!(q.status, crate::model::QuestionStatus::Ready | crate::model::QuestionStatus::Generating))
                .count();
            json!({
                "id": n.id,
                "title": n.title,
                "status": n.status.as_str(),
                "answered": answered,
                "pending": pending,
            })
        })
        .collect();

    Ok(json!({
        "session": session_row(&session, active_session::get_active_id(ctx.store.as_ref()).as_deref()),
        "outline": {
            "status": session.outline_status.as_str(),
            "nodes": node_rows,
        },
        "question_counts": counts,
        "hint": "draft → save_outline/confirm_outline；confirmed → answer/skip ready 题；全部节点 covered → finish_session",
    }))
}

fn get_outline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let info = sched_call(ctx, |reply| SchedulerMsg::GetOutline {
        session_id: id,
        reply,
    })?;
    Ok(serde_json::to_value(info).map_err(|e| e.to_string())?)
}

fn generate_outline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::GenerateOutline {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true, "status": "generating" }))
}

fn save_outline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let raw_nodes = args
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or("missing nodes array")?;

    let existing: std::collections::HashMap<String, crate::model::OutlineNode> = ctx
        .store
        .get_outline_nodes(&id)
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.id.clone(), n))
        .collect();

    let now = chrono::Utc::now().to_rfc3339();
    let mut nodes: Vec<OutlineNode> = Vec::with_capacity(raw_nodes.len());
    for (i, rn) in raw_nodes.iter().enumerate() {
        let title = rn
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or("node missing title")?
            .to_string();
        let node_id = rn
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let excluded = rn
            .get("excluded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let prev = node_id.as_ref().and_then(|nid| existing.get(nid));
        let status = if excluded {
            OutlineNodeStatus::Excluded
        } else {
            prev.map(|p| p.status).unwrap_or(OutlineNodeStatus::Pending)
        };

        nodes.push(OutlineNode {
            id: node_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            session_id: id.clone(),
            title,
            description: rn
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status,
            display_order: i as i32,
            created_at: prev.map(|p| p.created_at.clone()).unwrap_or_else(|| now.clone()),
        });
    }

    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::SaveOutline {
        session_id: id,
        nodes,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn confirm_outline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::ConfirmOutline {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true, "next": "poll list_questions?status=ready" }))
}

fn dismiss_outline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::DismissOutline {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn list_questions(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let filter = args.get("status").and_then(|v| v.as_str());
    let node_titles: std::collections::HashMap<String, String> = ctx
        .store
        .get_outline_nodes(&id)
        .unwrap_or_default()
        .into_iter()
        .map(|n| (n.id, n.title))
        .collect();

    let questions = sched_call(ctx, |reply| SchedulerMsg::GetQuestions {
        session_id: id,
        reply,
    })?;

    let items: Vec<Value> = questions
        .iter()
        .filter(|q| filter.map(|f| q.status.as_str() == f).unwrap_or(true))
        .map(|q| question_row(q, &node_titles))
        .collect();

    Ok(json!({ "count": items.len(), "questions": items }))
}

fn answer_question(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let question_id = arg_str(args, "question_id")?.to_string();
    let answer = parse_answer(args.get("answer").ok_or("missing answer")?)?;
    let app = app(ctx)?;

    sched_call(ctx, |reply| SchedulerMsg::AnswerQuestion {
        session_id: id,
        question_id,
        answer,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn skip_question(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let question_id = arg_str(args, "question_id")?.to_string();
    let app = app(ctx)?;

    sched_call(ctx, |reply| SchedulerMsg::SkipQuestion {
        session_id: id,
        question_id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn send_message(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let content = arg_str(args, "content")?.to_string();
    let app = app(ctx)?;

    sched_call(ctx, |reply| SchedulerMsg::SendMessage {
        session_id: id,
        content,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn request_batch(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::RequestBatch {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn finish_session(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    let (decisions, summary) = sched_call(ctx, |reply| SchedulerMsg::FinishSession {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({
        "ok": true,
        "decisions": decisions,
        "summary": summary,
    }))
}

fn generate_prototype(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let feedback = args
        .get("feedback")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let app = app(ctx)?;
    let result = sched_call(ctx, |reply| SchedulerMsg::GeneratePrototype {
        session_id: id,
        feedback,
        app_handle: app,
        reply,
    })?;
    Ok(serde_json::to_value(result).map_err(|e| e.to_string())?)
}

fn get_messages(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let messages = sched_call(ctx, |reply| SchedulerMsg::GetMessages {
        session_id: id,
        reply,
    })?;
    let items: Vec<Value> = messages
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "created_at": m.created_at,
            })
        })
        .collect();
    Ok(json!({ "count": items.len(), "messages": items }))
}

fn get_prototype_versions(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let versions = sched_call(ctx, |reply| SchedulerMsg::GetPrototypeVersions {
        session_id: id,
        reply,
    })?;
    Ok(json!({ "count": versions.len(), "versions": versions }))
}

fn regenerate_stale(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let question_ids: Vec<String> = args
        .get("question_ids")
        .and_then(|v| v.as_array())
        .ok_or("missing question_ids")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    if question_ids.is_empty() {
        return Err("question_ids is empty".to_string());
    }
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::RegenerateStale {
        session_id: id,
        question_ids,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn dismiss_stale(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let question_id = arg_str(args, "question_id")?.to_string();
    sched_call(ctx, |reply| SchedulerMsg::DismissStale {
        question_id,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

// ==================== timeline / branch ====================

fn get_timeline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let status = sched_call(ctx, |reply| SchedulerMsg::GetTimeline {
        session_id: id.clone(),
        reply,
    })?;
    let commits = sched_call(ctx, |reply| SchedulerMsg::ListTimelineCommits {
        session_id: id,
        limit: Some(80),
        reply,
    })?;
    Ok(json!({
        "status": status,
        "commits": commits,
    }))
}

fn checkout_ref(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let ref_name = arg_str(args, "ref")?.to_string();
    let app = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::CheckoutTimeline {
        session_id: id,
        ref_name,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn fork_branch(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let branch_name = arg_str(args, "branch_name")?.to_string();
    let from_sha = args
        .get("from_sha")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let app = app(ctx)?;
    let name = sched_call(ctx, |reply| SchedulerMsg::ForkTimeline {
        session_id: id,
        from_sha,
        branch_name,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true, "branch": name }))
}

fn run_graph(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app = app(ctx)?;
    let graph = sched_call(ctx, |reply| SchedulerMsg::RunIterationGraph {
        session_id: id,
        app_handle: app,
        reply,
    })?;
    Ok(json!({ "ok": true, "graph": graph }))
}

fn cancel_graph(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    sched_call(ctx, |reply| SchedulerMsg::CancelIterationGraph {
        session_id: id,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn cancel_prototype(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let _ = ctx;
    let id = session_id(args)?;
    let was_running = crate::agent::request_cancel(&id);
    Ok(json!({ "ok": true, "was_running": was_running }))
}

fn get_agent_log(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let _ = ctx;
    let id = session_id(args)?;
    let tail = args
        .get("tail")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let lines = crate::agent::get_agent_log(&id, tail);
    Ok(json!({ "count": lines.len(), "lines": lines }))
}

fn get_app_log(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let _ = ctx;
    let tail = args
        .get("tail")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let lines = crate::agent::get_app_log(tail);
    Ok(json!({ "count": lines.len(), "lines": lines }))
}

// ==================== pipeline: spec → tickets → branch map ====================

fn get_pipeline(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let info = sched_call(ctx, |reply| SchedulerMsg::GetPipeline {
        session_id: id,
        reply,
    })?;
    Ok(serde_json::to_value(&info).map_err(|e| e.to_string())?)
}

fn generate_spec(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::GenerateSpec {
        session_id: id,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true, "note": "spec 生成中，轮询 get_pipeline 直到 stage=spec_draft" }))
}

fn save_spec(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let spec = arg_str(args, "spec")?.to_string();
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::SaveSpec {
        session_id: id,
        spec,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn confirm_spec(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::ConfirmSpec {
        session_id: id,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true, "note": "spec 已确认并写入 docs/spec.md；tickets 生成中" }))
}

fn generate_tickets(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::GenerateTickets {
        session_id: id,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true, "note": "tickets 生成中，轮询 get_pipeline" }))
}

fn save_tickets(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let arr = args
        .get("tickets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing tickets array".to_string())?;
    let sid = id.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let mut tickets = Vec::new();
    for (i, t) in arr.iter().enumerate() {
        let tid = t
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("tickets[{i}].id missing"))?
            .to_string();
        tickets.push(crate::model::Ticket {
            id: tid,
            session_id: sid.clone(),
            title: t
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("tickets[{i}].title missing"))?
                .to_string(),
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status: t
                .get("status")
                .and_then(|v| v.as_str())
                .map(crate::model::TicketStatus::from_str)
                .unwrap_or(crate::model::TicketStatus::Pending),
            depends_on: t
                .get("depends_on")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            branches: t
                .get("branches")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            display_order: t
                .get("display_order")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32)
                .unwrap_or(i as i32),
            created_at: now.clone(),
        });
    }
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::SaveTickets {
        session_id: id,
        tickets,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn confirm_tickets(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::ConfirmTickets {
        session_id: id,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true, "note": "stage=developing，可用 run_ticket 开开发线" }))
}

fn set_ticket_status(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let tid = arg_str(args, "ticket_id")?.to_string();
    let status = crate::model::TicketStatus::from_str(arg_str(args, "status")?);
    let app_handle = app(ctx)?;
    sched_call(ctx, |reply| SchedulerMsg::SetTicketStatus {
        ticket_id: tid,
        status,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true }))
}

fn run_ticket(ctx: &McpContext, args: &Value) -> Result<Value, String> {
    let id = session_id(args)?;
    let tid = arg_str(args, "ticket_id")?.to_string();
    let rounds = args.get("rounds").and_then(|v| v.as_i64()).map(|n| n as i32).unwrap_or(3);
    let branch_name = args.get("branch_name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let from_sha = args.get("from_sha").and_then(|v| v.as_str()).map(|s| s.to_string());
    let feedback = args.get("feedback").and_then(|v| v.as_str()).map(|s| s.to_string());
    let app_handle = app(ctx)?;
    let branch = sched_call(ctx, |reply| SchedulerMsg::RunTicket {
        session_id: id,
        ticket_id: tid,
        rounds,
        branch_name,
        from_sha,
        feedback,
        app_handle,
        reply,
    })?;
    Ok(json!({ "ok": true, "branch": branch, "note": "开发线已启动，用 get_timeline / get_agent_log 观察进度" }))
}
