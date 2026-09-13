use serde::{Deserialize, Serialize};

/// JSON schema description used in the LLM prompt to constrain output shape.
/// This is embedded as text in the system prompt (not sent as a formal JSON Schema).
pub const QUESTION_JSON_TEMPLATE: &str = r#"{"id":"q_xxx","type":"choice|multi|open","category":"intent|choice|open|tradeoff|dependency","question":"题干","context":"上下文说明","options":[{"label":"选项A","description":"说明"}],"recommended_option":"选项A","rationale":"推荐理由","depends_on":["q_xxx"]}"#;

/// A loose schema struct used to parse the JSON inside <question> tags from the LLM.
/// Fields are optional to be tolerant of partial output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmQuestionJson {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub q_type: Option<String>,
    pub category: Option<String>,
    pub question: Option<String>,
    pub context: Option<String>,
    pub options: Option<Vec<LlmOptionJson>>,
    pub recommended_option: Option<String>,
    pub rationale: Option<String>,
    pub depends_on: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOptionJson {
    pub label: Option<String>,
    pub description: Option<String>,
}
