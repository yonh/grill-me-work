use crate::model::{DecisionEntry, Question, Session};
use crate::store::{Result, Store};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Markdown,
    Json,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => ExportFormat::Json,
            _ => ExportFormat::Markdown,
        }
    }
}

/// Export a session to a string (markdown or json).
/// The frontend handles file saving.
pub fn export_session(
    store: &dyn Store,
    session_id: &str,
    format: &ExportFormat,
    llm_summary: Option<&str>,
) -> Result<String> {
    let session = store
        .get_session(session_id)?
        .ok_or_else(||
            crate::store::StoreError::NotFound(format!("session {}", session_id))
        )?;

    let decision_summary = store.get_decision_summary(session_id)?;
    let questions = store.get_questions(session_id)?;
    let answered_count = store.count_answered_questions(session_id)?;
    let skipped_count = store.count_skipped_questions(session_id)?;
    let total_count = questions.len() as i32;

    match format {
        ExportFormat::Markdown => Ok(export_markdown(
            &session,
            &decision_summary,
            &questions,
            answered_count,
            skipped_count,
            total_count,
            llm_summary,
        )),
        ExportFormat::Json => Ok(serde_json::to_string_pretty(&decision_summary)?),
    }
}

fn export_markdown(
    session: &Session,
    decision_summary: &[DecisionEntry],
    _questions: &[Question],
    answered_count: i32,
    skipped_count: i32,
    total_count: i32,
    llm_summary: Option<&str>,
) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Grill 会话：{}\n\n", session.title));
    md.push_str(&format!("**日期：** {}\n", session.created_at));
    md.push_str(&format!(
        "**状态：** 已完成（{}/{} 题已答，{} 题跳过）\n\n",
        answered_count, total_count, skipped_count
    ));

    md.push_str("## 已确认的决策\n\n");
    for (i, entry) in decision_summary.iter().enumerate() {
        md.push_str(&format!("### {}. {}\n", i + 1, entry.question));
        md.push_str(&format!("**决策：** {}\n", entry.answer));
        if let Some(rationale) = &entry.rationale {
            md.push_str(&format!("**原因：** {}\n", rationale));
        }
        md.push('\n');
    }

    // Skipped questions
    let skipped: Vec<_> = _questions.iter().filter(|q| q.status == crate::model::QuestionStatus::Skipped).collect();
    if !skipped.is_empty() {
        md.push_str("## 跳过的问题\n");
        for q in skipped {
            md.push_str(&format!("- Q{}: \"{}\" — 用户跳过\n", q.display_order, q.question));
        }
        md.push('\n');
    }

    if let Some(summary) = llm_summary {
        if !summary.is_empty() {
            md.push_str("## LLM 总结\n\n");
            md.push_str(summary);
            md.push('\n');
        }
    }

    md
}
