#[cfg(test)]
mod tests {
    use grill_me_v2_lib::llm::openai::OpenAIClient;
    use grill_me_v2_lib::model::*;
    use grill_me_v2_lib::store::{Result, Store, StoreError};

    // ============ XML tag splitting tests ============

    #[test]
    fn test_xml_tag_split_single_question() {
        let buffer = r#"<question>{"id":"q_001","type":"choice","question":"GUI 框架？","options":[{"label":"Tauri","description":"好"}]}</question>"#;
        let mut offset = 0;
        let (questions, remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].id, "q_001");
        assert_eq!(questions[0].question, "GUI 框架？");
        assert_eq!(questions[0].q_type, QuestionType::Choice);
        assert_eq!(questions[0].options.len(), 1);
        assert_eq!(questions[0].options[0].label, "Tauri");
        assert!(remaining.is_empty());
        assert_eq!(offset, 1);
    }

    #[test]
    fn test_xml_tag_split_multiple_questions() {
        let buffer = r#"<question>{"id":"q_1","type":"choice","question":"A？"}</question>
<question>{"id":"q_2","type":"open","question":"B？"}</question>"#;
        let mut offset = 0;
        let (questions, remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].id, "q_1");
        assert_eq!(questions[1].id, "q_2");
        assert_eq!(questions[1].q_type, QuestionType::Open);
        assert!(remaining.is_empty());
        assert_eq!(offset, 2);
    }

    #[test]
    fn test_xml_tag_partial_buffer_waits() {
        // Incomplete closing tag — should not extract anything
        let buffer = r#"<question>{"id":"q_1","type":"choice","question":"A？""#;
        let mut offset = 0;
        let (questions, remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 0);
        assert!(!remaining.is_empty()); // buffer preserved for next round
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_xml_tag_split_with_markdown_noise() {
        let buffer = r#"好的，我来生成问题：

<question>{"id":"q_1","type":"choice","question":"用什么语言？","options":[{"label":"Rust","description":"快"}]}</question>

下一题：
<question>{"id":"q_2","type":"multi","question":"支持哪些平台？","options":[{"label":"macOS"},{"label":"Linux"}]}</question>

希望对你有帮助！"#;
        let mut offset = 0;
        let (questions, remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].id, "q_1");
        assert_eq!(questions[1].id, "q_2");
        assert_eq!(questions[1].q_type, QuestionType::Multi);
        assert_eq!(questions[1].options.len(), 2);
        // remaining should only have the trailing noise text
        assert!(remaining.contains("希望对你有帮助"));
        assert_eq!(offset, 2);
    }

    #[test]
    fn test_xml_tag_malformed_discarded() {
        // Closing tag without opening tag
        let buffer = r#"random text </question><question>{"id":"q_1","type":"choice","question":"A？"}</question>"#;
        let mut offset = 0;
        let (questions, _remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].id, "q_1");
        assert_eq!(offset, 1);
    }

    #[test]
    fn test_xml_tag_json_parse_failure_discarded() {
        let buffer = r#"<question>{invalid json}</question><question>{"id":"q_2","type":"open","question":"B？"}</question>"#;
        let mut offset = 0;
        let (questions, _remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        // First question has invalid JSON, should be discarded; second should succeed
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].id, "q_2");
        assert_eq!(offset, 1);
    }

    #[test]
    fn test_xml_tag_incremental_streaming() {
        // Simulate streaming: first chunk has partial content, second completes it
        let mut full_buffer = String::new();
        let mut offset = 0;
        let mut all_questions = Vec::new();

        let chunk1 = r#"<question>{"id":"q_1","type":"choice","#;
        full_buffer.push_str(chunk1);
        let (qs, rem) = OpenAIClient::parse_questions_from_buffer(&full_buffer, "s1", "b1", &mut offset);
        assert_eq!(qs.len(), 0);
        all_questions.extend(qs);
        full_buffer = rem;

        let chunk2 = r#""question":"A？","options":[{"label":"X"}]}</question>"#;
        full_buffer.push_str(chunk2);
        let (qs, rem) = OpenAIClient::parse_questions_from_buffer(&full_buffer, "s1", "b1", &mut offset);
        assert_eq!(qs.len(), 1);
        all_questions.extend(qs);
        full_buffer = rem;

        assert_eq!(all_questions.len(), 1);
        assert_eq!(all_questions[0].id, "q_1");
        assert_eq!(all_questions[0].question, "A？");
        assert!(full_buffer.is_empty());
    }

    #[test]
    fn test_xml_tag_missing_question_field_skipped() {
        // Missing required "question" field
        let buffer = r#"<question>{"id":"q_1","type":"choice"}</question>"#;
        let mut offset = 0;
        let (questions, _remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 0);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_xml_tag_default_type_and_category() {
        // Omit type and category — should default
        let buffer = r#"<question>{"id":"q_1","question":"A？"}</question>"#;
        let mut offset = 0;
        let (questions, _remaining) = OpenAIClient::parse_questions_from_buffer(
            buffer,
            "s1",
            "b1",
            &mut offset,
        );
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].q_type, QuestionType::Choice); // default
        assert_eq!(questions[0].category, QuestionCategory::Intent); // default
    }

    // ============ AnswerValue serde tests ============

    #[test]
    fn test_answer_value_choice_serde() {
        let answer = AnswerValue::Choice {
            option: "Tauri".to_string(),
        };
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("\"kind\":\"choice\""));
        assert!(json.contains("\"option\":\"Tauri\""));
        let de: AnswerValue = serde_json::from_str(&json).unwrap();
        match de {
            AnswerValue::Choice { option } => assert_eq!(option, "Tauri"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_answer_value_multi_serde() {
        let answer = AnswerValue::Multi {
            options: vec!["A".to_string(), "B".to_string()],
        };
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("\"kind\":\"multi\""));
        let de: AnswerValue = serde_json::from_str(&json).unwrap();
        match de {
            AnswerValue::Multi { options } => {
                assert_eq!(options.len(), 2);
                assert_eq!(options[0], "A");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_answer_value_open_serde() {
        let answer = AnswerValue::Open {
            text: "some text".to_string(),
        };
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("\"kind\":\"open\""));
        let de: AnswerValue = serde_json::from_str(&json).unwrap();
        match de {
            AnswerValue::Open { text } => assert_eq!(text, "some text"),
            _ => panic!("wrong variant"),
        }
    }

    // ============ In-memory store mock for scheduler/export tests ============

    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockStore {
        sessions: Mutex<HashMap<String, Session>>,
        questions: Mutex<HashMap<String, Question>>,
        decision_summary: Mutex<HashMap<String, DecisionEntry>>,
        batches: Mutex<HashMap<String, Batch>>,
        settings: Mutex<HashMap<String, String>>,
        next_batch_no: Mutex<i32>,
        tickets: Mutex<HashMap<String, Ticket>>,
        rounds: Mutex<HashMap<String, Round>>,
    }

    impl MockStore {
        fn new() -> Self {
            let settings = HashMap::from([
                ("base_url".to_string(), "https://api.openai.com/v1".to_string()),
                ("api_key".to_string(), "test-key".to_string()),
                ("model_name".to_string(), "gpt-4o".to_string()),
                ("temperature".to_string(), "0.7".to_string()),
                ("batch_size".to_string(), "5".to_string()),
                ("max_concurrent_batches".to_string(), "2".to_string()),
                ("debounce_seconds".to_string(), "10".to_string()),
                ("questions_per_screen".to_string(), "5".to_string()),
            ]);
            MockStore {
                sessions: Mutex::new(HashMap::new()),
                questions: Mutex::new(HashMap::new()),
                decision_summary: Mutex::new(HashMap::new()),
                batches: Mutex::new(HashMap::new()),
                settings: Mutex::new(settings),
                next_batch_no: Mutex::new(0),
                tickets: Mutex::new(HashMap::new()),
                rounds: Mutex::new(HashMap::new()),
            }
        }

        fn add_question(&self, q: Question) {
            self.questions.lock().unwrap().insert(q.id.clone(), q);
        }
    }

    impl Store for MockStore {
        fn create_session(&self, session: &Session) -> Result<()> {
            self.sessions.lock().unwrap().insert(session.id.clone(), session.clone());
            Ok(())
        }
        fn delete_session(&self, id: &str) -> Result<()> {
            self.sessions.lock().unwrap().remove(id);
            self.questions.lock().unwrap().retain(|_, q| q.session_id != id);
            Ok(())
        }
        fn get_sessions(&self) -> Result<Vec<Session>> {
            Ok(self.sessions.lock().unwrap().values().cloned().collect())
        }
        fn get_session(&self, id: &str) -> Result<Option<Session>> {
            Ok(self.sessions.lock().unwrap().get(id).cloned())
        }
        fn get_questions(&self, session_id: &str) -> Result<Vec<Question>> {
            let mut qs: Vec<Question> = self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id)
                .cloned()
                .collect();
            qs.sort_by_key(|q| q.display_order);
            Ok(qs)
        }
        fn insert_question(&self, q: &Question) -> Result<()> {
            self.questions.lock().unwrap().insert(q.id.clone(), q.clone());
            Ok(())
        }
        fn update_question_status(&self, id: &str, status: QuestionStatus) -> Result<()> {
            if let Some(q) = self.questions.lock().unwrap().get_mut(id) {
                q.status = status;
            }
            Ok(())
        }
        fn update_question_answer(&self, id: &str, answer: &AnswerValue, version: u32) -> Result<()> {
            if let Some(q) = self.questions.lock().unwrap().get_mut(id) {
                q.answer = Some(answer.clone());
                q.answer_version = version;
                q.status = QuestionStatus::Answered;
            }
            Ok(())
        }
        fn mark_stale(&self, question_ids: &[String]) -> Result<()> {
            let mut qs = self.questions.lock().unwrap();
            for id in question_ids {
                if let Some(q) = qs.get_mut(id) {
                    q.status = QuestionStatus::Stale;
                }
            }
            Ok(())
        }
        fn get_answered_questions(&self, session_id: &str) -> Result<Vec<Question>> {
            Ok(self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.status == QuestionStatus::Answered)
                .cloned()
                .collect())
        }
        fn get_decision_summary(&self, session_id: &str) -> Result<Vec<DecisionEntry>> {
            let mut entries: Vec<DecisionEntry> = self.decision_summary.lock().unwrap()
                .values()
                .filter(|e| {
                    // Check if this entry's question belongs to the session
                    self.questions.lock().unwrap()
                        .get(&e.question_id)
                        .map(|q| q.session_id == session_id)
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            entries.sort_by(|a, b| a.question_id.cmp(&b.question_id));
            Ok(entries)
        }
        fn upsert_decision_entry(&self, _session_id: &str, entry: &DecisionEntry) -> Result<()> {
            self.decision_summary.lock().unwrap().insert(entry.question_id.clone(), entry.clone());
            Ok(())
        }
        fn insert_batch(&self, batch: &Batch) -> Result<()> {
            self.batches.lock().unwrap().insert(batch.id.clone(), batch.clone());
            Ok(())
        }
        fn update_batch_status(&self, id: &str, status: BatchStatus) -> Result<()> {
            if let Some(b) = self.batches.lock().unwrap().get_mut(id) {
                b.status = status;
            }
            Ok(())
        }
        fn get_max_batch_no(&self, session_id: &str) -> Result<i32> {
            let max = self.batches.lock().unwrap()
                .values()
                .filter(|b| b.session_id == session_id)
                .map(|b| b.batch_no)
                .max()
                .unwrap_or(0);
            Ok(max)
        }
        fn get_max_display_order(&self, session_id: &str) -> Result<i32> {
            let max = self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id)
                .map(|q| q.display_order)
                .max()
                .unwrap_or(-1);
            Ok(max)
        }
        fn count_ready_questions(&self, session_id: &str) -> Result<i32> {
            Ok(self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.status == QuestionStatus::Ready)
                .count() as i32)
        }
        fn get_setting(&self, key: &str) -> Result<Option<String>> {
            Ok(self.settings.lock().unwrap().get(key).cloned())
        }
        fn set_setting(&self, key: &str, value: &str) -> Result<()> {
            self.settings.lock().unwrap().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn get_all_settings(&self) -> Result<Settings> {
            let s = self.settings.lock().unwrap();
            Ok(Settings {
                base_url: s.get("base_url").cloned().unwrap_or_default(),
                api_key: s.get("api_key").cloned().unwrap_or_default(),
                model_name: s.get("model_name").cloned().unwrap_or_default(),
                temperature: s.get("temperature").and_then(|v| v.parse().ok()).unwrap_or(0.7),
                batch_size: s.get("batch_size").and_then(|v| v.parse().ok()).unwrap_or(5),
                max_concurrent_batches: s.get("max_concurrent_batches").and_then(|v| v.parse().ok()).unwrap_or(2),
                debounce_seconds: s.get("debounce_seconds").and_then(|v| v.parse().ok()).unwrap_or(10),
                questions_per_screen: s.get("questions_per_screen").and_then(|v| v.parse().ok()).unwrap_or(5),
                agent_tool: s.get("agent_tool").cloned().unwrap_or_else(|| "opencode".into()),
                agent_model: s.get("agent_model").cloned().unwrap_or_default(),
                agent_effort: s.get("agent_effort").cloned().unwrap_or_else(|| "auto".into()),
                agent_auto_approve: s.get("agent_auto_approve").map(|v| v != "0" && v != "false").unwrap_or(true),
                prototype_auto_paused: s.get("prototype_auto_paused").map(|v| v == "1" || v == "true").unwrap_or(false),
            })
        }
        fn save_settings(&self, settings: &Settings) -> Result<()> {
            let mut s = self.settings.lock().unwrap();
            s.insert("base_url".to_string(), settings.base_url.clone());
            s.insert("api_key".to_string(), settings.api_key.clone());
            s.insert("model_name".to_string(), settings.model_name.clone());
            s.insert("temperature".to_string(), settings.temperature.to_string());
            s.insert("batch_size".to_string(), settings.batch_size.to_string());
            s.insert("max_concurrent_batches".to_string(), settings.max_concurrent_batches.to_string());
            s.insert("debounce_seconds".to_string(), settings.debounce_seconds.to_string());
            s.insert("questions_per_screen".to_string(), settings.questions_per_screen.to_string());
            Ok(())
        }
        fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
            if let Some(s) = self.sessions.lock().unwrap().get_mut(id) {
                s.status = status;
            }
            Ok(())
        }
        fn save_session_summary(&self, _id: &str, _summary: &str) -> Result<()> { Ok(()) }
        fn save_prototype_version_meta(&self, _id: &str, _version: i32) -> Result<()> { Ok(()) }
        fn save_prototype_version(&self, _version: &PrototypeVersion) -> Result<()> { Ok(()) }
        fn get_prototype_versions(&self, _session_id: &str) -> Result<Vec<PrototypeVersion>> { Ok(vec![]) }
        fn get_latest_prototype_snapshot(&self, _session_id: &str) -> Result<Option<PrototypeVersion>> { Ok(None) }
        fn find_dependents(&self, session_id: &str, question_id: &str) -> Result<Vec<String>> {
            let dependents: Vec<String> = self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.depends_on.contains(&question_id.to_string()))
                .map(|q| q.id.clone())
                .collect();
            Ok(dependents)
        }
        fn get_question(&self, id: &str) -> Result<Option<Question>> {
            Ok(self.questions.lock().unwrap().get(id).cloned())
        }
        fn count_answered_questions(&self, session_id: &str) -> Result<i32> {
            Ok(self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.status == QuestionStatus::Answered)
                .count() as i32)
        }
        fn count_skipped_questions(&self, session_id: &str) -> Result<i32> {
            Ok(self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.status == QuestionStatus::Skipped)
                .count() as i32)
        }
        fn insert_message(&self, _msg: &ChatMessage) -> Result<()> { Ok(()) }
        fn get_messages(&self, _session_id: &str) -> Result<Vec<ChatMessage>> { Ok(vec![]) }
        fn add_message_id_to_question(&self, _question_id: &str, _message_id: &str) -> Result<()> { Ok(()) }
        fn get_outline_nodes(&self, _session_id: &str) -> Result<Vec<OutlineNode>> { Ok(vec![]) }
        fn replace_outline_nodes(&self, _session_id: &str, _nodes: &[OutlineNode]) -> Result<()> { Ok(()) }
        fn set_session_outline_status(&self, _session_id: &str, _status: OutlineStatus) -> Result<()> { Ok(()) }
        fn skip_questions_for_nodes(&self, _session_id: &str, _node_ids: &[String]) -> Result<()> { Ok(()) }
        fn get_skipped_questions(&self, session_id: &str) -> Result<Vec<Question>> {
            Ok(self.questions.lock().unwrap()
                .values()
                .filter(|q| q.session_id == session_id && q.status == QuestionStatus::Skipped)
                .cloned()
                .collect())
        }
        fn set_session_pipeline_stage(&self, session_id: &str, stage: PipelineStage) -> Result<()> {
            if let Some(s) = self.sessions.lock().unwrap().get_mut(session_id) {
                s.pipeline_stage = stage;
            }
            Ok(())
        }
        fn set_session_spec(&self, session_id: &str, spec: Option<&str>) -> Result<()> {
            if let Some(s) = self.sessions.lock().unwrap().get_mut(session_id) {
                s.spec = spec.map(|x| x.to_string());
            }
            Ok(())
        }
        fn get_tickets(&self, session_id: &str) -> Result<Vec<Ticket>> {
            let mut ts: Vec<Ticket> = self.tickets.lock().unwrap()
                .values()
                .filter(|t| t.session_id == session_id)
                .cloned()
                .collect();
            ts.sort_by_key(|t| t.display_order);
            Ok(ts)
        }
        fn get_ticket(&self, ticket_id: &str) -> Result<Option<Ticket>> {
            Ok(self.tickets.lock().unwrap().get(ticket_id).cloned())
        }
        fn replace_tickets(&self, session_id: &str, tickets: &[Ticket]) -> Result<()> {
            let mut map = self.tickets.lock().unwrap();
            map.retain(|_, t| t.session_id != session_id);
            for t in tickets {
                map.insert(t.id.clone(), t.clone());
            }
            Ok(())
        }
        fn set_ticket_status(&self, ticket_id: &str, status: TicketStatus) -> Result<()> {
            if let Some(t) = self.tickets.lock().unwrap().get_mut(ticket_id) {
                t.status = status;
            }
            Ok(())
        }
        fn add_ticket_branch(&self, ticket_id: &str, branch: &str) -> Result<()> {
            if let Some(t) = self.tickets.lock().unwrap().get_mut(ticket_id) {
                if !t.branches.iter().any(|b| b == branch) {
                    t.branches.push(branch.to_string());
                }
            }
            Ok(())
        }

        fn create_round(&self, round: &Round) -> Result<()> {
            self.rounds
                .lock()
                .unwrap()
                .insert(round.id.clone(), round.clone());
            Ok(())
        }

        fn update_round(&self, round: &Round) -> Result<()> {
            self.rounds
                .lock()
                .unwrap()
                .insert(round.id.clone(), round.clone());
            Ok(())
        }

        fn get_rounds(&self, session_id: &str) -> Result<Vec<Round>> {
            Ok(self
                .rounds
                .lock()
                .unwrap()
                .values()
                .filter(|r| r.session_id == session_id)
                .cloned()
                .collect())
        }

        fn get_round(&self, round_id: &str) -> Result<Option<Round>> {
            Ok(self.rounds.lock().unwrap().get(round_id).cloned())
        }

        fn get_current_round(&self, session_id: &str) -> Result<Option<Round>> {
            let cur = self
                .sessions
                .lock()
                .unwrap()
                .get(session_id)
                .and_then(|s| s.current_round_id.clone());
            Ok(cur.and_then(|rid| self.rounds.lock().unwrap().get(&rid).cloned()))
        }

        fn set_current_round(&self, session_id: &str, round_id: Option<&str>) -> Result<()> {
            if let Some(s) = self.sessions.lock().unwrap().get_mut(session_id) {
                s.current_round_id = round_id.map(|r| r.to_string());
            }
            Ok(())
        }

        fn get_round_archive(&self, round_id: &str) -> Result<RoundArchive> {
            let round = self
                .rounds
                .lock()
                .unwrap()
                .get(round_id)
                .cloned()
                .ok_or_else(|| StoreError::NotFound(round_id.to_string()))?;
            Ok(RoundArchive {
                round,
                nodes: vec![],
                questions: vec![],
                tickets: vec![],
                messages: vec![],
                decisions: vec![],
            })
        }
    }

    // ============ Stale marking / dependency graph tests ============

    fn make_question(id: &str, session_id: &str, depends_on: Vec<String>, order: i32) -> Question {
        Question {
            id: id.to_string(),
            session_id: session_id.to_string(),
            batch_id: "b1".to_string(),
            q_type: QuestionType::Choice,
            category: QuestionCategory::Intent,
            question: format!("Question {}", id),
            context: None,
            options: vec![QuestionOption { label: "A".to_string(), description: None }],
            recommended_option: None,
            rationale: None,
            depends_on,
            status: QuestionStatus::Ready,
            answer: None,
            answer_version: 0,
            display_order: order,
            message_id: None,
            outline_node_id: None,
        }
    }

    #[test]
    fn test_stale_marking_direct_dependency() {
        let store = MockStore::new();
        let session_id = "s1";
        // Q1 <- Q2 (Q2 depends on Q1)
        store.add_question(make_question("q1", session_id, vec![], 0));
        store.add_question(make_question("q2", session_id, vec!["q1".to_string()], 1));

        // When Q1 is answered, Q2 should be found as dependent
        let dependents = store.find_dependents(session_id, "q1").unwrap();
        assert_eq!(dependents, vec!["q2"]);

        // Mark Q2 as stale
        store.mark_stale(&dependents).unwrap();
        let q2 = store.get_question("q2").unwrap().unwrap();
        assert_eq!(q2.status, QuestionStatus::Stale);
    }

    #[test]
    fn test_stale_marking_transitive_dependency() {
        let store = MockStore::new();
        let session_id = "s1";
        // Q1 <- Q2 <- Q3 (chain)
        store.add_question(make_question("q1", session_id, vec![], 0));
        store.add_question(make_question("q2", session_id, vec!["q1".to_string()], 1));
        store.add_question(make_question("q3", session_id, vec!["q2".to_string()], 2));

        // Find direct dependents of Q1
        let dep1 = store.find_dependents(session_id, "q1").unwrap();
        assert_eq!(dep1, vec!["q2"]);

        // Find transitive: dependents of Q2
        let dep2 = store.find_dependents(session_id, "q2").unwrap();
        assert_eq!(dep2, vec!["q3"]);

        // Full transitive closure: Q1 -> Q2 -> Q3
        let mut all_stale = vec![];
        let mut to_check = vec!["q1".to_string()];
        while let Some(qid) = to_check.pop() {
            let deps = store.find_dependents(session_id, &qid).unwrap();
            for d in deps {
                if !all_stale.contains(&d) {
                    all_stale.push(d.clone());
                    to_check.push(d);
                }
            }
        }
        assert_eq!(all_stale.len(), 2);
        assert!(all_stale.contains(&"q2".to_string()));
        assert!(all_stale.contains(&"q3".to_string()));
    }

    #[test]
    fn test_stale_marking_no_dependents() {
        let store = MockStore::new();
        let session_id = "s1";
        store.add_question(make_question("q1", session_id, vec![], 0));
        store.add_question(make_question("q2", session_id, vec![], 1)); // independent

        let dependents = store.find_dependents(session_id, "q1").unwrap();
        assert!(dependents.is_empty());
    }

    // ============ Water level calculation tests ============

    #[test]
    fn test_water_level_low_trigger() {
        let batch_size = 5;
        let low_water = batch_size / 2; // 2
        let high_water = 2 * batch_size; // 10

        // Inventory = 2, should trigger (<= low_water)
        let inventory = 2;
        assert!(inventory <= low_water, "should trigger at low water");

        // Inventory = 3, should NOT trigger (> low_water)
        let inventory = 3;
        assert!(!(inventory <= low_water), "should not trigger above low water");

        // Inventory = 10, should stop (>= high_water)
        let inventory = 10;
        assert!(inventory >= high_water, "should stop at high water");

        // Inventory = 9, should not stop (< high_water)
        let inventory = 9;
        assert!(!(inventory >= high_water), "should not stop below high water");
    }

    // ============ Export format tests ============

    #[test]
    fn test_export_markdown_format() {
        let store = MockStore::new();
        let session_id = "s1";

        // Create session
        let session = Session {
            id: session_id.to_string(),
            title: "测试项目".to_string(),
            initial_context: None,
            role: "pm".to_string(),
            status: SessionStatus::Completed,
            summary: None,
            prototype_version: 0,
            outline_status: OutlineStatus::None,
            pipeline_stage: PipelineStage::None,
            spec: None,
            current_round_id: None,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        };
        store.create_session(&session).unwrap();

        // Add a decision entry
        let entry = DecisionEntry {
            question_id: "q1".to_string(),
            question: "用什么框架？".to_string(),
            answer: "Tauri".to_string(),
            rationale: Some("跨平台好".to_string()),
            category: QuestionCategory::Intent,
        };
        store.upsert_decision_entry(session_id, &entry).unwrap();

        // Add the question to the store (needed for session lookup)
        let mut q = make_question("q1", session_id, vec![], 0);
        q.status = QuestionStatus::Answered;
        q.answer = Some(AnswerValue::Choice { option: "Tauri".to_string() });
        store.add_question(q);

        let md = grill_me_v2_lib::export::export_session(
            &store,
            session_id,
            &grill_me_v2_lib::export::ExportFormat::Markdown,
            None,
        ).unwrap();

        assert!(md.contains("# Grill 会话：测试项目"));
        assert!(md.contains("**日期：** 2026-07-01"));
        assert!(md.contains("用什么框架？"));
        assert!(md.contains("**决策：** Tauri"));
        assert!(md.contains("**原因：** 跨平台好"));
    }

    #[test]
    fn test_export_json_format() {
        let store = MockStore::new();
        let session_id = "s1";

        let session = Session {
            id: session_id.to_string(),
            title: "测试".to_string(),
            initial_context: None,
            role: "pm".to_string(),
            status: SessionStatus::Completed,
            summary: None,
            prototype_version: 0,
            outline_status: OutlineStatus::None,
            pipeline_stage: PipelineStage::None,
            spec: None,
            current_round_id: None,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        };
        store.create_session(&session).unwrap();

        let entry = DecisionEntry {
            question_id: "q1".to_string(),
            question: "问题？".to_string(),
            answer: "答案".to_string(),
            rationale: None,
            category: QuestionCategory::Choice,
        };
        store.upsert_decision_entry(session_id, &entry).unwrap();
        store.add_question(make_question("q1", session_id, vec![], 0));

        let json = grill_me_v2_lib::export::export_session(
            &store,
            session_id,
            &grill_me_v2_lib::export::ExportFormat::Json,
            None,
        ).unwrap();

        // Should be valid JSON array
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 1);
        assert_eq!(parsed[0]["question"], "问题？");
        assert_eq!(parsed[0]["answer"], "答案");
    }

    #[test]
    fn test_export_markdown_with_llm_summary() {
        let store = MockStore::new();
        let session_id = "s1";

        let session = Session {
            id: session_id.to_string(),
            title: "测试".to_string(),
            initial_context: None,
            role: "pm".to_string(),
            status: SessionStatus::Completed,
            summary: None,
            prototype_version: 0,
            outline_status: OutlineStatus::None,
            pipeline_stage: PipelineStage::None,
            spec: None,
            current_round_id: None,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        };
        store.create_session(&session).unwrap();
        store.add_question(make_question("q1", session_id, vec![], 0));

        let md = grill_me_v2_lib::export::export_session(
            &store,
            session_id,
            &grill_me_v2_lib::export::ExportFormat::Markdown,
            Some("这是一个很好的需求，建议先做 MVP。"),
        ).unwrap();

        assert!(md.contains("## LLM 总结"));
        assert!(md.contains("这是一个很好的需求，建议先做 MVP。"));
    }

    #[test]
    fn test_export_markdown_with_skipped_questions() {
        let store = MockStore::new();
        let session_id = "s1";

        let session = Session {
            id: session_id.to_string(),
            title: "测试".to_string(),
            initial_context: None,
            role: "pm".to_string(),
            status: SessionStatus::Completed,
            summary: None,
            prototype_version: 0,
            outline_status: OutlineStatus::None,
            pipeline_stage: PipelineStage::None,
            spec: None,
            current_round_id: None,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        };
        store.create_session(&session).unwrap();

        // Add a skipped question
        let mut q = make_question("q_skip", session_id, vec![], 0);
        q.status = QuestionStatus::Skipped;
        q.question = "跳过的问题？".to_string();
        store.add_question(q);

        let md = grill_me_v2_lib::export::export_session(
            &store,
            session_id,
            &grill_me_v2_lib::export::ExportFormat::Markdown,
            None,
        ).unwrap();

        assert!(md.contains("## 跳过的问题"));
        assert!(md.contains("跳过的问题？"));
        assert!(md.contains("用户跳过"));
    }

    // ============ Decision summary incremental test ============

    #[test]
    fn test_decision_summary_incremental() {
        let store = MockStore::new();
        let session_id = "s1";

        // Answer Q1 → add to decision summary
        let entry1 = DecisionEntry {
            question_id: "q1".to_string(),
            question: "框架？".to_string(),
            answer: "Tauri".to_string(),
            rationale: Some("好".to_string()),
            category: QuestionCategory::Intent,
        };
        store.upsert_decision_entry(session_id, &entry1).unwrap();
        store.add_question(make_question("q1", session_id, vec![], 0));

        let summary = store.get_decision_summary(session_id).unwrap();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].answer, "Tauri");

        // Answer Q2 → add to decision summary
        let entry2 = DecisionEntry {
            question_id: "q2".to_string(),
            question: "语言？".to_string(),
            answer: "Rust".to_string(),
            rationale: None,
            category: QuestionCategory::Choice,
        };
        store.upsert_decision_entry(session_id, &entry2).unwrap();
        store.add_question(make_question("q2", session_id, vec![], 1));

        let summary = store.get_decision_summary(session_id).unwrap();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].answer, "Tauri");
        assert_eq!(summary[1].answer, "Rust");

        // Re-answer Q1 → upsert (replace, not duplicate)
        let entry1_updated = DecisionEntry {
            question_id: "q1".to_string(),
            question: "框架？".to_string(),
            answer: "Electron".to_string(), // changed answer
            rationale: Some("改主意了".to_string()),
            category: QuestionCategory::Intent,
        };
        store.upsert_decision_entry(session_id, &entry1_updated).unwrap();

        let summary = store.get_decision_summary(session_id).unwrap();
        assert_eq!(summary.len(), 2); // still 2, not 3
        assert_eq!(summary[0].answer, "Electron"); // updated
    }
}

