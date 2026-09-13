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
    pub created_at: String,
    pub updated_at: String,
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
