pub mod sqlite;

use crate::model::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub trait Store: Send + Sync + 'static {
    fn create_session(&self, session: &Session) -> Result<()>;
    fn delete_session(&self, id: &str) -> Result<()>;
    fn get_sessions(&self) -> Result<Vec<Session>>;
    fn get_session(&self, id: &str) -> Result<Option<Session>>;
    fn get_questions(&self, session_id: &str) -> Result<Vec<Question>>;
    fn insert_question(&self, q: &Question) -> Result<()>;
    fn update_question_status(&self, id: &str, status: QuestionStatus) -> Result<()>;
    fn update_question_answer(
        &self,
        id: &str,
        answer: &AnswerValue,
        version: u32,
    ) -> Result<()>;
    fn mark_stale(&self, question_ids: &[String]) -> Result<()>;
    fn get_answered_questions(&self, session_id: &str) -> Result<Vec<Question>>;
    fn get_decision_summary(&self, session_id: &str) -> Result<Vec<DecisionEntry>>;
    fn upsert_decision_entry(&self, session_id: &str, entry: &DecisionEntry) -> Result<()>;
    fn insert_batch(&self, batch: &Batch) -> Result<()>;
    fn update_batch_status(&self, id: &str, status: BatchStatus) -> Result<()>;
    fn get_max_batch_no(&self, session_id: &str) -> Result<i32>;
    fn get_max_display_order(&self, session_id: &str) -> Result<i32>;
    fn count_ready_questions(&self, session_id: &str) -> Result<i32>;
    fn get_setting(&self, key: &str) -> Result<Option<String>>;
    fn set_setting(&self, key: &str, value: &str) -> Result<()>;
    fn get_all_settings(&self) -> Result<Settings>;
    fn save_settings(&self, settings: &Settings) -> Result<()>;
    fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()>;
    fn save_session_summary(&self, id: &str, summary: &str) -> Result<()>;
    fn save_prototype_version_meta(&self, id: &str, version: i32) -> Result<()>;
    fn save_prototype_version(&self, version: &PrototypeVersion) -> Result<()>;
    fn get_prototype_versions(&self, session_id: &str) -> Result<Vec<PrototypeVersion>>;
    fn get_latest_prototype_snapshot(&self, session_id: &str) -> Result<Option<PrototypeVersion>>;
    /// Find all questions in a session that depend (directly) on the given question id.
    fn find_dependents(&self, session_id: &str, question_id: &str) -> Result<Vec<String>>;
    /// Get a question by id.
    fn get_question(&self, id: &str) -> Result<Option<Question>>;
    /// Count answered + skipped questions in a session.
    fn count_answered_questions(&self, session_id: &str) -> Result<i32>;
    /// Count skipped questions in a session.
    fn count_skipped_questions(&self, session_id: &str) -> Result<i32>;
    // Chat messages
    fn insert_message(&self, msg: &ChatMessage) -> Result<()>;
    fn get_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>>;
    fn add_message_id_to_question(&self, question_id: &str, message_id: &str) -> Result<()>;
    // Interview outline
    fn get_outline_nodes(&self, session_id: &str) -> Result<Vec<OutlineNode>>;
    /// Replace the whole outline for a session (add/edit/delete/reorder/toggle in one call).
    /// Clears `outline_node_id` on questions pointing at removed nodes.
    fn replace_outline_nodes(&self, session_id: &str, nodes: &[OutlineNode]) -> Result<()>;
    fn set_session_outline_status(&self, session_id: &str, status: OutlineStatus) -> Result<()>;
    /// Skip all ready/stale questions attached to the given outline nodes.
    fn skip_questions_for_nodes(&self, session_id: &str, node_ids: &[String]) -> Result<()>;
    fn get_skipped_questions(&self, session_id: &str) -> Result<Vec<Question>>;
}
