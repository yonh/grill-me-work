use crate::model::Question;
use serde::Serialize;

pub const EVENT_NEW_QUESTION: &str = "new_question";
pub const EVENT_STALE_MARKED: &str = "stale_marked";
pub const SESSION_COMPLETE: &str = "session_complete";
pub const EVENT_ERROR: &str = "error";
pub const EVENT_BATCH_STATUS: &str = "batch_status";
pub const EVENT_PROTOTYPE_STATUS: &str = "prototype_status";
pub const EVENT_PROTOTYPE_UPDATED: &str = "prototype_updated";
pub const EVENT_AGENT_OUTPUT: &str = "agent_output";
pub const EVENT_GRAPH_PROGRESS: &str = "graph_progress";
pub const EVENT_GRAPH_NODE_STATUS: &str = "graph_node_status";
pub const EVENT_TIMELINE_UPDATED: &str = "timeline_updated";
pub const EVENT_ACTIVE_SESSION: &str = "active_session_changed";

#[derive(Debug, Clone, Serialize)]
pub struct NewQuestionPayload {
    pub session_id: String,
    pub question: Question,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaleMarkedPayload {
    pub session_id: String,
    pub question_ids: Vec<String>,
    pub triggered_by_question_id: String,
    pub triggered_by_question_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionCompletePayload {
    pub session_id: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub session_id: String,
    pub message: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchStatusPayload {
    pub session_id: String,
    pub status: String,
    pub remaining: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrototypeStatusPayload {
    pub session_id: String,
    /// generating | done | failed
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrototypeUpdatedPayload {
    pub session_id: String,
    pub version: i32,
    pub preview_url: String,
    pub changelog: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentOutputPayload {
    pub session_id: String,
    pub stream: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphProgressPayload {
    pub session_id: String,
    /// started | step_start | step_done | step_failed | done | cancelled
    pub status: String,
    pub step_index: u32,
    pub step_total: u32,
    pub node_id: Option<String>,
    pub label: Option<String>,
    pub message: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNodeStatusPayload {
    pub session_id: String,
    pub node_id: String,
    pub status: String,
    pub round: Option<u32>,
    pub total_rounds: Option<u32>,
    pub commit: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineUpdatedPayload {
    pub session_id: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub commit_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveSessionPayload {
    pub session_id: Option<String>,
    pub title: Option<String>,
}
