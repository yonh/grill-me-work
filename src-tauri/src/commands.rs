use crate::model::*;
use crate::scheduler::SchedulerMsg;
use crate::store::Store;
use tauri::{AppHandle, State};
use tokio::sync::oneshot;

/// Handle to the scheduler actor, stored in Tauri's managed state.
pub struct ActorHandle {
    pub tx: tokio::sync::mpsc::Sender<SchedulerMsg>,
}

#[tauri::command]
pub async fn create_session(
    title: String,
    role: String,
    initial_context: Option<String>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<Session, String> {
    log::info!("[cmd] create_session: title=\"{}\", role=\"{}\", has_context={}", title, role, initial_context.is_some());
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::CreateSession {
            title,
            role,
            initial_context,
            app_handle: app_handle.clone(),
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    let session = rx
        .await
        .map_err(|_| "scheduler actor dropped reply".to_string())??;
    log::info!("[cmd] create_session OK: session_id={}", session.id);

    // Trigger initial batch generation
    log::info!("[cmd] sending StartInitialBatch for session={}", session.id);
    state
        .tx
        .send(SchedulerMsg::StartInitialBatch {
            session_id: session.id.clone(),
            app_handle,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;

    Ok(session)
}

#[tauri::command]
pub async fn delete_session(
    id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::DeleteSession {
            id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_sessions(
    state: State<'_, ActorHandle>,
) -> Result<Vec<Session>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetSessions { reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_session(
    id: String,
    state: State<'_, ActorHandle>,
) -> Result<Option<Session>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetSession { id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_questions(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<Vec<Question>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetQuestions { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn answer_question(
    session_id: String,
    question_id: String,
    answer: AnswerValue,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::AnswerQuestion {
            session_id,
            question_id,
            answer,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn skip_question(
    session_id: String,
    question_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SkipQuestion {
            session_id,
            question_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn regenerate_stale(
    session_id: String,
    question_ids: Vec<String>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    log::info!("[cmd] regenerate_stale: session={}, {} questions", session_id, question_ids.len());
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::RegenerateStale {
            session_id,
            question_ids,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn dismiss_stale(
    question_id: String,
    state: State<'_, ActorHandle>,
) -> Result<(), String> {
    log::info!("[cmd] dismiss_stale: question={}", question_id);
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::DismissStale {
            question_id,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn finish_session(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(Vec<DecisionEntry>, String), String> {
    log::info!("[cmd] finish_session: session={}", session_id);
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::FinishSession {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    let (entries, summary) = rx
        .await
        .map_err(|_| "scheduler actor dropped reply".to_string())??;
    log::info!("[cmd] finish_session OK: {} decisions, summary {} chars", entries.len(), summary.len());
    Ok((entries, summary))
}

#[tauri::command]
pub async fn generate_prototype(
    session_id: String,
    feedback: Option<String>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<crate::scheduler::PrototypeGenResult, String> {
    log::info!("[cmd] generate_prototype: session={}, feedback={}", session_id, feedback.as_ref().map(|s| s.len()).unwrap_or(0));
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GeneratePrototype {
            session_id,
            feedback,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    let result = rx
        .await
        .map_err(|_| "scheduler actor dropped reply".to_string())??;
    log::info!("[cmd] generate_prototype OK: v{}", result.version);
    Ok(result)
}

/// Ensure the local preview server is running and return its base URL.
#[tauri::command]
pub async fn get_prototype_preview_url(session_id: String) -> Result<String, String> {
    crate::prototype::ensure_preview_server(&session_id).map_err(|e| e.to_string())
}

/// List files in the session prototype directory.
#[tauri::command]
pub async fn get_prototype_files(
    session_id: String,
) -> Result<Vec<crate::prototype::PrototypeFile>, String> {
    Ok(crate::prototype::read_snapshot(&session_id).files)
}

/// Read one prototype file.
#[tauri::command]
pub async fn read_prototype_file(
    session_id: String,
    path: String,
) -> Result<String, String> {
    crate::prototype::read_file(&session_id, &path)
        .ok_or_else(|| format!("file not found: {path}"))
}

/// List available coding-agent CLIs on this machine.
#[tauri::command]
pub async fn list_agent_tools() -> Result<Vec<crate::agent::AgentToolStatus>, String> {
    Ok(crate::agent::list_tools())
}

#[tauri::command]
pub async fn get_prototype_versions(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<Vec<crate::model::PrototypeVersion>, String> {
    log::info!("[cmd] get_prototype_versions: session={}", session_id);
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetPrototypeVersions {
            session_id,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn send_message(
    session_id: String,
    content: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    log::info!("[cmd] send_message: session={}, content_len={}", session_id, content.len());
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SendMessage {
            session_id,
            content,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_messages(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<Vec<crate::model::ChatMessage>, String> {
    log::info!("[cmd] get_messages: session={}", session_id);
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetMessages {
            session_id,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn export_session(
    session_id: String,
    format: String,
    state: State<'_, ActorHandle>,
) -> Result<String, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ExportSession {
            session_id,
            format,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, ActorHandle>,
) -> Result<Settings, String> {
    log::info!("[cmd] get_settings");
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetSettings { reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    let result = rx
        .await
        .map_err(|_| "scheduler actor dropped reply".to_string())?;
    log::info!("[cmd] get_settings result: {:?}", result.as_ref().map(|s| format!("base_url={}, model={}, key_set={}", s.base_url, s.model_name, !s.api_key.is_empty())).map_err(|e| e.clone()));
    result
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    state: State<'_, ActorHandle>,
) -> Result<(), String> {
    log::info!("[cmd] save_settings: base_url={}, model={}, key_set={}", settings.base_url, settings.model_name, !settings.api_key.is_empty());
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SaveSettings { settings, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

// --- Interview outline ---

#[tauri::command]
pub async fn get_outline(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<crate::scheduler::OutlineInfo, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetOutline { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn generate_outline(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GenerateOutline {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn save_outline(
    session_id: String,
    nodes: Vec<OutlineNode>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SaveOutline {
            session_id,
            nodes,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn confirm_outline(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ConfirmOutline {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn dismiss_outline(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::DismissOutline {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

/// Ask for one more question batch even after the interview saturated.
#[tauri::command]
pub async fn request_batch(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::RequestBatch {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

// --- Iteration graph / timeline ---

#[tauri::command]
pub async fn get_timeline(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<crate::git::GitStatus, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetTimeline { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn init_timeline(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<crate::git::GitStatus, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::InitTimeline {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_prototype_dir_path(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<String, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetPrototypeDirPath { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn open_prototype_dir(
    session_id: String,
    app_handle: AppHandle,
) -> Result<String, String> {
    let ws = crate::workspace::session_workspace_dir(&session_id);
    std::fs::create_dir_all(&ws).map_err(|e| e.to_string())?;
    let path = ws.to_string_lossy().to_string();
    use tauri_plugin_opener::OpenerExt;
    app_handle
        .opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| format!("打开目录失败: {e}"))?;
    Ok(path)
}

#[tauri::command]
pub async fn list_timeline_commits(
    session_id: String,
    limit: Option<usize>,
    state: State<'_, ActorHandle>,
) -> Result<Vec<crate::git::TimelineCommit>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ListTimelineCommits {
            session_id,
            limit,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn checkout_timeline(
    session_id: String,
    ref_name: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::CheckoutTimeline {
            session_id,
            ref_name,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn fork_timeline(
    session_id: String,
    from_sha: Option<String>,
    branch_name: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ForkTimeline {
            session_id,
            from_sha,
            branch_name,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_iteration_graph(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<crate::graph::IterationGraph, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetIterationGraph { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn save_iteration_graph(
    session_id: String,
    graph: crate::graph::IterationGraph,
    state: State<'_, ActorHandle>,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SaveIterationGraph {
            session_id,
            graph,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn run_iteration_graph(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<crate::graph::IterationGraph, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::RunIterationGraph {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn cancel_iteration_graph(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::CancelIterationGraph { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

/// MCP server endpoint info for the UI / debugging.
#[tauri::command]
pub async fn get_mcp_info() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "default_port": crate::mcp::DEFAULT_MCP_PORT,
        "tools": crate::mcp::tools::tool_names(),
        "endpoint_hint": format!("http://127.0.0.1:{}/mcp", crate::mcp::DEFAULT_MCP_PORT),
        "health_hint": format!("http://127.0.0.1:{}/health", crate::mcp::DEFAULT_MCP_PORT),
    }))
}

/// Currently open project (shared with MCP).
#[tauri::command]
pub async fn get_active_session(
    store: State<'_, std::sync::Arc<dyn Store>>,
) -> Result<crate::active_session::ActiveSessionInfo, String> {
    Ok(crate::active_session::describe(store.inner().as_ref()))
}

/// Switch the currently open project; UI listens to `active_session_changed`.
#[tauri::command]
pub async fn set_active_session(
    session_id: Option<String>,
    store: State<'_, std::sync::Arc<dyn Store>>,
    app_handle: AppHandle,
) -> Result<crate::active_session::ActiveSessionInfo, String> {
    crate::active_session::set_active(
        store.inner().as_ref(),
        session_id.as_deref(),
        Some(&app_handle),
    )
}

// --- Pipeline: spec + tickets ---

#[tauri::command]
pub async fn get_pipeline(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<crate::scheduler::PipelineInfo, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetPipeline { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn generate_spec(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GenerateSpec {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn save_spec(
    session_id: String,
    spec: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SaveSpec {
            session_id,
            spec,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn confirm_spec(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ConfirmSpec {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn generate_tickets(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GenerateTickets {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn save_tickets(
    session_id: String,
    tickets: Vec<crate::model::Ticket>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SaveTickets {
            session_id,
            tickets,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn confirm_tickets(
    session_id: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ConfirmTickets {
            session_id,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn set_ticket_status(
    ticket_id: String,
    status: String,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::SetTicketStatus {
            ticket_id,
            status: crate::model::TicketStatus::from_str(&status),
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn run_ticket(
    session_id: String,
    ticket_id: String,
    rounds: Option<i32>,
    branch_name: Option<String>,
    from_sha: Option<String>,
    feedback: Option<String>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::RunTicket {
            session_id,
            ticket_id,
            rounds: rounds.unwrap_or(3),
            branch_name,
            from_sha,
            feedback,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

// --- Development rounds ---

#[tauri::command]
pub async fn start_round(
    session_id: String,
    title: String,
    goal: Option<String>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<crate::model::Round, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::StartRound {
            session_id,
            title,
            goal,
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn archive_round(
    session_id: String,
    force: Option<bool>,
    state: State<'_, ActorHandle>,
    app_handle: AppHandle,
) -> Result<crate::model::Round, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ArchiveRound {
            session_id,
            force: force.unwrap_or(false),
            app_handle,
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn list_rounds(
    session_id: String,
    state: State<'_, ActorHandle>,
) -> Result<Vec<crate::model::Round>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ListRounds { session_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn get_round_archive(
    round_id: String,
    state: State<'_, ActorHandle>,
) -> Result<crate::model::RoundArchive, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::GetRoundArchive { round_id, reply })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}

#[tauri::command]
pub async fn list_activity(
    session_id: String,
    limit: Option<i64>,
    state: State<'_, ActorHandle>,
) -> Result<Vec<crate::model::Activity>, String> {
    let (reply, rx) = oneshot::channel();
    state
        .tx
        .send(SchedulerMsg::ListActivity {
            session_id,
            limit: limit.unwrap_or(200),
            reply,
        })
        .await
        .map_err(|_| "scheduler actor disconnected".to_string())?;
    rx.await
        .map_err(|_| "scheduler actor dropped reply".to_string())?
}
