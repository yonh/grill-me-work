pub mod batch;

use crate::events::*;
use crate::export::{export_session, ExportFormat};
use crate::llm::openai::OpenAIClient;
use crate::llm::prompt;
use crate::llm::{LlmError, QuestionSource};
use crate::model::*;
use crate::store::Store;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

pub enum SchedulerMsg {
    CreateSession {
        title: String,
        role: String,
        initial_context: Option<String>,
        reply: oneshot::Sender<Result<Session, String>>,
    },
    DeleteSession {
        id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    GetSessions {
        reply: oneshot::Sender<Result<Vec<Session>, String>>,
    },
    GetSession {
        id: String,
        reply: oneshot::Sender<Result<Option<Session>, String>>,
    },
    GetQuestions {
        session_id: String,
        reply: oneshot::Sender<Result<Vec<Question>, String>>,
    },
    AnswerQuestion {
        session_id: String,
        question_id: String,
        answer: AnswerValue,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SkipQuestion {
        session_id: String,
        question_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RegenerateStale {
        session_id: String,
        question_ids: Vec<String>,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    DismissStale {
        question_id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    FinishSession {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(Vec<DecisionEntry>, String), String>>,
    },
    GeneratePrototype {
        session_id: String,
        feedback: Option<String>,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<PrototypeGenResult, String>>,
    },
    GetPrototypeVersions {
        session_id: String,
        reply: oneshot::Sender<Result<Vec<crate::model::PrototypeVersion>, String>>,
    },
    /// Internal: batch task completed, decrement active_batches
    BatchDone {
        session_id: String,
        questions_generated: i32,
        app_handle: AppHandle,
    },
    SendMessage {
        session_id: String,
        content: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    GetMessages {
        session_id: String,
        reply: oneshot::Sender<Result<Vec<crate::model::ChatMessage>, String>>,
    },
    ExportSession {
        session_id: String,
        format: String,
        reply: oneshot::Sender<Result<String, String>>,
    },
    GetSettings {
        reply: oneshot::Sender<Result<Settings, String>>,
    },
    SaveSettings {
        settings: Settings,
        reply: oneshot::Sender<Result<(), String>>,
    },
    StartInitialBatch {
        session_id: String,
        app_handle: AppHandle,
    },
    // --- Interview outline ---
    GetOutline {
        session_id: String,
        reply: oneshot::Sender<Result<OutlineInfo, String>>,
    },
    GenerateOutline {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    SaveOutline {
        session_id: String,
        nodes: Vec<OutlineNode>,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ConfirmOutline {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    DismissOutline {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Manually request one more question batch (clears the saturated flag).
    RequestBatch {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Internal: outline generation task finished (clears outline_generating).
    OutlineDone {
        session_id: String,
    },
    // --- Iteration graph / timeline ---
    GetTimeline {
        session_id: String,
        reply: oneshot::Sender<Result<crate::git::GitStatus, String>>,
    },
    /// Manually init git for an existing prototype (old sessions).
    InitTimeline {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<crate::git::GitStatus, String>>,
    },
    GetPrototypeDirPath {
        session_id: String,
        reply: oneshot::Sender<Result<String, String>>,
    },
    ListTimelineCommits {
        session_id: String,
        limit: Option<usize>,
        reply: oneshot::Sender<Result<Vec<crate::git::TimelineCommit>, String>>,
    },
    CheckoutTimeline {
        session_id: String,
        ref_name: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ForkTimeline {
        session_id: String,
        from_sha: Option<String>,
        branch_name: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<String, String>>,
    },
    GetIterationGraph {
        session_id: String,
        reply: oneshot::Sender<Result<crate::graph::IterationGraph, String>>,
    },
    SaveIterationGraph {
        session_id: String,
        graph: crate::graph::IterationGraph,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RunIterationGraph {
        session_id: String,
        app_handle: AppHandle,
        reply: oneshot::Sender<Result<crate::graph::IterationGraph, String>>,
    },
    CancelIterationGraph {
        session_id: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

/// Per-session scheduler state.
#[derive(Default)]
struct SessionState {
    active_batches: i32,
    last_generation_time: Option<std::time::Instant>,
    /// The LLM returned 0 questions — interview is saturated; stop auto-refilling
    /// until the user changes the outline or explicitly requests more.
    saturated: bool,
    /// An outline generation task is in flight (in-memory; clears on OutlineDone).
    outline_generating: bool,
}

/// Outline info returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutlineInfo {
    pub status: OutlineStatus,
    pub nodes: Vec<OutlineNode>,
}

/// The scheduler actor's shared state.
struct SchedulerState {
    sessions: HashMap<String, SessionState>,
}

impl SchedulerState {
    fn get_or_create(&mut self, session_id: &str) -> &mut SessionState {
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(SessionState::default)
    }
}

pub async fn run_actor(store: Arc<dyn Store>, mut rx: mpsc::Receiver<SchedulerMsg>, tx: mpsc::Sender<SchedulerMsg>) {
    let store: Arc<dyn Store> = store;
    let mut state = SchedulerState {
        sessions: HashMap::new(),
    };

    log::info!("[scheduler] actor started, waiting for messages...");

    let pending_prototype: Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
        Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));

    while let Some(msg) = rx.recv().await {
        log::info!("[scheduler] received message: {}", msg_name(&msg));
        match msg {
            SchedulerMsg::CreateSession {
                title,
                role,
                initial_context,
                reply,
            } => {
                log::info!("[scheduler] CreateSession: title=\"{}\", role=\"{}\", has_context={}", title, role, initial_context.is_some());
                let result = handle_create_session(&*store, &title, &role, initial_context);
                match &result {
                    Ok(session) => log::info!("[scheduler] CreateSession OK: session_id={}", session.id),
                    Err(e) => log::error!("[scheduler] CreateSession FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::DeleteSession { id, reply } => {
                log::info!("[scheduler] DeleteSession: id={}", id);
                let result = store.delete_session(&id).map_err(|e| e.to_string());
                state.sessions.remove(&id);
                if result.is_ok() {
                    crate::active_session::clear_if_matches(store.as_ref(), &id, None);
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetSessions { reply } => {
                let result = store.get_sessions().map_err(|e| e.to_string());
                match &result {
                    Ok(sessions) => log::info!("[scheduler] GetSessions OK: {} sessions", sessions.len()),
                    Err(e) => log::error!("[scheduler] GetSessions FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetSession { id, reply } => {
                let result = store.get_session(&id).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            SchedulerMsg::GetQuestions { session_id, reply } => {
                let result = store.get_questions(&session_id).map_err(|e| e.to_string());
                match &result {
                    Ok(qs) => log::info!("[scheduler] GetQuestions OK: session={}, {} questions", session_id, qs.len()),
                    Err(e) => log::error!("[scheduler] GetQuestions FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::AnswerQuestion {
                session_id,
                question_id,
                answer,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] AnswerQuestion: session={}, question={}, answer={:?}", session_id, question_id, answer);
                let result = handle_answer_question(
                    store.clone(),
                    &mut state,
                    &session_id,
                    &question_id,
                    &answer,
                    &app_handle,
                    tx.clone(),
                );
                match &result {
                    Ok(()) => {
                        log::info!("[scheduler] AnswerQuestion OK");
                        // Prototype-first: every answer incrementally updates the prototype
                        let note = match store.get_question(&question_id) {
                            Ok(Some(q)) => {
                                let ans = answer.to_human_string();
                                let short_q: String = q.question.chars().take(40).collect();
                                let short_a: String = ans.chars().take(40).collect();
                                format!("{short_q} → {short_a}")
                            }
                            _ => format!("回答了 {}", question_id),
                        };
                        spawn_auto_prototype_update(
                            store.clone(),
                            &session_id,
                            note,
                            app_handle.clone(),
                            pending_prototype.clone(),
                        );
                    }
                    Err(e) => log::error!("[scheduler] AnswerQuestion FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::SkipQuestion {
                session_id,
                question_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] SkipQuestion: session={}, question={}", session_id, question_id);
                let result = handle_skip_question(
                    store.clone(),
                    &mut state,
                    &session_id,
                    &question_id,
                    &app_handle,
                    tx.clone(),
                );
                let _ = reply.send(result);
            }
            SchedulerMsg::RegenerateStale {
                session_id,
                question_ids,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] RegenerateStale: session={}, {} questions", session_id, question_ids.len());
                let result = handle_regenerate_stale(
                    store.clone(),
                    &mut state,
                    &session_id,
                    &question_ids,
                    &app_handle,
                    tx.clone(),
                );
                let _ = reply.send(result);
            }
            SchedulerMsg::DismissStale {
                question_id,
                reply,
            } => {
                log::info!("[scheduler] DismissStale: question={}", question_id);
                let result = store
                    .update_question_status(&question_id, QuestionStatus::Ready)
                    .map_err(|e| e.to_string());
                match &result {
                    Ok(()) => log::info!("[scheduler] DismissStale OK: question={} → ready", question_id),
                    Err(e) => log::error!("[scheduler] DismissStale FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::FinishSession {
                session_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] FinishSession: session={}", session_id);
                let result =
                    handle_finish_session(&*store, &mut state, &session_id, &app_handle).await;
                match &result {
                    Ok((entries, summary)) => log::info!("[scheduler] FinishSession OK: {} decisions, summary_len={}", entries.len(), summary.len()),
                    Err(e) => log::error!("[scheduler] FinishSession FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GeneratePrototype {
                session_id,
                feedback,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] GeneratePrototype: session={}, feedback={}", session_id, feedback.as_deref().map(|s| s.len()).unwrap_or(0));
                let trigger = if feedback.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
                    "用户反馈修改".to_string()
                } else {
                    "用户手动触发生成".to_string()
                };
                let result =
                    handle_generate_prototype(&*store, &session_id, feedback.as_deref(), Some(&trigger), &app_handle).await;
                match &result {
                    Ok(r) => log::info!("[scheduler] GeneratePrototype OK: v{}, {} files", r.version, r.file_count),
                    Err(e) => log::error!("[scheduler] GeneratePrototype FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetPrototypeVersions {
                session_id,
                reply,
            } => {
                let result = store
                    .get_prototype_versions(&session_id)
                    .map_err(|e| e.to_string());
                match &result {
                    Ok(versions) => log::info!("[scheduler] GetPrototypeVersions OK: {} versions", versions.len()),
                    Err(e) => log::error!("[scheduler] GetPrototypeVersions FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::BatchDone {
                session_id,
                questions_generated,
                app_handle,
            } => {
                // Decrement active_batches
                if let Some(session_state) = state.sessions.get_mut(&session_id) {
                    if session_state.active_batches > 0 {
                        session_state.active_batches -= 1;
                    }
                    log::info!(
                        "[scheduler] BatchDone: session={}, generated={}, active_batches now={}",
                        session_id, questions_generated, session_state.active_batches
                    );
                }

                // A batch just finished — node coverage may have changed.
                refresh_outline_coverage(&*store, &session_id, &app_handle);

                // If LLM returned 0 questions, it may indicate the interview is complete
                if questions_generated == 0 {
                    log::info!("[scheduler] BatchDone: 0 questions generated, marking saturated + emitting interview_may_complete");
                    state.get_or_create(&session_id).saturated = true;
                    let _ = app_handle.emit(
                        "interview_may_complete",
                        serde_json::json!({ "session_id": session_id }),
                    );
                } else {
                    // Re-check water levels after batch completion
                    check_water_levels_and_trigger(
                        store.clone(),
                        &mut state,
                        &session_id,
                        &[],
                        &app_handle,
                        tx.clone(),
                    );
                }
            }
            SchedulerMsg::SendMessage {
                session_id,
                content,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] SendMessage: session={}, content_len={}", session_id, content.len());
                let result = handle_send_message(&*store, &session_id, &content, &app_handle).await;
                match &result {
                    Ok(()) => log::info!("[scheduler] SendMessage OK"),
                    Err(e) => log::error!("[scheduler] SendMessage FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetMessages {
                session_id,
                reply,
            } => {
                let result = store.get_messages(&session_id).map_err(|e| e.to_string());
                match &result {
                    Ok(msgs) => log::info!("[scheduler] GetMessages OK: {} messages", msgs.len()),
                    Err(e) => log::error!("[scheduler] GetMessages FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::ExportSession {
                session_id,
                format,
                reply,
            } => {
                log::info!("[scheduler] ExportSession: session={}, format={}", session_id, format);
                let fmt = ExportFormat::from_str(&format);
                let result = export_session(&*store, &session_id, &fmt, None)
                    .map_err(|e| e.to_string());
                match &result {
                    Ok(content) => log::info!("[scheduler] ExportSession OK: {} chars", content.len()),
                    Err(e) => log::error!("[scheduler] ExportSession FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetSettings { reply } => {
                let result = store.get_all_settings().map_err(|e| e.to_string());
                match &result {
                    Ok(s) => log::info!("[scheduler] GetSettings OK: base_url={}, model={}, api_key_set={}, batch_size={}", s.base_url, s.model_name, !s.api_key.is_empty(), s.batch_size),
                    Err(e) => log::error!("[scheduler] GetSettings FAILED: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::SaveSettings { settings, reply } => {
                log::info!("[scheduler] SaveSettings: base_url={}, model={}, api_key_set={}", settings.base_url, settings.model_name, !settings.api_key.is_empty());
                let result = store.save_settings(&settings).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            SchedulerMsg::StartInitialBatch {
                session_id,
                app_handle,
            } => {
                log::info!("[scheduler] StartInitialBatch: session={}", session_id);
                // Outline-first flow: a fresh session generates the interview
                // outline and waits for user confirmation before any questions.
                let outline_status = store
                    .get_session(&session_id)
                    .ok()
                    .flatten()
                    .map(|s| s.outline_status)
                    .unwrap_or(OutlineStatus::None);
                match outline_status {
                    OutlineStatus::None => {
                        state.get_or_create(&session_id).outline_generating = true;
                        spawn_outline_generation(
                            store.clone(),
                            &session_id,
                            &app_handle,
                            tx.clone(),
                        );
                    }
                    OutlineStatus::Confirmed => {
                        check_water_levels_and_trigger(
                            store.clone(),
                            &mut state,
                            &session_id,
                            &[],
                            &app_handle,
                            tx.clone(),
                        );
                    }
                    // generating / draft → wait for the user
                    _ => {}
                }
            }
            SchedulerMsg::GetOutline { session_id, reply } => {
                let status = store
                    .get_session(&session_id)
                    .ok()
                    .flatten()
                    .map(|s| s.outline_status)
                    .unwrap_or(OutlineStatus::None);
                let nodes = store.get_outline_nodes(&session_id).unwrap_or_default();
                let _ = reply.send(Ok(OutlineInfo { status, nodes }));
            }
            SchedulerMsg::GenerateOutline {
                session_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] GenerateOutline: session={}", session_id);
                // Skip only while a task is actually in flight (in-memory flag —
                // a stale 'generating' status after restart stays retryable).
                let in_flight = state.get_or_create(&session_id).outline_generating;
                if !in_flight {
                    state.get_or_create(&session_id).outline_generating = true;
                    spawn_outline_generation(
                        store.clone(),
                        &session_id,
                        &app_handle,
                        tx.clone(),
                    );
                }
                let _ = reply.send(Ok(()));
            }
            SchedulerMsg::OutlineDone { session_id } => {
                state.get_or_create(&session_id).outline_generating = false;
            }
            SchedulerMsg::SaveOutline {
                session_id,
                nodes,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] SaveOutline: session={}, {} nodes", session_id, nodes.len());
                let result = handle_save_outline(
                    store.clone(),
                    &mut state,
                    &session_id,
                    nodes,
                    &app_handle,
                    tx.clone(),
                );
                let _ = reply.send(result);
            }
            SchedulerMsg::ConfirmOutline {
                session_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] ConfirmOutline: session={}", session_id);
                let result = store
                    .set_session_outline_status(&session_id, OutlineStatus::Confirmed)
                    .map_err(|e| e.to_string());
                if result.is_ok() {
                    state.get_or_create(&session_id).saturated = false;
                    emit_outline_updated(&*store, &session_id, &app_handle);
                    // Kick off the first scoped batch
                    check_water_levels_and_trigger(
                        store.clone(),
                        &mut state,
                        &session_id,
                        &[],
                        &app_handle,
                        tx.clone(),
                    );
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::DismissOutline {
                session_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] DismissOutline: session={} → free mode", session_id);
                // Free mode: no outline scoping, but saturation still applies.
                let result = store
                    .replace_outline_nodes(&session_id, &[])
                    .and_then(|_| {
                        store.set_session_outline_status(&session_id, OutlineStatus::Confirmed)
                    })
                    .map_err(|e| e.to_string());
                if result.is_ok() {
                    state.get_or_create(&session_id).saturated = false;
                    emit_outline_updated(&*store, &session_id, &app_handle);
                    check_water_levels_and_trigger(
                        store.clone(),
                        &mut state,
                        &session_id,
                        &[],
                        &app_handle,
                        tx.clone(),
                    );
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::RequestBatch {
                session_id,
                app_handle,
                reply,
            } => {
                log::info!("[scheduler] RequestBatch: session={}", session_id);
                state.get_or_create(&session_id).saturated = false;
                spawn_batch_generation(
                    store.clone(),
                    &mut state,
                    &session_id,
                    &[],
                    &app_handle,
                    tx.clone(),
                );
                let _ = reply.send(Ok(()));
            }
            SchedulerMsg::GetTimeline { session_id, reply } => {
                let dir = crate::workspace::session_workspace_dir(&session_id);
                // Old sessions: if files already exist but no repo, bootstrap git so
                // the timeline isn't stuck until the next generation.
                let mut st = crate::git::status(&dir);
                if st.available && !st.initialized && st.has_files {
                    match crate::git::ensure_repo(&dir)
                        .and_then(|_| crate::git::commit_all(&dir, "v0 · 初始化既有原型"))
                    {
                        Ok(sha) => {
                            log::info!("[timeline] auto-init git for existing prototype: {sha}");
                            st = crate::git::status(&dir);
                        }
                        Err(e) => log::warn!("[timeline] auto-init failed: {e}"),
                    }
                }
                let _ = reply.send(Ok(st));
            }
            SchedulerMsg::InitTimeline {
                session_id,
                app_handle,
                reply,
            } => {
                let dir = crate::workspace::session_workspace_dir(&session_id);
                let result = (|| -> Result<crate::git::GitStatus, String> {
                    let _ = crate::workspace::ensure_workspace(&session_id)
                        .map_err(|e| e.to_string())?;
                    crate::git::ensure_repo(&dir)?;
                    let st0 = crate::git::status(&dir);
                    if st0.has_files {
                        crate::git::commit_all(&dir, "v0 · 初始化既有原型")?;
                    }
                    Ok(crate::git::status(&dir))
                })();
                if let Ok(st) = &result {
                    let _ = app_handle.emit(
                        EVENT_TIMELINE_UPDATED,
                        TimelineUpdatedPayload {
                            session_id: session_id.clone(),
                            branch: st.branch.clone(),
                            head: st.head.clone(),
                            commit_count: 0,
                        },
                    );
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetPrototypeDirPath { session_id, reply } => {
                let dir = crate::workspace::ensure_workspace(&session_id)
                    .map_err(|e| e.to_string());
                let _ = reply.send(dir.map(|p| p.to_string_lossy().to_string()));
            }
            SchedulerMsg::ListTimelineCommits {
                session_id,
                limit,
                reply,
            } => {
                let dir = crate::workspace::session_workspace_dir(&session_id);
                let result = crate::git::log_graph(&dir, limit.unwrap_or(80));
                match &result {
                    Ok(c) => log::info!("[scheduler] ListTimelineCommits: {} commits", c.len()),
                    Err(e) => log::warn!("[scheduler] ListTimelineCommits: {}", e),
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::CheckoutTimeline {
                session_id,
                ref_name,
                app_handle,
                reply,
            } => {
                let dir = crate::workspace::session_workspace_dir(&session_id);
                let result = crate::git::checkout(&dir, &ref_name).map_err(|e| e.to_string());
                if result.is_ok() {
                    let _ = crate::prototype::ensure_preview_server(&session_id);
                    let st = crate::git::status(&dir);
                    let _ = app_handle.emit(
                        EVENT_TIMELINE_UPDATED,
                        TimelineUpdatedPayload {
                            session_id: session_id.clone(),
                            branch: st.branch,
                            head: st.head,
                            commit_count: 0,
                        },
                    );
                    let _ = app_handle.emit(
                        EVENT_PROTOTYPE_STATUS,
                        PrototypeStatusPayload {
                            session_id: session_id.clone(),
                            status: "done".to_string(),
                            message: Some(format!("已切换到 {ref_name}")),
                        },
                    );
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::ForkTimeline {
                session_id,
                from_sha,
                branch_name,
                app_handle,
                reply,
            } => {
                let dir = crate::workspace::session_workspace_dir(&session_id);
                let name = crate::git::sanitize_branch_name(&branch_name);
                let result = crate::git::checkout_new_branch(&dir, &name, from_sha.as_deref())
                    .map(|_| name.clone())
                    .map_err(|e| e.to_string());
                if result.is_ok() {
                    let st = crate::git::status(&dir);
                    let _ = app_handle.emit(
                        EVENT_TIMELINE_UPDATED,
                        TimelineUpdatedPayload {
                            session_id: session_id.clone(),
                            branch: st.branch,
                            head: st.head,
                            commit_count: 0,
                        },
                    );
                }
                let _ = reply.send(result);
            }
            SchedulerMsg::GetIterationGraph { session_id, reply } => {
                let result = load_or_default_graph(&store, &session_id);
                let _ = reply.send(result);
            }
            SchedulerMsg::SaveIterationGraph {
                session_id,
                graph,
                reply,
            } => {
                let mut g = graph;
                g.session_id = session_id.clone();
                g.updated_at = chrono::Utc::now().to_rfc3339();
                let json = match serde_json::to_string_pretty(&g) {
                    Ok(j) => j,
                    Err(e) => {
                        let _ = reply.send(Err(e.to_string()));
                        continue;
                    }
                };
                let result = crate::prototype::save_iteration_graph(&session_id, &json)
                    .map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            SchedulerMsg::RunIterationGraph {
                session_id,
                app_handle,
                reply,
            } => {
                let result = spawn_iteration_graph_run(store.clone(), &session_id, &app_handle);
                let _ = reply.send(result);
            }
            SchedulerMsg::CancelIterationGraph { session_id, reply } => {
                {
                    let mut guard = cancel_sessions().lock().unwrap();
                    guard.insert(session_id.clone());
                }
                let _ = reply.send(Ok(()));
            }
        }
    }
    log::warn!("[scheduler] actor stopped (channel closed)");
}

fn msg_name(msg: &SchedulerMsg) -> &'static str {
    match msg {
        SchedulerMsg::CreateSession { .. } => "CreateSession",
        SchedulerMsg::DeleteSession { .. } => "DeleteSession",
        SchedulerMsg::GetSessions { .. } => "GetSessions",
        SchedulerMsg::GetSession { .. } => "GetSession",
        SchedulerMsg::GetQuestions { .. } => "GetQuestions",
        SchedulerMsg::AnswerQuestion { .. } => "AnswerQuestion",
        SchedulerMsg::SkipQuestion { .. } => "SkipQuestion",
        SchedulerMsg::RegenerateStale { .. } => "RegenerateStale",
        SchedulerMsg::DismissStale { .. } => "DismissStale",
        SchedulerMsg::FinishSession { .. } => "FinishSession",
        SchedulerMsg::GeneratePrototype { .. } => "GeneratePrototype",
        SchedulerMsg::GetPrototypeVersions { .. } => "GetPrototypeVersions",
        SchedulerMsg::BatchDone { .. } => "BatchDone",
        SchedulerMsg::SendMessage { .. } => "SendMessage",
        SchedulerMsg::GetMessages { .. } => "GetMessages",
        SchedulerMsg::ExportSession { .. } => "ExportSession",
        SchedulerMsg::GetSettings { .. } => "GetSettings",
        SchedulerMsg::SaveSettings { .. } => "SaveSettings",
        SchedulerMsg::StartInitialBatch { .. } => "StartInitialBatch",
        SchedulerMsg::GetTimeline { .. } => "GetTimeline",
        SchedulerMsg::InitTimeline { .. } => "InitTimeline",
        SchedulerMsg::GetPrototypeDirPath { .. } => "GetPrototypeDirPath",
        SchedulerMsg::ListTimelineCommits { .. } => "ListTimelineCommits",
        SchedulerMsg::CheckoutTimeline { .. } => "CheckoutTimeline",
        SchedulerMsg::ForkTimeline { .. } => "ForkTimeline",
        SchedulerMsg::GetIterationGraph { .. } => "GetIterationGraph",
        SchedulerMsg::SaveIterationGraph { .. } => "SaveIterationGraph",
        SchedulerMsg::RunIterationGraph { .. } => "RunIterationGraph",
        SchedulerMsg::CancelIterationGraph { .. } => "CancelIterationGraph",
        SchedulerMsg::GetOutline { .. } => "GetOutline",
        SchedulerMsg::GenerateOutline { .. } => "GenerateOutline",
        SchedulerMsg::SaveOutline { .. } => "SaveOutline",
        SchedulerMsg::ConfirmOutline { .. } => "ConfirmOutline",
        SchedulerMsg::DismissOutline { .. } => "DismissOutline",
        SchedulerMsg::RequestBatch { .. } => "RequestBatch",
        SchedulerMsg::OutlineDone { .. } => "OutlineDone",
    }
}

fn handle_create_session(
    store: &dyn Store,
    title: &str,
    role: &str,
    initial_context: Option<String>,
) -> Result<Session, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.to_string(),
        initial_context,
        role: role.to_string(),
        status: SessionStatus::Active,
        summary: None,
        prototype_version: 0,
        outline_status: OutlineStatus::None,
        created_at: now.clone(),
        updated_at: now,
    };
    store
        .create_session(&session)
        .map_err(|e| e.to_string())?;
    Ok(session)
}

fn handle_answer_question(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    question_id: &str,
    answer: &AnswerValue,
    app_handle: &AppHandle,
    tx: mpsc::Sender<SchedulerMsg>,
) -> Result<(), String> {
    // 1. Get the question
    let question = store
        .get_question(question_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("question {} not found", question_id))?;

    log::info!("[answer] Processing: question=\"{}\", current_status={:?}", question.question, question.status);

    // 2. Update answer + status
    let new_version = question.answer_version + 1;
    store
        .update_question_answer(question_id, answer, new_version)
        .map_err(|e| e.to_string())?;
    log::info!("[answer] Updated answer in DB: version={}", new_version);

    // 3. Update decision summary
    let entry = DecisionEntry {
        question_id: question_id.to_string(),
        question: question.question.clone(),
        answer: answer.to_human_string(),
        rationale: question.rationale.clone(),
        category: question.category.clone(),
    };
    store
        .upsert_decision_entry(session_id, &entry)
        .map_err(|e| e.to_string())?;
    log::info!("[answer] Updated decision summary: answer=\"{}\"", entry.answer);

    // 4. Find dependents (transitive) and mark stale
    let stale_ids = find_transitive_dependents(&*store, session_id, question_id);
    if !stale_ids.is_empty() {
        log::info!("[answer] Found {} transitive dependents to mark stale: {:?}", stale_ids.len(), stale_ids);
        store
            .mark_stale(&stale_ids)
            .map_err(|e| e.to_string())?;
        let _ = app_handle.emit(
            EVENT_STALE_MARKED,
            StaleMarkedPayload {
                session_id: session_id.to_string(),
                question_ids: stale_ids.clone(),
                triggered_by_question_id: question_id.to_string(),
                triggered_by_question_text: question.question.clone(),
            },
        );
        log::info!("[answer] Emitted stale_marked event");
    } else {
        log::info!("[answer] No dependents to mark stale");
    }

    // 5. Refresh outline coverage (an answer may complete a node)
    refresh_outline_coverage(&*store, session_id, app_handle);

    // 6. Check water levels → maybe trigger new batch
    check_water_levels_and_trigger(
        store,
        state,
        session_id,
        &[question_id.to_string()],
        app_handle,
        tx,
    );

    Ok(())
}

fn handle_skip_question(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    question_id: &str,
    app_handle: &AppHandle,
    tx: mpsc::Sender<SchedulerMsg>,
) -> Result<(), String> {
    store
        .update_question_status(question_id, QuestionStatus::Skipped)
        .map_err(|e| e.to_string())?;

    refresh_outline_coverage(&*store, session_id, app_handle);

    // Check water levels → maybe trigger new batch
    check_water_levels_and_trigger(
        store,
        state,
        session_id,
        &[question_id.to_string()],
        app_handle,
        tx,
    );

    Ok(())
}

fn handle_regenerate_stale(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    question_ids: &[String],
    app_handle: &AppHandle,
    tx: mpsc::Sender<SchedulerMsg>,
) -> Result<(), String> {
    // Mark the specified questions as stale (they already are, but ensure)
    store
        .mark_stale(question_ids)
        .map_err(|e| e.to_string())?;

    // User explicitly wants regeneration — clear the saturation stop.
    state.get_or_create(session_id).saturated = false;

    // Trigger a new batch generation
    check_water_levels_and_trigger(store, state, session_id, question_ids, app_handle, tx);

    Ok(())
}

async fn handle_finish_session(
    store: &dyn Store,
    state: &mut SchedulerState,
    session_id: &str,
    app_handle: &AppHandle,
) -> Result<(Vec<DecisionEntry>, String), String> {
    // Mark session as completed
    store
        .update_session_status(session_id, SessionStatus::Completed)
        .map_err(|e| e.to_string())?;

    // Get decision summary
    let summary = store.get_decision_summary(session_id).map_err(|e| e.to_string())?;

    // Try to generate LLM summary
    let settings = store.get_all_settings().map_err(|e| e.to_string())?;
    let session = store
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session {} not found", session_id))?;

    let llm_summary = if !settings.api_key.is_empty() {
        let client = OpenAIClient::new(
            settings.base_url,
            settings.api_key,
            settings.model_name,
            settings.temperature,
        );
        let sys_prompt = prompt::build_summary_system_prompt(&session.role);
        let user_prompt = prompt::build_summary_user_prompt(&session, &summary);
        match client.generate_summary(&sys_prompt, &user_prompt).await {
            Ok(s) => s,
            Err(e) => {
                log::warn!("LLM summary generation failed: {}", e);
                let _ = app_handle.emit(
                    EVENT_ERROR,
                    ErrorPayload {
                        session_id: session_id.to_string(),
                        message: format!("LLM 总结生成失败: {}", e),
                        kind: "summary".to_string(),
                    },
                );
                String::new()
            }
        }
    } else {
        String::new()
    };

    // Save summary to DB
    if !llm_summary.is_empty() {
        store
            .save_session_summary(session_id, &llm_summary)
            .map_err(|e| e.to_string())?;
        log::info!("[finish] Saved summary to DB ({} chars)", llm_summary.len());
    }

    // Emit session complete event
    let _ = app_handle.emit(
        SESSION_COMPLETE,
        SessionCompletePayload {
            session_id: session_id.to_string(),
            suggestion: llm_summary.clone(),
        },
    );

    // Remove from scheduler state
    state.sessions.remove(session_id);

    Ok((summary, llm_summary))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PrototypeGenResult {
    pub version: i32,
    pub preview_url: String,
    pub changelog: Option<String>,
    pub file_count: usize,
    pub commit: Option<String>,
}

/// One-line commit subject: `v{n} · <why this version exists>`.
fn build_commit_reason(feedback: Option<&str>, trigger_note: Option<&str>, version: i32) -> String {
    let first_line = |s: &str| -> String {
        s.trim()
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(72)
            .collect::<String>()
            .trim()
            .to_string()
    };

    // Prefer a concrete decision/trigger over raw agent instruction.
    let reason = trigger_note
        .map(first_line)
        .filter(|s| !s.is_empty())
        .or_else(|| feedback.map(first_line).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "手动更新".into());

    // Strip noisy prefixes from loop/feedback when trigger is more specific
    let reason = reason
        .strip_prefix("[重点:")
        .and_then(|r| r.split_once(']').map(|(_, rest)| rest.trim().to_string()))
        .unwrap_or(reason);

    format!("v{version} · {reason}")
}

/// Sessions with a running iteration graph (prevents overlap).
fn running_sessions() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static CELL: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn cancel_sessions() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static CELL: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn load_or_default_graph(
    store: &Arc<dyn Store>,
    session_id: &str,
) -> Result<crate::graph::IterationGraph, String> {
    if let Some(json) = crate::prototype::load_iteration_graph(session_id) {
        if let Ok(g) = serde_json::from_str::<crate::graph::IterationGraph>(&json) {
            return Ok(g);
        }
    }
    let session = store
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session {} not found", session_id))?;
    let g = crate::graph::IterationGraph::default_for_session(session_id, &session.title);
    if let Ok(json) = serde_json::to_string_pretty(&g) {
        let _ = crate::prototype::save_iteration_graph(session_id, &json);
    }
    Ok(g)
}

/// Spawn a multi-round iteration graph run. Returns immediately with the saved graph.
fn spawn_iteration_graph_run(
    store: Arc<dyn Store>,
    session_id: &str,
    app_handle: &AppHandle,
) -> Result<crate::graph::IterationGraph, String> {
    let graph = load_or_default_graph(&store, session_id)?;
    let problems = crate::graph::validate_graph(&graph);
    if !problems.is_empty() {
        return Err(format!("迭代图校验失败：{}", problems.join("；")));
    }
    let plan = crate::graph::compile_plan(&graph)?;

    {
        let mut running = running_sessions().lock().unwrap();
        if !running.insert(session_id.to_string()) {
            return Err("该会话已有迭代任务在运行".into());
        }
        let mut cancel = cancel_sessions().lock().unwrap();
        cancel.remove(session_id);
    }

    let session_id = session_id.to_string();
    let app_handle = app_handle.clone();
    let steps = plan.clone();

    tauri::async_runtime::spawn(async move {
        let total = steps.len() as u32;
        let _ = app_handle.emit(
            EVENT_GRAPH_PROGRESS,
            GraphProgressPayload {
                session_id: session_id.clone(),
                status: "started".into(),
                step_index: 0,
                step_total: total,
                node_id: None,
                label: None,
                message: Some(format!("共 {total} 步")),
                commit: None,
            },
        );

        let mut failed = false;
        for (idx, step) in steps.iter().enumerate() {
            let cancelled = {
                let mut cancel = cancel_sessions().lock().unwrap();
                cancel.remove(&session_id)
            };
            if cancelled {
                let _ = app_handle.emit(
                    EVENT_GRAPH_PROGRESS,
                    GraphProgressPayload {
                        session_id: session_id.clone(),
                        status: "cancelled".into(),
                        step_index: idx as u32,
                        step_total: total,
                        node_id: Some(step.node_id.clone()),
                        label: Some(step.label.clone()),
                        message: Some("已取消".into()),
                        commit: None,
                    },
                );
                break;
            }

            let _ = app_handle.emit(
                EVENT_GRAPH_PROGRESS,
                GraphProgressPayload {
                    session_id: session_id.clone(),
                    status: "step_start".into(),
                    step_index: idx as u32,
                    step_total: total,
                    node_id: Some(step.node_id.clone()),
                    label: Some(step.label.clone()),
                    message: Some(format!(
                        "{}/{} · {}",
                        idx + 1,
                        total,
                        step.label
                    )),
                    commit: None,
                },
            );
            let _ = app_handle.emit(
                EVENT_GRAPH_NODE_STATUS,
                GraphNodeStatusPayload {
                    session_id: session_id.clone(),
                    node_id: step.node_id.clone(),
                    status: "running".into(),
                    round: Some(step.round),
                    total_rounds: Some(step.total_rounds),
                    commit: None,
                    error: None,
                },
            );

            let trigger = {
                let base = step.label.trim();
                let round = if step.total_rounds > 1 {
                    format!(" {}/{}", step.round, step.total_rounds)
                } else {
                    String::new()
                };
                match &step.focus {
                    Some(f) if !f.trim().is_empty() => format!("{base}{round} · {}", f.trim()),
                    _ => format!("{base}{round}"),
                }
            };
            let feedback = {
                let mut fb = step.instruction.clone();
                if let Some(focus) = &step.focus {
                    if !focus.trim().is_empty() {
                        fb = format!("[重点: {focus}]\n{fb}");
                    }
                }
                fb
            };

            let result = handle_generate_prototype(
                store.as_ref(),
                &session_id,
                Some(&feedback),
                Some(&trigger),
                &app_handle,
            )
            .await;

            match result {
                Ok(r) => {
                    // handle_generate_prototype already committed with reason + file scope.
                    let commit = r.commit.clone();
                    let _ = app_handle.emit(
                        EVENT_GRAPH_NODE_STATUS,
                        GraphNodeStatusPayload {
                            session_id: session_id.clone(),
                            node_id: step.node_id.clone(),
                            status: "completed".into(),
                            round: Some(step.round),
                            total_rounds: Some(step.total_rounds),
                            commit: commit.clone(),
                            error: None,
                        },
                    );
                    let _ = app_handle.emit(
                        EVENT_GRAPH_PROGRESS,
                        GraphProgressPayload {
                            session_id: session_id.clone(),
                            status: "step_done".into(),
                            step_index: idx as u32,
                            step_total: total,
                            node_id: Some(step.node_id.clone()),
                            label: Some(step.label.clone()),
                            message: Some(format!("v{} 完成", r.version)),
                            commit,
                        },
                    );
                }
                Err(e) => {
                    failed = true;
                    let _ = app_handle.emit(
                        EVENT_GRAPH_NODE_STATUS,
                        GraphNodeStatusPayload {
                            session_id: session_id.clone(),
                            node_id: step.node_id.clone(),
                            status: "failed".into(),
                            round: Some(step.round),
                            total_rounds: Some(step.total_rounds),
                            commit: None,
                            error: Some(e.clone()),
                        },
                    );
                    let _ = app_handle.emit(
                        EVENT_GRAPH_PROGRESS,
                        GraphProgressPayload {
                            session_id: session_id.clone(),
                            status: "step_failed".into(),
                            step_index: idx as u32,
                            step_total: total,
                            node_id: Some(step.node_id.clone()),
                            label: Some(step.label.clone()),
                            message: Some(e),
                            commit: None,
                        },
                    );
                    break;
                }
            }
        }

        let _ = app_handle.emit(
            EVENT_GRAPH_PROGRESS,
            GraphProgressPayload {
                session_id: session_id.clone(),
                status: if failed { "failed".into() } else { "done".into() },
                step_index: total,
                step_total: total,
                node_id: None,
                label: None,
                message: Some(if failed {
                    "迭代中断".into()
                } else {
                    "迭代完成".into()
                }),
                commit: None,
            },
        );

        {
            let mut running = running_sessions().lock().unwrap();
            running.remove(&session_id);
        }
    });

    Ok(graph)
}

/// Compose the coding-agent task from interview decisions + optional feedback.
/// The agent owns file I/O; we only own requirements & prompt.
fn build_agent_task_prompt(
    session: &Session,
    decisions: &[DecisionEntry],
    feedback: Option<&str>,
    trigger_note: Option<&str>,
    is_first: bool,
) -> String {
    let mut p = String::new();
    p.push_str("# 任务：实现 / 增量更新静态多文件 HTML 原型\n\n");
    p.push_str("你在**会话工作区根目录**。请先读 `docs/`，再只在 `prototype/` 内创建/修改文件。不要只给建议。\n\n");

    p.push_str("## 工作区布局\n");
    p.push_str("```\n");
    p.push_str("./\n");
    p.push_str("├── docs/decisions.md   # 已确认访谈决策（权威）\n");
    p.push_str("├── docs/intent.md      # 产品意图摘要\n");
    p.push_str("└── prototype/          # 静态原型（唯一可写代码区）\n");
    p.push_str("    ├── index.html\n");
    p.push_str("    ├── css/styles.css\n");
    p.push_str("    ├── js/app.js\n");
    p.push_str("    └── pages/*.html\n");
    p.push_str("```\n\n");

    p.push_str("## 项目\n");
    p.push_str(&format!("- 主题：{}\n", session.title));
    p.push_str(&format!("- 角色视角：{}\n", session.role));
    if let Some(ctx) = &session.initial_context {
        if !ctx.is_empty() {
            p.push_str(&format!("- 初始上下文：{}\n", ctx));
        }
    }
    if let Some(note) = trigger_note {
        if !note.is_empty() {
            p.push_str(&format!("- 本次触发：{}\n", note));
        }
    }
    p.push('\n');

    p.push_str("## 已确认的访谈决策\n");
    p.push_str("（完整版见 `docs/decisions.md`；用户自定义回复优先于预设选项标签）\n");
    if decisions.is_empty() {
        p.push_str("（暂无）\n");
    }
    for d in decisions {
        p.push_str(&format!("- {}: {}\n", d.question, d.answer));
    }
    p.push('\n');

    if let Some(fb) = feedback {
        if !fb.is_empty() {
            p.push_str("## 用户对当前原型的修改意见\n");
            p.push_str(fb);
            p.push_str("\n\n");
        }
    }

    p.push_str("## 目录与技术约束\n");
    p.push_str("- **只改 `prototype/` 下的文件**；`docs/` 只读\n");
    p.push_str("- 静态站点，无构建工具、无 package.json、无框架\n");
    p.push_str("- 入口固定 `prototype/index.html`；样式 `prototype/css/styles.css`；逻辑 `prototype/js/app.js`；次级页 `prototype/pages/*.html`\n");
    p.push_str("- 相对路径引用；禁止外部 CDN/字体/网络图片；图标用内联 SVG 或 CSS\n");
    p.push_str("- 中文占位数据，贴合业务场景，不要 Lorem Ipsum\n");
    p.push_str("- 看起来像真实产品：完整导航/布局/关键操作，可点击切换\n\n");

    if is_first {
        p.push_str("## 要求（首次生成）\n");
        p.push_str("在 `prototype/` 从零搭建完整可运行的多文件原型，确保 `prototype/index.html` 可直接打开。\n");
    } else {
        p.push_str("## 要求（增量更新）\n");
        p.push_str("阅读 `prototype/` 现有文件，只修改与最新决策/反馈相关的部分；保持已有风格与结构稳定；必要时新增/删除页面。\n");
    }

    p.push_str("\n完成后确认：入口可打开、路径正确、无外部依赖。\n");
    p
}

/// Generate or update the multi-file prototype by invoking a local coding agent CLI.
async fn handle_generate_prototype(
    store: &dyn Store,
    session_id: &str,
    feedback: Option<&str>,
    trigger_note: Option<&str>,
    app_handle: &AppHandle,
) -> Result<PrototypeGenResult, String> {
    let session = store
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session {} not found", session_id))?;

    let decision_summary = store
        .get_decision_summary(session_id)
        .map_err(|e| e.to_string())?;

    let settings = store.get_all_settings().map_err(|e| e.to_string())?;

    // Workspace root: docs/ + prototype/ + .git. Agent cwd = workspace root.
    let ws = crate::workspace::ensure_workspace(session_id).map_err(|e| e.to_string())?;
    let workdir = ws.join("prototype");
    let workdir_str = ws.to_string_lossy().to_string();
    let is_first = !crate::prototype::has_prototype(session_id);

    // Seed a minimal scaffold so the agent always has a starting point
    if is_first {
        crate::prototype::seed_scaffold(session_id).map_err(|e| e.to_string())?;
    }

    // Sync structured spec for the agent to read
    let outline_nodes = store.get_outline_nodes(session_id).unwrap_or_default();
    let _ = crate::workspace::write_decisions_md(
        session_id,
        &session.title,
        &session.role,
        session.initial_context.as_deref(),
        &decision_summary,
        &outline_nodes,
    );
    let _ = crate::workspace::write_intent_md(
        session_id,
        &session.title,
        &session.role,
        session.initial_context.as_deref(),
    );

    let prompt = build_agent_task_prompt(
        &session,
        &decision_summary,
        feedback,
        trigger_note,
        is_first,
    );

    // Persist the task at workspace root so the agent (and humans) can read it
    let task_path = ws.join("TASK.md");
    let _ = std::fs::write(&task_path, &prompt);

    let _ = app_handle.emit(
        EVENT_PROTOTYPE_STATUS,
        PrototypeStatusPayload {
            session_id: session_id.to_string(),
            status: "generating".to_string(),
            message: Some(format!("agent: {}", settings.agent_tool)),
        },
    );

    log::info!(
        "[prototype] Running agent={} model={} workspace={} proto={} first={}",
        settings.agent_tool,
        settings.agent_model,
        workdir_str,
        workdir.display(),
        is_first
    );

    let tool = settings.agent_tool.clone();
    let model = settings.agent_model.clone();
    let effort = settings.agent_effort.clone();
    let auto_approve = settings.agent_auto_approve;
    let sid = session_id.to_string();
    let app_for_lines = app_handle.clone();
    let sid_for_log = sid.clone();

    crate::agent::clear_agent_log(&sid);
    let cancel_flag = crate::agent::new_cancel_flag(&sid);

    let run_result = tauri::async_runtime::spawn_blocking(move || {
        crate::agent::run_agent_to_completion(
            &tool,
            &model,
            &prompt,
            &workdir_str,
            auto_approve,
            &effort,
            Some(cancel_flag),
            move |stream, text| {
                crate::agent::push_agent_log(&sid_for_log, stream, text);
                let _ = app_for_lines.emit(
                    EVENT_AGENT_OUTPUT,
                    AgentOutputPayload {
                        session_id: sid.clone(),
                        stream: stream.to_string(),
                        text: text.to_string(),
                    },
                );
            },
        )
    })
    .await
    .map_err(|e| format!("agent join: {e}"))?;

    crate::agent::clear_cancel_flag(session_id);

    let code = match run_result {
        Ok(c) => c,
        Err(e) if e == crate::agent::ERR_CANCELLED => {
            let _ = app_handle.emit(
                EVENT_PROTOTYPE_STATUS,
                PrototypeStatusPayload {
                    session_id: session_id.to_string(),
                    status: "cancelled".to_string(),
                    message: Some("已取消".to_string()),
                },
            );
            return Err("已取消".to_string());
        }
        Err(e) => {
            let _ = app_handle.emit(
                EVENT_PROTOTYPE_STATUS,
                PrototypeStatusPayload {
                    session_id: session_id.to_string(),
                    status: "failed".to_string(),
                    message: Some(e.clone()),
                },
            );
            let _ = app_handle.emit(
                EVENT_ERROR,
                ErrorPayload {
                    session_id: session_id.to_string(),
                    message: format!("原型 agent 失败: {}", e),
                    kind: "prototype".to_string(),
                },
            );
            return Err(e);
        }
    };

    if code != 0 {
        let msg = format!("agent 退出码 {code}");
        let _ = app_handle.emit(
            EVENT_PROTOTYPE_STATUS,
            PrototypeStatusPayload {
                session_id: session_id.to_string(),
                status: "failed".to_string(),
                message: Some(msg.clone()),
            },
        );
        let _ = app_handle.emit(
            EVENT_ERROR,
            ErrorPayload {
                session_id: session_id.to_string(),
                message: format!("原型生成失败: {}", msg),
                kind: "prototype".to_string(),
            },
        );
        return Err(msg);
    }

    if !crate::prototype::has_prototype(session_id) {
        let msg = "agent 完成但未找到 index.html".to_string();
        let _ = app_handle.emit(
            EVENT_ERROR,
            ErrorPayload {
                session_id: session_id.to_string(),
                message: msg.clone(),
                kind: "prototype".to_string(),
            },
        );
        return Err(msg);
    }

    let new_version = session.prototype_version + 1;
    let final_snapshot = crate::prototype::read_snapshot(session_id);
    let snapshot_json = serde_json::to_string(&final_snapshot).map_err(|e| e.to_string())?;
    let subject_reason = build_commit_reason(feedback, trigger_note, new_version);
    let changelog = format!(
        "{}（{} 个文件）",
        subject_reason,
        final_snapshot.files.len()
    );

    let version_record = crate::model::PrototypeVersion {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        version: new_version,
        snapshot_json,
        feedback: feedback.map(|s| s.to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    store
        .save_prototype_version(&version_record)
        .map_err(|e| e.to_string())?;
    store
        .save_prototype_version_meta(session_id, new_version)
        .map_err(|e| e.to_string())?;

    let preview_url =
        crate::prototype::ensure_preview_server(session_id).map_err(|e| e.to_string())?;

    let file_count = final_snapshot.files.len();
    log::info!(
        "[prototype] Agent applied v{}: {} files, preview={}",
        new_version,
        file_count,
        preview_url
    );

    // Record one git commit per successful generation (decision-tree node).
    // Subject: why this version exists; body: full trigger + instruction + file scope.
    let commit = {
        let mut body = String::new();
        if let Some(note) = trigger_note {
            if !note.trim().is_empty() {
                body.push_str(&format!("原因: {}\n", note.trim()));
            }
        }
        if let Some(fb) = feedback {
            let one: String = fb.trim().lines().next().unwrap_or("").chars().take(160).collect();
            if !one.is_empty() {
                body.push_str(&format!("任务: {one}\n"));
            }
        }
        body.push_str(&format!("结果: {} 个文件已更新\n", file_count));
        crate::git::commit_all_with_body(&ws, &subject_reason, Some(&body))
    };
    match &commit {
        Ok(sha) => log::info!("[prototype] git commit {sha}"),
        Err(e) => log::warn!("[prototype] git commit skipped: {}", e),
    }
    let commit_sha = commit.ok();

    let _ = app_handle.emit(
        EVENT_PROTOTYPE_UPDATED,
        PrototypeUpdatedPayload {
            session_id: session_id.to_string(),
            version: new_version,
            preview_url: preview_url.clone(),
            changelog: Some(changelog.clone()),
            file_count,
        },
    );
    let _ = app_handle.emit(
        EVENT_PROTOTYPE_STATUS,
        PrototypeStatusPayload {
            session_id: session_id.to_string(),
            status: "done".to_string(),
            message: Some(changelog.clone()),
        },
    );

    Ok(PrototypeGenResult {
        version: new_version,
        preview_url,
        changelog: Some(changelog),
        file_count,
        commit: commit_sha,
    })
}

/// Minimum answered decisions required before the first prototype generation.
const MIN_ANSWERS_FOR_FIRST_GEN: usize = 3;

/// After an answer, auto-trigger an incremental prototype update (debounced per session).
/// First generation waits until at least MIN_ANSWERS_FOR_FIRST_GEN answers exist.
fn spawn_auto_prototype_update(
    store: Arc<dyn Store>,
    session_id: &str,
    trigger_note: String,
    app_handle: AppHandle,
    pending: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
) {
    let session_id = session_id.to_string();
    {
        let mut guard = pending.lock().unwrap();
        if guard.contains(&session_id) {
            log::info!("[prototype] auto-update already pending for {}", session_id);
            return;
        }
        guard.insert(session_id.clone());
    }

    tauri::async_runtime::spawn(async move {
        // Small debounce so rapid answers coalesce
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        {
            let mut guard = pending.lock().unwrap();
            guard.remove(&session_id);
        }

        let decisions = match store.get_decision_summary(&session_id) {
            Ok(d) => d,
            Err(e) => {
                log::error!("[prototype] auto-update failed to load decisions: {}", e);
                return;
            }
        };

        let has_proto = crate::prototype::has_prototype(&session_id);

        // Global pause switch: skip auto gen; manual generate_prototype still runs
        let paused = store
            .get_all_settings()
            .map(|s| s.prototype_auto_paused)
            .unwrap_or(false);
        if paused {
            log::info!("[prototype] auto-update skipped: paused by user");
            let _ = app_handle.emit(
                EVENT_PROTOTYPE_STATUS,
                PrototypeStatusPayload {
                    session_id: session_id.clone(),
                    status: "paused".to_string(),
                    message: Some("已暂停自动生成".to_string()),
                },
            );
            return;
        }

        if !has_proto {
            // First generation: wait until we have enough signal
            if decisions.len() < MIN_ANSWERS_FOR_FIRST_GEN {
                log::info!(
                    "[prototype] first gen waiting: {}/{} answers",
                    decisions.len(),
                    MIN_ANSWERS_FOR_FIRST_GEN
                );
                let _ = app_handle.emit(
                    EVENT_PROTOTYPE_STATUS,
                    PrototypeStatusPayload {
                        session_id: session_id.clone(),
                        status: "waiting_answers".to_string(),
                        message: Some(format!(
                            "还需 {} 条回答后开始生成原型",
                            MIN_ANSWERS_FOR_FIRST_GEN.saturating_sub(decisions.len())
                        )),
                    },
                );
                return;
            }
        } else if decisions.is_empty() {
            log::info!("[prototype] auto-update skipped: no decisions yet");
            return;
        }

        match handle_generate_prototype(
            store.as_ref(),
            &session_id,
            None,
            Some(&trigger_note),
            &app_handle,
        )
        .await
        {
            Ok(r) => log::info!(
                "[prototype] auto-update done: v{}, {} files",
                r.version,
                r.file_count
            ),
            Err(e) => log::error!("[prototype] auto-update failed: {}", e),
        }
    });
}

/// Handle a user chat message: save it, call LLM with tools, stream response, process tool calls.
async fn handle_send_message(
    store: &dyn Store,
    session_id: &str,
    content: &str,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let session = store
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session {} not found", session_id))?;

    let settings = store.get_all_settings().map_err(|e| e.to_string())?;
    if settings.api_key.is_empty() {
        return Err("API key not set".to_string());
    }

    // 1. Save user message
    let user_msg = crate::model::ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        question_ids: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    store.insert_message(&user_msg).map_err(|e| e.to_string())?;

    // 2. Load chat history for context
    let history = store.get_messages(session_id).map_err(|e| e.to_string())?;
    let decision_summary = store.get_decision_summary(session_id).map_err(|e| e.to_string())?;

    // 3. Build API messages
    let system_prompt = prompt::build_chat_system_prompt(&session);
    let mut api_messages = vec![crate::llm::openai::ApiMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];

    // Add decision summary as context
    if !decision_summary.is_empty() {
        let summary_text = decision_summary
            .iter()
            .map(|d| format!("- {}: {}", d.question, d.answer))
            .collect::<Vec<_>>()
            .join("\n");
        api_messages.push(crate::llm::openai::ApiMessage {
            role: "system".to_string(),
            content: format!("已确认的决策：\n{}", summary_text),
        });
    }

    // Add chat history (limit to last 20 messages to avoid context overflow)
    let history_limit = 20;
    let start = if history.len() > history_limit {
        history.len() - history_limit
    } else {
        0
    };
    for msg in &history[start..] {
        api_messages.push(crate::llm::openai::ApiMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    // 4. Build tools
    let tools = prompt::build_chat_tools();

    // 5. Call LLM with streaming
    let client = OpenAIClient::new(
        settings.base_url,
        settings.api_key,
        settings.model_name,
        settings.temperature,
    );

    log::info!("[chat] Sending to LLM: {} messages, {} tools", api_messages.len(), tools.len());

    let session_id_owned = session_id.to_string();
    let app_handle_clone = app_handle.clone();

    let mut on_text = |delta: &str| {
        let _ = app_handle_clone.emit(
            "chat_stream",
            serde_json::json!({
                "session_id": session_id_owned,
                "delta": delta,
            }),
        );
    };

    let (full_text, tool_calls) = client
        .chat_with_tools(&api_messages, &tools, &mut on_text)
        .await
        .map_err(|e| {
            log::error!("[chat] LLM call failed: {}", e);
            e.to_string()
        })?;

    log::info!("[chat] LLM response: {} chars, {} tool_calls", full_text.len(), tool_calls.len());

    // 6. Process tool calls
    let mut question_ids: Vec<String> = Vec::new();

    for tc in &tool_calls {
        log::info!("[chat] Tool call: {} args_len={}", tc.name, tc.arguments.len());
        match tc.name.as_str() {
            "create_batch" => {
                // Parse questions from tool arguments
                match serde_json::from_str::<serde_json::Value>(&tc.arguments) {
                    Ok(args) => {
                        if let Some(questions_arr) = args.get("questions").and_then(|v| v.as_array()) {
                            let display_order_offset = store.get_max_display_order(session_id).unwrap_or(0) + 1;
                            let batch_id = uuid::Uuid::new_v4().to_string();

                            for (i, q_json) in questions_arr.iter().enumerate() {
                                let q_type = q_json.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                                let question_text = q_json.get("question").and_then(|v| v.as_str()).unwrap_or("");
                                let rationale = q_json.get("rationale").and_then(|v| v.as_str()).map(|s| s.to_string());

                                let options: Vec<crate::model::QuestionOption> = q_json
                                    .get("options")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter().filter_map(|o| {
                                            let label = o.get("label").and_then(|v| v.as_str())?.to_string();
                                            let desc = o.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                                            Some(crate::model::QuestionOption { label, description: desc })
                                        }).collect()
                                    })
                                    .unwrap_or_default();

                                let q_id = uuid::Uuid::new_v4().to_string();
                                let question = crate::model::Question {
                                    id: q_id.clone(),
                                    session_id: session_id.to_string(),
                                    batch_id: batch_id.clone(),
                                    q_type: crate::model::QuestionType::from_str(q_type),
                                    category: crate::model::QuestionCategory::Intent,
                                    question: question_text.to_string(),
                                    context: None,
                                    options,
                                    recommended_option: None,
                                    rationale: rationale.clone(),
                                    depends_on: vec![],
                                    status: crate::model::QuestionStatus::Ready,
                                    answer: None,
                                    answer_version: 0,
                                    display_order: display_order_offset + i as i32,
                                    message_id: None, // will be set after message is saved
                                    outline_node_id: None, // chat-tool questions are unscoped
                                };

                                if let Err(e) = store.insert_question(&question) {
                                    log::error!("[chat] Failed to insert question: {}", e);
                                    continue;
                                }
                                question_ids.push(q_id.clone());

                                // Emit new_question event
                                let _ = app_handle.emit(
                                    EVENT_NEW_QUESTION,
                                    NewQuestionPayload {
                                        session_id: session_id.to_string(),
                                        question,
                                    },
                                );
                            }
                            log::info!("[chat] Created {} questions from tool call", question_ids.len());
                        }
                    }
                    Err(e) => {
                        log::error!("[chat] Failed to parse create_batch args: {}", e);
                    }
                }
            }
            "finish_interview" => {
                log::info!("[chat] finish_interview tool called, triggering session completion");
                let _ = app_handle.emit(
                    "interview_may_complete",
                    serde_json::json!({ "session_id": session_id }),
                );
            }
            _ => {
                log::warn!("[chat] Unknown tool: {}", tc.name);
            }
        }
    }

    // 7. Save assistant message
    let assistant_msg = crate::model::ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: full_text,
        question_ids: question_ids.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let assistant_msg_id = assistant_msg.id.clone();
    store.insert_message(&assistant_msg).map_err(|e| e.to_string())?;

    // 8. Link questions to the assistant message
    for qid in &question_ids {
        let _ = store.add_message_id_to_question(qid, &assistant_msg_id);
    }

    // 9. Emit chat_message_done
    let _ = app_handle.emit(
        "chat_message_done",
        serde_json::json!({
            "session_id": session_id,
            "message": assistant_msg,
        }),
    );

    Ok(())
}

/// Find all questions that transitively depend on the given question.
fn find_transitive_dependents(store: &dyn Store, session_id: &str, question_id: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut queue = vec![question_id.to_string()];
    let mut visited = std::collections::HashSet::new();

    while let Some(qid) = queue.pop() {
        if visited.contains(&qid) {
            continue;
        }
        visited.insert(qid.clone());

        if let Ok(dependents) = store.find_dependents(session_id, &qid) {
            for dep in dependents {
                if !result.contains(&dep) {
                    result.push(dep.clone());
                    queue.push(dep);
                }
            }
        }
    }

    result
}

/// Check water levels and trigger a new batch if needed.
fn check_water_levels_and_trigger(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    triggered_by: &[String],
    app_handle: &AppHandle,
    tx: mpsc::Sender<SchedulerMsg>,
) {
    let settings = match store.get_all_settings() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[water] Failed to get settings: {}", e);
            return;
        }
    };

    // --- Outline gates ---
    let session = match store.get_session(session_id) {
        Ok(Some(s)) => s,
        _ => return,
    };
    match session.outline_status {
        // Outline still generating or waiting for user confirmation — hold all batches.
        OutlineStatus::Generating | OutlineStatus::Draft => {
            log::info!("[water] session={}: SKIP (outline_status={})", session_id, session.outline_status.as_str());
            return;
        }
        _ => {}
    }

    let session_state = state.get_or_create(session_id);
    if session_state.saturated {
        log::info!("[water] session={}: SKIP (saturated — LLM signalled completion)", session_id);
        return;
    }

    if session.outline_status == OutlineStatus::Confirmed {
        let nodes = store.get_outline_nodes(session_id).unwrap_or_default();
        if !nodes.is_empty() && !nodes.iter().any(|n| n.status == OutlineNodeStatus::Pending) {
            log::info!("[water] session={}: STOP (all outline nodes covered/excluded)", session_id);
            let _ = app_handle.emit(
                "interview_may_complete",
                serde_json::json!({ "session_id": session_id }),
            );
            return;
        }
    }

    let inventory = store.count_ready_questions(session_id).unwrap_or(0);
    let low_water = settings.batch_size / 2;
    let high_water = 2 * settings.batch_size;

    let session_state = state.get_or_create(session_id);

    log::info!(
        "[water] session={}: inventory={}, low={}, high={}, active_batches={}, max={}, debounce={}s",
        session_id, inventory, low_water, high_water,
        session_state.active_batches, settings.max_concurrent_batches, settings.debounce_seconds
    );

    if inventory >= high_water {
        log::info!("[water] session={}: STOP (inventory >= high_water)", session_id);
        return;
    }

    if inventory > low_water {
        log::info!("[water] session={}: SKIP (inventory > low_water, no trigger needed)", session_id);
        return;
    }

    // inventory <= low_water
    if session_state.active_batches >= settings.max_concurrent_batches {
        log::info!("[water] session={}: SKIP (active_batches >= max_concurrent)", session_id);
        return;
    }

    // Check debounce
    if let Some(last_time) = session_state.last_generation_time {
        let elapsed = last_time.elapsed().as_secs();
        if elapsed < settings.debounce_seconds as u64 {
            log::info!(
                "[water] session={}: SKIP (debounce: {}s < {}s)",
                session_id, elapsed, settings.debounce_seconds
            );
            return;
        }
    }

    log::info!("[water] session={}: TRIGGER new batch generation", session_id);
    spawn_batch_generation(store, state, session_id, triggered_by, app_handle, tx.clone());
}

/// Spawn a batch generation task.
fn spawn_batch_generation(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    triggered_by: &[String],
    app_handle: &AppHandle,
    scheduler_tx: mpsc::Sender<SchedulerMsg>,
) {
    log::info!("[batch] spawn_batch_generation: session={}", session_id);

    let settings = match store.get_all_settings() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[batch] Failed to get settings for batch: {}", e);
            return;
        }
    };

    if settings.api_key.is_empty() {
        log::warn!("[batch] No API key set, skipping batch generation");
        let _ = app_handle.emit(
            EVENT_ERROR,
            ErrorPayload {
                session_id: session_id.to_string(),
                message: "未设置 API Key，无法生成问题。请在设置中配置 API Key。".to_string(),
                kind: "auth".to_string(),
            },
        );
        return;
    }

    log::info!("[batch] API key is set (len={}), base_url={}, model={}", settings.api_key.len(), settings.base_url, settings.model_name);

    // Get session
    let session = match store.get_session(session_id) {
        Ok(Some(s)) => s,
        _ => {
            log::error!("[batch] Session not found: {}", session_id);
            return;
        }
    };

    // Get decision summary and recent answers for prompt
    let decision_summary = store.get_decision_summary(session_id).unwrap_or_default();
    let answered = store.get_answered_questions(session_id).unwrap_or_default();

    // Outline scoping: only when the user confirmed an outline.
    let outline_nodes: Vec<OutlineNode> = if session.outline_status == OutlineStatus::Confirmed {
        store.get_outline_nodes(session_id).unwrap_or_default()
    } else {
        Vec::new()
    };
    let skipped = store.get_skipped_questions(session_id).unwrap_or_default();

    // Per-node stats for the prompt + valid node ids for node_id validation.
    let all_questions = store.get_questions(session_id).unwrap_or_default();
    let mut node_stats: std::collections::HashMap<String, prompt::NodeStats> =
        std::collections::HashMap::new();
    for q in &all_questions {
        if let Some(nid) = &q.outline_node_id {
            let s = node_stats.entry(nid.clone()).or_insert(prompt::NodeStats {
                answered: 0,
                pending: 0,
            });
            match q.status {
                QuestionStatus::Answered | QuestionStatus::Skipped => s.answered += 1,
                QuestionStatus::Ready | QuestionStatus::Generating | QuestionStatus::Stale => {
                    s.pending += 1
                }
            }
        }
    }
    let valid_node_ids: std::collections::HashSet<String> =
        outline_nodes.iter().map(|n| n.id.clone()).collect();

    log::info!(
        "[batch] session={}: decision_summary={} entries, answered={} questions, outline_nodes={}, cold_start={}",
        session_id, decision_summary.len(), answered.len(), outline_nodes.len(),
        decision_summary.is_empty() && answered.is_empty()
    );

    // Create batch record
    let batch_no = store.get_max_batch_no(session_id).unwrap_or(0) + 1;
    let batch_id = uuid::Uuid::new_v4().to_string();
    let batch = Batch {
        id: batch_id.clone(),
        session_id: session_id.to_string(),
        batch_no,
        status: BatchStatus::Generating,
        triggered_by_answers: triggered_by.to_vec(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Err(e) = store.insert_batch(&batch) {
        log::error!("[batch] Failed to insert batch: {}", e);
        return;
    }
    log::info!("[batch] Created batch: id={}, batch_no={}", batch_id, batch_no);

    // Update session state
    let session_state = state.get_or_create(session_id);
    session_state.active_batches += 1;
    session_state.last_generation_time = Some(std::time::Instant::now());

    // Emit batch_status: generating so frontend knows to show loading state
    let _ = app_handle.emit(
        EVENT_BATCH_STATUS,
        BatchStatusPayload {
            session_id: session_id.to_string(),
            status: "generating".to_string(),
            remaining: store.count_ready_questions(session_id).unwrap_or(0),
        },
    );

    // Build prompts
    let system_prompt = prompt::build_system_prompt(settings.batch_size, &session.role);
    let user_prompt = prompt::build_user_prompt(
        &session,
        &decision_summary,
        &answered,
        settings.batch_size,
        &outline_nodes,
        &node_stats,
        &skipped,
    );

    // Create LLM client
    let client = OpenAIClient::new(
        settings.base_url,
        settings.api_key,
        settings.model_name,
        settings.temperature,
    );

    let batch_id = batch_id.clone();
    let session_id = session_id.to_string();
    let app_handle = app_handle.clone();
    let batch_size = settings.batch_size;
    let store = store.clone();
    let scheduler_tx = scheduler_tx.clone();

    // Spawn the batch generation task
    log::info!("[batch] Spawning tokio task for batch_id={}", batch_id);
    tokio::spawn(async move {
        log::info!("[batch-task] Started: batch_id={}, session={}", batch_id, session_id);

        let display_order_offset = std::sync::Arc::new(std::sync::Mutex::new(
            store
                .get_max_display_order(&session_id)
                .unwrap_or(0)
                + 1,
        ));
        log::info!("[batch-task] display_order_offset start = {}", *display_order_offset.lock().unwrap());

        // Counter for questions generated in this batch
        let question_count = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));

        // Wrap callback in Arc<Mutex> so it can be reused across retries
        let session_id_arc = std::sync::Arc::new(session_id.clone());
        let batch_id_arc = std::sync::Arc::new(batch_id.clone());
        let store_arc = store.clone();
        let app_handle_arc = std::sync::Arc::new(app_handle.clone());
        let display_order_arc = display_order_offset.clone();
        let question_count_arc = question_count.clone();

        let callback = std::sync::Arc::new(std::sync::Mutex::new(
            move |mut q: Question| {
                // Override session_id and batch_id (they were set to empty in the parser)
                q.session_id = session_id_arc.clone().as_ref().clone();
                q.batch_id = batch_id_arc.clone().as_ref().clone();
                // Drop node_ids that don't exist in the current outline
                if let Some(nid) = &q.outline_node_id {
                    if !valid_node_ids.contains(nid) {
                        q.outline_node_id = None;
                    }
                }
                let mut offset_guard = display_order_arc.lock().unwrap();
                q.display_order = *offset_guard;
                *offset_guard += 1;
                drop(offset_guard);

                question_count_arc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                log::info!(
                    "[batch-callback] Parsed question: id={}, type={:?}, order={}, question=\"{}\"",
                    q.id, q.q_type, q.display_order, q.question.chars().take(50).collect::<String>()
                );

                // Insert into DB
                if let Err(e) = store_arc.insert_question(&q) {
                    log::error!("[batch-callback] Failed to insert question {}: {}", q.id, e);
                    return;
                }
                log::info!("[batch-callback] Inserted question {} into DB", q.id);

                // Emit event
                let _ = app_handle_arc.emit(
                    EVENT_NEW_QUESTION,
                    NewQuestionPayload {
                        session_id: session_id_arc.clone().as_ref().clone(),
                        question: q,
                    },
                );
                log::info!("[batch-callback] Emitted new_question event");
            },
        ));

        // Retry logic
        let max_retries = 3;
        let mut attempt = 0;
        loop {
            attempt += 1;
            log::info!("[batch-task] Attempt {} for batch_id={}", attempt, batch_id);
            let cb = callback.clone();
            let on_question = Box::new(move |q: Question| {
                let guard = cb.lock().unwrap();
                guard(q);
            });
            let result = client
                .generate_batch(
                    &system_prompt,
                    &user_prompt,
                    batch_size,
                    on_question,
                )
                .await;

            match result {
                Ok(()) => {
                    // Update batch status to done
                    let _ = store.update_batch_status(&batch_id, BatchStatus::Done);

                    // Emit batch status
                    let remaining = store.count_ready_questions(&session_id).unwrap_or(0);
                    let _ = app_handle.emit(
                        EVENT_BATCH_STATUS,
                        BatchStatusPayload {
                            session_id: session_id.clone(),
                            status: "done".to_string(),
                            remaining,
                        },
                    );
                    break;
                }
                Err(LlmError::Auth) => {
                    let _ = store.update_batch_status(&batch_id, BatchStatus::Failed);
                    let _ = app_handle.emit(
                        EVENT_ERROR,
                        ErrorPayload {
                            session_id: session_id.clone(),
                            message: "API Key 无效".to_string(),
                            kind: "auth".to_string(),
                        },
                    );
                    break;
                }
                Err(LlmError::RateLimited) => {
                    if attempt > max_retries {
                        let _ = store.update_batch_status(&batch_id, BatchStatus::Failed);
                        let _ = app_handle.emit(
                            EVENT_ERROR,
                            ErrorPayload {
                                session_id: session_id.clone(),
                                message: "请求频率受限，请稍后重试".to_string(),
                                kind: "rate_limit".to_string(),
                            },
                        );
                        break;
                    }
                    // Longer backoff for 429: 10s, 30s, 60s
                    let delay = match attempt {
                        1 => 10,
                        2 => 30,
                        _ => 60,
                    };
                    log::warn!("Rate limited, retrying in {}s (attempt {})", delay, attempt);
                    let _ = app_handle.emit(
                        EVENT_ERROR,
                        ErrorPayload {
                            session_id: session_id.clone(),
                            message: format!("请求频率受限，{}秒后重试", delay),
                            kind: "rate_limit".to_string(),
                        },
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
                Err(e) => {
                    if attempt > max_retries {
                        let _ = store.update_batch_status(&batch_id, BatchStatus::Failed);
                        let _ = app_handle.emit(
                            EVENT_ERROR,
                            ErrorPayload {
                                session_id: session_id.clone(),
                                message: format!("生成问题失败: {}", e),
                                kind: "generation".to_string(),
                            },
                        );
                        break;
                    }
                    // Exponential backoff: 1s, 2s, 4s
                    let delay = 1 << (attempt - 1);
                    log::warn!("Generation error: {}, retrying in {}s (attempt {})", e, delay, attempt);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
            }
        }

        // Notify scheduler actor that this batch is done (decrement active_batches)
        let generated = question_count.load(std::sync::atomic::Ordering::Relaxed);
        let _ = scheduler_tx.send(SchedulerMsg::BatchDone {
            session_id: session_id.clone(),
            questions_generated: generated,
            app_handle: app_handle.clone(),
        }).await;
        log::info!("[batch-task] Done: batch_id={}, generated={} questions", batch_id, generated);
    });
}

// ==================== Interview outline ====================

/// Emit the current outline state (status + nodes) to the frontend.
fn emit_outline_updated(store: &dyn Store, session_id: &str, app_handle: &AppHandle) {
    let status = store
        .get_session(session_id)
        .ok()
        .flatten()
        .map(|s| s.outline_status)
        .unwrap_or(OutlineStatus::None);
    let nodes = store.get_outline_nodes(session_id).unwrap_or_default();
    let _ = app_handle.emit(
        EVENT_OUTLINE_UPDATED,
        OutlineUpdatedPayload {
            session_id: session_id.to_string(),
            status: status.as_str().to_string(),
            nodes,
        },
    );
}

/// Auto-mark pending outline nodes as covered once they have at least one
/// answered question and nothing left pending. Emits outline_updated on change.
fn refresh_outline_coverage(store: &dyn Store, session_id: &str, app_handle: &AppHandle) {
    let confirmed = store
        .get_session(session_id)
        .ok()
        .flatten()
        .map(|s| s.outline_status == OutlineStatus::Confirmed)
        .unwrap_or(false);
    if !confirmed {
        return;
    }
    let nodes = match store.get_outline_nodes(session_id) {
        Ok(n) if !n.is_empty() => n,
        _ => return,
    };
    let questions = store.get_questions(session_id).unwrap_or_default();

    let mut updated = nodes.clone();
    let mut changed = false;
    for node in updated.iter_mut() {
        if node.status != OutlineNodeStatus::Pending {
            continue;
        }
        let mut answered = 0usize;
        let mut pending = 0usize;
        for q in &questions {
            if q.outline_node_id.as_deref() == Some(node.id.as_str()) {
                match q.status {
                    QuestionStatus::Answered => answered += 1,
                    QuestionStatus::Ready
                    | QuestionStatus::Generating
                    | QuestionStatus::Stale => pending += 1,
                    QuestionStatus::Skipped => {}
                }
            }
        }
        if answered >= 1 && pending == 0 {
            node.status = OutlineNodeStatus::Covered;
            changed = true;
        }
    }

    if changed {
        if let Err(e) = store.replace_outline_nodes(session_id, &updated) {
            log::error!("[outline] coverage update failed: {}", e);
            return;
        }
        log::info!("[outline] session={}: auto-marked covered nodes", session_id);
        emit_outline_updated(store, session_id, app_handle);
    }
}

/// Generate the interview outline via LLM and move the session to `draft`.
/// On failure the session falls back to `none` so the user can retry or skip.
fn spawn_outline_generation(
    store: Arc<dyn Store>,
    session_id: &str,
    app_handle: &AppHandle,
    scheduler_tx: mpsc::Sender<SchedulerMsg>,
) {
    let settings = match store.get_all_settings() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[outline] Failed to get settings: {}", e);
            let _ = scheduler_tx.try_send(SchedulerMsg::OutlineDone {
                session_id: session_id.to_string(),
            });
            return;
        }
    };

    if settings.api_key.is_empty() {
        let _ = app_handle.emit(
            EVENT_ERROR,
            ErrorPayload {
                session_id: session_id.to_string(),
                message: "未设置 API Key，无法生成访谈大纲。请在设置中配置 API Key。".to_string(),
                kind: "auth".to_string(),
            },
        );
        let _ = scheduler_tx.try_send(SchedulerMsg::OutlineDone {
            session_id: session_id.to_string(),
        });
        return;
    }

    if let Err(e) = store.set_session_outline_status(session_id, OutlineStatus::Generating) {
        log::error!("[outline] set generating failed: {}", e);
        let _ = scheduler_tx.try_send(SchedulerMsg::OutlineDone {
            session_id: session_id.to_string(),
        });
        return;
    }
    emit_outline_updated(&*store, session_id, app_handle);

    let session_id = session_id.to_string();
    let app_handle = app_handle.clone();
    tokio::spawn(async move {
        let session = match store.get_session(&session_id) {
            Ok(Some(s)) => s,
            _ => {
                log::error!("[outline] session not found: {}", session_id);
                return;
            }
        };

        let client = OpenAIClient::new(
            settings.base_url,
            settings.api_key,
            settings.model_name,
            settings.temperature,
        );
        let system_prompt = prompt::build_outline_system_prompt(&session.role);
        let user_prompt = prompt::build_outline_user_prompt(&session);

        match client.generate_summary(&system_prompt, &user_prompt).await {
            Ok(text) => {
                let parsed = prompt::parse_outline_response(&text);
                if parsed.is_empty() {
                    log::warn!("[outline] empty/parse-failed outline for session={}", session_id);
                    let _ =
                        store.set_session_outline_status(&session_id, OutlineStatus::None);
                    let _ = app_handle.emit(
                        EVENT_ERROR,
                        ErrorPayload {
                            session_id: session_id.clone(),
                            message: "访谈大纲生成失败，可点击重试，或跳过直接开始访谈".to_string(),
                            kind: "generation".to_string(),
                        },
                    );
                    emit_outline_updated(&*store, &session_id, &app_handle);
                    return;
                }

                // Normalize node ids: keep provided ones, assign n1..nN when
                // missing or duplicated.
                let mut seen = std::collections::HashSet::new();
                let now = chrono::Utc::now().to_rfc3339();
                let nodes: Vec<OutlineNode> = parsed
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let mut id = n
                            .id
                            .clone()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| format!("n{}", i + 1));
                        while !seen.insert(id.clone()) {
                            id = format!("{}_{}", id, i + 1);
                        }
                        OutlineNode {
                            id,
                            session_id: session_id.clone(),
                            title: n
                                .title
                                .clone()
                                .filter(|t| !t.trim().is_empty())
                                .unwrap_or_else(|| format!("主题 {}", i + 1)),
                            description: n.description.clone(),
                            status: OutlineNodeStatus::Pending,
                            display_order: i as i32,
                            created_at: now.clone(),
                        }
                    })
                    .collect();

                let result = store
                    .replace_outline_nodes(&session_id, &nodes)
                    .and_then(|_| {
                        store.set_session_outline_status(&session_id, OutlineStatus::Draft)
                    });
                if let Err(e) = result {
                    log::error!("[outline] save failed: {}", e);
                    return;
                }
                log::info!(
                    "[outline] session={}: draft outline with {} nodes",
                    session_id,
                    nodes.len()
                );
                emit_outline_updated(&*store, &session_id, &app_handle);
            }
            Err(e) => {
                log::error!("[outline] generation failed: {}", e);
                let _ = store.set_session_outline_status(&session_id, OutlineStatus::None);
                let kind = match e {
                    LlmError::Auth => "auth",
                    LlmError::RateLimited => "rate_limit",
                    _ => "generation",
                };
                let _ = app_handle.emit(
                    EVENT_ERROR,
                    ErrorPayload {
                        session_id: session_id.clone(),
                        message: format!("访谈大纲生成失败: {}（可重试或跳过直接开始）", e),
                        kind: kind.to_string(),
                    },
                );
                emit_outline_updated(&*store, &session_id, &app_handle);
            }
        }

        let _ = scheduler_tx
            .send(SchedulerMsg::OutlineDone {
                session_id: session_id.clone(),
            })
            .await;
    });
}

/// Save a user-edited outline: replace nodes, drop questions for closed nodes,
/// reset saturation, then re-evaluate whether to generate or complete.
fn handle_save_outline(
    store: Arc<dyn Store>,
    state: &mut SchedulerState,
    session_id: &str,
    mut nodes: Vec<OutlineNode>,
    app_handle: &AppHandle,
    tx: mpsc::Sender<SchedulerMsg>,
) -> Result<(), String> {
    // Normalize ordering + session binding
    for (i, n) in nodes.iter_mut().enumerate() {
        n.display_order = i as i32;
        n.session_id = session_id.to_string();
    }

    // Closing a node drops its unanswered questions — they are now out of scope.
    let closed: Vec<String> = nodes
        .iter()
        .filter(|n| n.status != OutlineNodeStatus::Pending)
        .map(|n| n.id.clone())
        .collect();

    store
        .replace_outline_nodes(session_id, &nodes)
        .map_err(|e| e.to_string())?;
    store
        .skip_questions_for_nodes(session_id, &closed)
        .map_err(|e| e.to_string())?;

    // User steered the interview — allow generation again.
    state.get_or_create(session_id).saturated = false;

    refresh_outline_coverage(&*store, session_id, app_handle);
    emit_outline_updated(&*store, session_id, app_handle);
    check_water_levels_and_trigger(store, state, session_id, &[], app_handle, tx);
    Ok(())
}
