pub mod role;
pub mod schema;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub initial_context: Option<String>,
    pub role: String,
    pub status: SessionStatus,
    pub summary: Option<String>,
    /// Multi-file prototype lives on disk; this is only a version counter.
    pub prototype_version: i32,
    /// Interview outline lifecycle: none | generating | draft | confirmed
    pub outline_status: OutlineStatus,
    /// Pipeline stage gating development: none(legacy) | interviewing | spec_draft | tickets_draft | developing
    pub pipeline_stage: PipelineStage,
    /// Product spec markdown (draft or confirmed; mirrored to docs/spec.md on confirm).
    /// This is the *live working state of the current round* — archived rounds
    /// keep their own snapshot in `rounds.spec`.
    pub spec: Option<String>,
    /// The active development round; NULL between rounds (after archive, before start).
    pub current_round_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// One development cycle (轮次) inside a project: interview → spec → tickets →
/// branch-map development → archive. All round-scoped rows (questions, outline
/// nodes, tickets, messages, decisions, batches) carry `round_id` and are only
/// visible while the round is the session's current round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round {
    pub id: String,
    pub session_id: String,
    /// 1-based cycle number inside the project.
    pub number: i32,
    /// Task name, e.g. "首屏静态画面" / "动作效果".
    pub title: String,
    /// Direction for this round's interview — feeds the outline prompt.
    pub goal: Option<String>,
    pub status: RoundStatus,
    /// Pipeline stage snapshot taken at archive time.
    pub pipeline_stage: PipelineStage,
    /// Spec snapshot taken at archive time.
    pub spec: Option<String>,
    /// LLM/heuristic summary snapshot taken at archive time.
    pub summary: Option<String>,
    pub created_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoundStatus {
    Active,
    /// Archived — data preserved but out of the working area.
    Archived,
}

impl RoundStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoundStatus::Active => "active",
            RoundStatus::Archived => "archived",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "archived" => RoundStatus::Archived,
            _ => RoundStatus::Active,
        }
    }
}

/// Full read of one round's data — used for archive snapshots / history view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundArchive {
    pub round: Round,
    pub nodes: Vec<OutlineNode>,
    pub questions: Vec<Question>,
    pub tickets: Vec<Ticket>,
    pub messages: Vec<ChatMessage>,
    pub decisions: Vec<DecisionEntry>,
}

/// Session pipeline stage — the spec→tickets gate before development.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    /// Legacy/free-running session — development ungated.
    None,
    /// Interview running (outline + questions).
    Interviewing,
    /// Spec generated, awaiting user confirmation.
    SpecDraft,
    /// Tickets generated, awaiting user confirmation.
    TicketsDraft,
    /// Tickets confirmed — dev lines may run.
    Developing,
}

impl PipelineStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            PipelineStage::None => "none",
            PipelineStage::Interviewing => "interviewing",
            PipelineStage::SpecDraft => "spec_draft",
            PipelineStage::TicketsDraft => "tickets_draft",
            PipelineStage::Developing => "developing",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "interviewing" => PipelineStage::Interviewing,
            "spec_draft" => PipelineStage::SpecDraft,
            "tickets_draft" => PipelineStage::TicketsDraft,
            "developing" => PipelineStage::Developing,
            _ => PipelineStage::None,
        }
    }
}

/// One development ticket — a scoped work item the agent can implement on its own branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TicketStatus,
    /// Ticket ids this ticket depends on.
    pub depends_on: Vec<String>,
    /// Git branch names spawned for this ticket (gacha lines).
    pub branches: Vec<String>,
    pub display_order: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Pending,
    InProgress,
    Done,
    /// User reviewed and dropped this ticket.
    Rejected,
}

impl TicketStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TicketStatus::Pending => "pending",
            TicketStatus::InProgress => "in_progress",
            TicketStatus::Done => "done",
            TicketStatus::Rejected => "rejected",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => TicketStatus::InProgress,
            "done" => TicketStatus::Done,
            "rejected" => TicketStatus::Rejected,
            _ => TicketStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutlineStatus {
    /// No outline — legacy free-running interview.
    None,
    /// LLM is generating the outline.
    Generating,
    /// Outline generated, waiting for user confirmation.
    Draft,
    /// User confirmed; question batches are scoped to uncovered nodes.
    Confirmed,
}

impl OutlineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutlineStatus::None => "none",
            OutlineStatus::Generating => "generating",
            OutlineStatus::Draft => "draft",
            OutlineStatus::Confirmed => "confirmed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "generating" => OutlineStatus::Generating,
            "draft" => OutlineStatus::Draft,
            "confirmed" => OutlineStatus::Confirmed,
            _ => OutlineStatus::None,
        }
    }
}

/// One topic node in the interview outline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: OutlineNodeStatus,
    pub display_order: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutlineNodeStatus {
    /// Still needs questions.
    Pending,
    /// Sufficiently covered (auto when it has answers and no pending questions, or set by user).
    Covered,
    /// User unchecked it — never generate questions for it.
    Excluded,
}