// ============ Pipeline: tickets tests ============

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    #[test]
    fn parse_tickets_response_in_tag() {
        let text = r#"一些前缀说明
<tickets>{"tickets":[{"key":"t1","title":"地图数据模型","description":"实现地图配置结构","depends_on":[]},{"key":"t2","title":"地图渲染","description":"按数据渲染地图","depends_on":["t1"]}]}</tickets>
"#;
        let ts = grill_me_v2_lib::llm::prompt::parse_tickets_response(text);
        assert_eq!(ts.len(), 2);
        assert_eq!(ts[0].title.as_deref(), Some("地图数据模型"));
        assert_eq!(ts[1].depends_on.as_deref(), Some(&["t1".to_string()][..]));
    }

    #[test]
    fn parse_tickets_response_raw_json_fallback() {
        let text = r#"{"tickets":[{"key":"a","title":"X","depends_on":null}]}"#;
        let ts = grill_me_v2_lib::llm::prompt::parse_tickets_response(text);
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].key.as_deref(), Some("a"));
    }

    #[test]
    fn parse_spec_response_extracts_tag() {
        let text = "前面的话\n<spec>\n# 标题\n内容\n</spec>\n后面的话";
        let spec = grill_me_v2_lib::llm::prompt::parse_spec_response(text);
        assert_eq!(spec, "# 标题\n内容");
    }
}
