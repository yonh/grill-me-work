pub mod design_system;
pub mod openai;
pub mod prompt;

use crate::model::Question;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum LlmError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("auth error: invalid api key")]
    Auth,
    #[error("rate limited (429)")]
    RateLimited,
    #[error("server error: {0}")]
    Server(String),
    #[error("timeout")]
    Timeout,
    #[error("stream ended unexpectedly")]
    StreamEnded,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("other: {0}")]
    Other(String),
}

pub trait QuestionSource: Send + Sync + 'static {
    /// Stream questions for a batch. Calls `on_question` for each parsed question.
    /// Returns Ok(()) on completion, Err on failure.
    fn generate_batch(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        batch_size: i32,
        on_question: Box<dyn FnMut(Question) + Send>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), LlmError>> + Send>,
    >;

    /// Generate a summary for a completed session. Returns the summary text.
    fn generate_summary(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send>,
    >;
}