impl OutlineNodeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutlineNodeStatus::Pending => "pending",
            OutlineNodeStatus::Covered => "covered",
            OutlineNodeStatus::Excluded => "excluded",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "covered" => OutlineNodeStatus::Covered,
            "excluded" => OutlineNodeStatus::Excluded,
            _ => OutlineNodeStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Completed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "completed" => SessionStatus::Completed,
            _ => SessionStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub session_id: String,
    pub batch_id: String,
    pub q_type: QuestionType,
    pub category: QuestionCategory,
    pub question: String,
    pub context: Option<String>,
    pub options: Vec<QuestionOption>,
    pub recommended_option: Option<String>,
    pub rationale: Option<String>,
    pub depends_on: Vec<String>,
    pub status: QuestionStatus,
    pub answer: Option<AnswerValue>,
    pub answer_version: u32,
    pub display_order: i32,
    pub message_id: Option<String>,
    /// Outline node this question belongs to (None = unscoped / legacy).
    pub outline_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    Choice,
    Multi,
    Open,
}

impl QuestionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionType::Choice => "choice",
            QuestionType::Multi => "multi",
            QuestionType::Open => "open",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "multi" => QuestionType::Multi,
            "open" => QuestionType::Open,
            _ => QuestionType::Choice,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionCategory {
    Intent,
    Choice,
    Open,
    Tradeoff,
    Dependency,
}

impl QuestionCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionCategory::Intent => "intent",
            QuestionCategory::Choice => "choice",
            QuestionCategory::Open => "open",
            QuestionCategory::Tradeoff => "tradeoff",
            QuestionCategory::Dependency => "dependency",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "choice" => QuestionCategory::Choice,
            "open" => QuestionCategory::Open,
            "tradeoff" => QuestionCategory::Tradeoff,
            "dependency" => QuestionCategory::Dependency,
            _ => QuestionCategory::Intent,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Generating,
    Ready,
    Answered,
    Skipped,
    Stale,
}

impl QuestionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestionStatus::Generating => "generating",
            QuestionStatus::Ready => "ready",
            QuestionStatus::Answered => "answered",
            QuestionStatus::Skipped => "skipped",
            QuestionStatus::Stale => "stale",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "ready" => QuestionStatus::Ready,
            "answered" => QuestionStatus::Answered,
            "skipped" => QuestionStatus::Skipped,
            "stale" => QuestionStatus::Stale,
            _ => QuestionStatus::Generating,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnswerValue {
    Choice { option: String },
    Multi { options: Vec<String> },
    Open { text: String },
}

impl AnswerValue {
    /// Convert answer to a human-readable string for the decision summary.
    pub fn to_human_string(&self) -> String {
        match self {
            AnswerValue::Choice { option } => option.clone(),
            AnswerValue::Multi { options } => options.join(", "),
            AnswerValue::Open { text } => text.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub id: String,
    pub session_id: String,
    pub batch_no: i32,
    pub status: BatchStatus,
    pub triggered_by_answers: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Generating,
    Done,
    Failed,
}

impl BatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BatchStatus::Generating => "generating",
            BatchStatus::Done => "done",
            BatchStatus::Failed => "failed",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "done" => BatchStatus::Done,
            "failed" => BatchStatus::Failed,
            _ => BatchStatus::Generating,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub temperature: f64,
    pub batch_size: i32,
    pub max_concurrent_batches: i32,
    pub debounce_seconds: i32,
    pub questions_per_screen: i32,
    /// Coding agent used to write the prototype: opencode | codex | devin | agy
    pub agent_tool: String,
    /// Model id passed to the agent CLI (may be empty for tool default)
    pub agent_model: String,
    /// Optional effort/variant: auto | low | medium | high | xhigh | max
    pub agent_effort: String,
    /// Auto-approve agent file writes
    pub agent_auto_approve: bool,
    /// Pause auto prototype generation after answers (manual generate still works)
    pub prototype_auto_paused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model_name: "gpt-4o".to_string(),
            temperature: 0.7,
            batch_size: 5,
            max_concurrent_batches: 2,
            debounce_seconds: 10,
            questions_per_screen: 5,
            agent_tool: "opencode".to_string(),
            agent_model: String::new(),
            agent_effort: "auto".to_string(),
            agent_auto_approve: true,
            prototype_auto_paused: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub question_id: String,
    pub question: String,
    pub answer: String,
    pub rationale: Option<String>,
    pub category: QuestionCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypeVersion {
    pub id: String,
    pub session_id: String,
    pub version: i32,
    /// JSON snapshot of the multi-file prototype: { files: [{path, content}], changelog }
    pub snapshot_json: String,
    pub feedback: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String, // "user" | "assistant"
    pub content: String,
    pub question_ids: Vec<String>,
    pub created_at: String,
}
