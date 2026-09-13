use crate::llm::{LlmError, QuestionSource};
use crate::model::schema::LlmQuestionJson;
use crate::model::*;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub struct OpenAIClient {
    base_url: String,
    api_key: String,
    model: String,
    temperature: f64,
}

impl OpenAIClient {
    pub fn new(base_url: String, api_key: String, model: String, temperature: f64) -> Self {
        OpenAIClient {
            base_url,
            api_key,
            model,
            temperature,
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ApiMessage>,
    temperature: f64,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Clone)]
pub struct ApiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ApiFunction,
}

#[derive(Serialize, Clone)]
pub struct ApiFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatStreamChunk {
    choices: Vec<ChatStreamChoice>,
}

#[derive(Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
}

#[derive(Deserialize)]
struct ChatStreamDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatStreamToolCall>>,
}

#[derive(Deserialize)]
struct ChatStreamToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChatStreamToolFunction>,
}

#[derive(Deserialize)]
struct ChatStreamToolFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Parsed tool call result
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Deserialize)]
struct ChatNonStreamResponse {
    choices: Vec<ChatNonStreamChoice>,
}

#[derive(Deserialize)]
struct ChatNonStreamChoice {
    message: ApiMessage,
}

impl OpenAIClient {
    fn build_headers(&self) -> Result<HeaderMap, LlmError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if self.api_key.is_empty() {
            return Err(LlmError::Auth);
        }
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|_| LlmError::Auth)?,
        );
        Ok(headers)
    }

    fn chat_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        format!("{}/chat/completions", base)
    }

    /// Stream a chat completion with tool calling support.
    /// Calls `on_text` for each text delta, returns complete text and tool_calls at the end.
    pub async fn chat_with_tools(
        &self,
        messages: &[ApiMessage],
        tools: &[ApiTool],
        on_text: &mut (impl FnMut(&str) + Send),
    ) -> Result<(String, Vec<ToolCall>), LlmError> {
        let headers = self.build_headers()?;
        let chat_url = self.chat_url();

        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: self.temperature,
            stream: true,
            tools: Some(tools.to_vec()),
            tool_choice: Some("auto".to_string()),
        };

        log::info!("[llm] POST {} (model={}, stream=true, tools={})", chat_url, self.model, tools.len());

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()?;

        let response = http_client
            .post(&chat_url)
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if status.as_u16() == 401 {
            return Err(LlmError::Auth);
        }
        if status.as_u16() == 429 {
            return Err(LlmError::RateLimited);
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::Server(format!("{}: {}", status, body)));
        }

        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();
        let mut full_text = String::new();
        // Accumulate tool_calls by index
        let mut tool_call_names: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
        let mut tool_call_args: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
        let mut total_chunks = 0;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            total_chunks += 1;
            let text = String::from_utf8_lossy(&chunk);
            sse_buffer.push_str(&text);

            let parts: Vec<String> = sse_buffer.split('\n').map(|s| s.to_string()).collect();
            let n = parts.len();
            sse_buffer = parts[n - 1].clone();

            for line in &parts[..n.saturating_sub(1)] {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let data = if let Some(d) = line.strip_prefix("data: ") {
                    d
                } else if let Some(d) = line.strip_prefix("data:") {
                    d
                } else {
                    continue;
                };

                if data.trim() == "[DONE]" {
                    continue;
                }

                if let Ok(chunk_data) = serde_json::from_str::<ChatStreamChunk>(data) {
                    if let Some(choice) = chunk_data.choices.first() {
                        // Text content
                        if let Some(content) = &choice.delta.content {
                            full_text.push_str(content);
                            on_text(content);
                        }
                        // Tool calls
                        if let Some(tc_list) = &choice.delta.tool_calls {
                            for tc in tc_list {
                                if let Some(name) = tc.function.as_ref().and_then(|f| f.name.clone()) {
                                    tool_call_names.insert(tc.index, name);
                                }
                                if let Some(args) = &tc.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                    tool_call_args.entry(tc.index).or_default().push_str(args);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Build final tool calls list
        let mut tool_calls = Vec::new();
        let mut indices: Vec<usize> = tool_call_names.keys().cloned().collect();
        indices.sort();
        for idx in indices {
            let name = tool_call_names.get(&idx).cloned().unwrap_or_default();
            let args = tool_call_args.get(&idx).cloned().unwrap_or_default();
            tool_calls.push(ToolCall { name, arguments: args });
        }

        log::info!(
            "[llm] Stream ended. {} chunks, {} text chars, {} tool_calls",
            total_chunks, full_text.len(), tool_calls.len()
        );

        Ok((full_text, tool_calls))
    }

    /// Parse the XML buffer, extracting complete <question>...</question> blocks.
    /// Returns (extracted_questions, remaining_buffer).
    pub fn parse_questions_from_buffer(
        buffer: &str,
        session_id: &str,
        batch_id: &str,
        display_order_offset: &mut i32,
    ) -> (Vec<Question>, String) {
        let mut questions = Vec::new();
        let mut remaining = buffer.to_string();

        loop {
            // Find the next closing tag
            let close_pos = match remaining.find("</question>") {
                Some(pos) => pos,
                None => break,
            };

            // Find the nearest opening tag before the closing tag
            let open_pos = match remaining[..close_pos].rfind("<question>") {
                Some(pos) => pos,
                None => {
                    // No opening tag found; discard everything up to and including the closing tag
                    remaining = remaining[close_pos + "</question>".len()..].to_string();
                    continue;
                }
            };

            let inner = &remaining[open_pos + "<question>".len()..close_pos];
            let inner = inner.trim();

            // Try to parse the JSON
            if let Ok(llm_q) = serde_json::from_str::<LlmQuestionJson>(inner) {
                if let Some(q) = Self::llm_question_to_question(
                    &llm_q,
                    session_id,
                    batch_id,
                    *display_order_offset,
                ) {
                    *display_order_offset += 1;
                    questions.push(q);
                } else {
                    log::warn!("Skipping question with missing required fields: {}", inner);
                }
            } else {
                log::warn!("Failed to parse question JSON: {}", inner);
            }

            // Remove processed text
            remaining = remaining[close_pos + "</question>".len()..].to_string();
        }

        (questions, remaining)
    }

    fn llm_question_to_question(
        llm_q: &LlmQuestionJson,
        session_id: &str,
        batch_id: &str,
        display_order: i32,
    ) -> Option<Question> {
        let question_text = llm_q.question.clone()?;
        let q_type = llm_q
            .q_type
            .as_deref()
            .map(QuestionType::from_str)
            .unwrap_or(QuestionType::Choice);
        let category = llm_q
            .category
            .as_deref()
            .map(QuestionCategory::from_str)
            .unwrap_or(QuestionCategory::Intent);

        let id = llm_q
            .id
            .clone()
            .unwrap_or_else(|| format!("q_{}", uuid::Uuid::new_v4()));

        let options: Vec<QuestionOption> = llm_q
            .options
            .as_ref()
            .map(|opts| {
                opts.iter()
                    .filter_map(|o| {
                        o.label.clone().map(|label| QuestionOption {
                            label,
                            description: o.description.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(Question {
            id,
            session_id: session_id.to_string(),
            batch_id: batch_id.to_string(),
            q_type,
            category,
            question: question_text,
            context: llm_q.context.clone(),
            options,
            recommended_option: llm_q.recommended_option.clone(),
            rationale: llm_q.rationale.clone(),
            depends_on: llm_q.depends_on.clone().unwrap_or_default(),
            status: QuestionStatus::Ready,
            answer: None,
            answer_version: 0,
            display_order,
            message_id: None,
            outline_node_id: llm_q.node_id.clone(),
        })
    }
}

impl QuestionSource for OpenAIClient {
    fn generate_batch(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _batch_size: i32,
        on_question: Box<dyn FnMut(Question) + Send>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LlmError>> + Send>> {
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let temperature = self.temperature;
        let system_prompt = system_prompt.to_string();
        let user_prompt = user_prompt.to_string();

        Box::pin(async move {
            let client = OpenAIClient::new(base_url, api_key, model, temperature);
            let headers = client.build_headers()?;

            let chat_url = client.chat_url();
            log::info!("[llm] POST {} (model={}, stream=true)", chat_url, client.model);
            log::info!("[llm] system_prompt len={} chars, user_prompt len={} chars", system_prompt.len(), user_prompt.len());

            let request = ChatRequest {
                model: client.model.clone(),
                messages: vec![
                    ApiMessage {
                        role: "system".to_string(),
                        content: system_prompt,
                    },
                    ApiMessage {
                        role: "user".to_string(),
                        content: user_prompt,
                    },
                ],
                temperature: client.temperature,
                stream: true,
                tools: None,
                tool_choice: None,
            };

            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()?;

            let response = http_client
                .post(&chat_url)
                .headers(headers)
                .json(&request)
                .send()
                .await?;

            let status = response.status();
            log::info!("[llm] Response status: {}", status);
            if status.as_u16() == 401 {
                log::error!("[llm] Auth error (401)");
                return Err(LlmError::Auth);
            }
            if status.as_u16() == 429 {
                log::error!("[llm] Rate limited (429)");
                return Err(LlmError::RateLimited);
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                log::error!("[llm] Server error {}: {}", status, body.chars().take(200).collect::<String>());
                return Err(LlmError::Server(format!("{}: {}", status, body)));
            }

            log::info!("[llm] Stream started, reading chunks...");
            let mut stream = response.bytes_stream();
            let mut sse_buffer = String::new();      // raw SSE bytes (may contain partial lines)
            let mut content_buffer = String::new();  // accumulated delta.content text
            let mut display_order_offset: i32 = 0;
            let mut on_question = on_question;
            let mut total_chunks = 0;
            let mut total_content_chars = 0;

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                total_chunks += 1;
                let text = String::from_utf8_lossy(&chunk);
                sse_buffer.push_str(&text);

                // Split SSE buffer into complete lines + leftover
                let mut done = false;
                let parts: Vec<String> = sse_buffer.split('\n').map(|s| s.to_string()).collect();
                let n = parts.len();
                // Last element after split is either empty (if ended with \n) or a partial line
                sse_buffer = parts[n - 1].clone();

                for line in &parts[..n.saturating_sub(1)] {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data.trim() == "[DONE]" {
                            done = true;
                            continue;
                        }
                        if let Ok(chunk_data) = serde_json::from_str::<ChatStreamChunk>(data) {
                            if let Some(choice) = chunk_data.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    total_content_chars += content.len();
                                    content_buffer.push_str(content);
                                }
                            }
                        }
                    } else if let Some(data) = line.strip_prefix("data:") {
                        if data.trim() == "[DONE]" {
                            done = true;
                            continue;
                        }
                        if let Ok(chunk_data) = serde_json::from_str::<ChatStreamChunk>(data) {
                            if let Some(choice) = chunk_data.choices.first() {
                                if let Some(content) = &choice.delta.content {
                                    total_content_chars += content.len();
                                    content_buffer.push_str(content);
                                }
                            }
                        }
                    }
                }

                // Parse questions from the accumulated content buffer
                let (questions, remaining) = Self::parse_questions_from_buffer(
                    &content_buffer,
                    "",
                    "",
                    &mut display_order_offset,
                );
                if !questions.is_empty() {
                    log::info!("[llm] Parsed {} questions from stream (chunk #{}, content_buffer now {} chars)", questions.len(), total_chunks, content_buffer.len());
                }
                for q in questions {
                    on_question(q);
                }
                content_buffer = remaining;

                if done {
                    log::info!("[llm] Stream [DONE] received after {} chunks, {} content chars", total_chunks, total_content_chars);
                    break;
                }
            }

            log::info!("[llm] Stream ended. Total chunks={}, content chars={}", total_chunks, total_content_chars);
            let open_count = content_buffer.matches("<question>").count();
            let close_count = content_buffer.matches("</question>").count();
            log::info!("[llm] Final content buffer ({} chars): <question> tags={}, </question> tags={}", content_buffer.len(), open_count, close_count);
            log::info!("[llm] Final content buffer preview: {}", content_buffer.chars().take(500).collect::<String>());

            // Process any remaining content buffer
            let (questions, remaining) = Self::parse_questions_from_buffer(
                &content_buffer,
                "",
                "",
                &mut display_order_offset,
            );
            if !questions.is_empty() {
                log::info!("[llm] Parsed {} questions from remaining buffer", questions.len());
            }
            for q in questions {
                on_question(q);
            }
            if !remaining.trim().is_empty() {
                log::warn!("[llm] Unparsed remaining buffer ({} chars): {}...", remaining.len(), remaining.chars().take(200).collect::<String>());
            }

            Ok(())
        })
    }

    fn generate_summary(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, LlmError>> + Send>> {
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let temperature = self.temperature;
        let system_prompt = system_prompt.to_string();
        let user_prompt = user_prompt.to_string();

        Box::pin(async move {
            let client = OpenAIClient::new(base_url, api_key, model, temperature);
            let headers = client.build_headers()?;

            let request = ChatRequest {
                model: client.model.clone(),
                messages: vec![
                    ApiMessage {
                        role: "system".to_string(),
                        content: system_prompt,
                    },
                    ApiMessage {
                        role: "user".to_string(),
                        content: user_prompt,
                    },
                ],
                temperature: client.temperature,
                stream: false,
                tools: None,
                tool_choice: None,
            };

            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()?;

            let response = http_client
                .post(client.chat_url())
                .headers(headers)
                .json(&request)
                .send()
                .await?;

            let status = response.status();
            if status.as_u16() == 401 {
                return Err(LlmError::Auth);
            }
            if status.as_u16() == 429 {
                return Err(LlmError::RateLimited);
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(LlmError::Server(format!("{}: {}", status, body)));
            }

            let resp: ChatNonStreamResponse = response.json().await?;
            let content = resp
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .unwrap_or_default();

            Ok(content)
        })
    }
}
