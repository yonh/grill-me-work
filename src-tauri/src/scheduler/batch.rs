//! Batch state machine helpers.
//! The batch lifecycle is managed by the scheduler actor, but this module
//! provides utility functions for batch state transitions.

#![allow(dead_code)]

use crate::model::{Batch, BatchStatus};

/// Check if a batch is in a terminal state (done or failed).
pub fn is_terminal(status: &BatchStatus) -> bool {
    matches!(status, BatchStatus::Done | BatchStatus::Failed)
}

/// Check if a batch is currently generating.
pub fn is_generating(status: &BatchStatus) -> bool {
    matches!(status, BatchStatus::Generating)
}

/// Create a new batch with the given parameters.
pub fn create_batch(
    session_id: &str,
    batch_no: i32,
    triggered_by_answers: Vec<String>,
) -> Batch {
    Batch {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        batch_no,
        status: BatchStatus::Generating,
        triggered_by_answers,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}
