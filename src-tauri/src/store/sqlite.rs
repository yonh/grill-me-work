use crate::model::*;
use crate::store::{Result, Store};
use rusqlite::{params, Connection};
use std::sync::Mutex;

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(db_path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Self::run_migrations(&conn)?;
        Ok(SqliteStore {
            conn: Mutex::new(conn),
        })
    }

    fn run_migrations(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                initial_context TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS questions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                batch_id TEXT NOT NULL,
                q_type TEXT NOT NULL,
                category TEXT NOT NULL,
                question TEXT NOT NULL,
                context TEXT,
                options_json TEXT NOT NULL,
                recommended_option TEXT,
                rationale TEXT,
                depends_on_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'generating',
                answer_json TEXT,
                answer_version INTEGER NOT NULL DEFAULT 0,
                display_order INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE TABLE IF NOT EXISTS batches (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                batch_no INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'generating',
                triggered_by_answers_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS decision_summary (
                question_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                question TEXT NOT NULL,
                answer TEXT NOT NULL,
                rationale TEXT,
                category TEXT NOT NULL,
                display_order INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_questions_session ON questions(session_id);
            CREATE INDEX IF NOT EXISTS idx_batches_session ON batches(session_id);
            CREATE INDEX IF NOT EXISTS idx_decision_session ON decision_summary(session_id);

            CREATE TABLE IF NOT EXISTS prototype_versions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                snapshot_json TEXT NOT NULL,
                feedback TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_prototype_versions_session ON prototype_versions(session_id);

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                question_ids TEXT,
                tool_calls TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            "#,
        )?;

        // Migration: add message_id column to questions if not exists
        let has_message_id: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(questions)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let mut found = false;
            for row in rows {
                if row? == "message_id" {
                    found = true;
                }
            }
            found
        };
        if !has_message_id {
            conn.execute("ALTER TABLE questions ADD COLUMN message_id TEXT", [])?;
            log::info!("[db] Added message_id column to questions table");
        }

        // Migration: add role column to sessions if not exists
        let has_role: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let mut found = false;
            for row in rows {
                if row? == "role" {
                    found = true;
                }
            }
            found
        };
        if !has_role {
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN role TEXT NOT NULL DEFAULT 'pm';",
            )?;
        }

        // Migration: add summary column to sessions if not exists
        let has_summary: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let mut found = false;
            for row in rows {
                if row? == "summary" {
                    found = true;
                }
            }
            found
        };
        if !has_summary {
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN summary TEXT;",
            )?;
        }

        // Migration: prototype_version column (html blob removed in v2 — files live on disk)
        let has_prototype_version: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let mut found = false;
            for row in rows {
                if row? == "prototype_version" {
                    found = true;
                }
            }
            found
        };
        if !has_prototype_version {
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN prototype_version INTEGER NOT NULL DEFAULT 0;",
            )?;
        }

        Ok(())
    }
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        title: row.get("title")?,
        initial_context: row.get("initial_context")?,
        role: row.get("role")?,
        status: SessionStatus::from_str(row.get::<_, String>("status")?.as_str()),
        summary: row.get("summary")?,
        prototype_version: row.get("prototype_version")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_question(row: &rusqlite::Row) -> rusqlite::Result<Question> {
    let options_json: String = row.get("options_json")?;
    let options: Vec<QuestionOption> =
        serde_json::from_str(&options_json).unwrap_or_default();
    let depends_on_json: String = row.get("depends_on_json")?;
    let depends_on: Vec<String> =
        serde_json::from_str(&depends_on_json).unwrap_or_default();
    let answer_json: Option<String> = row.get("answer_json")?;
    let answer: Option<AnswerValue> = answer_json
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    Ok(Question {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        batch_id: row.get("batch_id")?,
        q_type: QuestionType::from_str(row.get::<_, String>("q_type")?.as_str()),
        category: QuestionCategory::from_str(row.get::<_, String>("category")?.as_str()),
        question: row.get("question")?,
        context: row.get("context")?,
        options,
        recommended_option: row.get("recommended_option")?,
        rationale: row.get("rationale")?,
        depends_on,
        status: QuestionStatus::from_str(row.get::<_, String>("status")?.as_str()),
        answer,
        answer_version: row.get("answer_version")?,
        display_order: row.get("display_order")?,
        message_id: row.get("message_id")?,
    })
}

impl Store for SqliteStore {
    fn create_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, title, initial_context, role, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id,
                session.title,
                session.initial_context,
                session.role,
                session.status.as_str(),
                session.created_at,
                session.updated_at,
            ],
        )?;
        Ok(())
    }

    fn delete_session(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM decision_summary WHERE session_id = ?1", params![id])?;
        conn.execute("DELETE FROM questions WHERE session_id = ?1", params![id])?;
        conn.execute("DELETE FROM batches WHERE session_id = ?1", params![id])?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn get_sessions(&self) -> Result<Vec<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, title, initial_context, role, status, summary, prototype_version, created_at, updated_at FROM sessions ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], row_to_session)?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, initial_context, role, status, summary, prototype_version, created_at, updated_at FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_session)?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    fn get_questions(&self, session_id: &str) -> Result<Vec<Question>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, batch_id, q_type, category, question, context, options_json, recommended_option, rationale, depends_on_json, status, answer_json, answer_version, display_order, message_id FROM questions WHERE session_id = ?1 ORDER BY display_order ASC",
        )?;
        let rows = stmt.query_map(params![session_id], row_to_question)?;
        let mut questions = Vec::new();
        for row in rows {
            questions.push(row?);
        }
        Ok(questions)
    }

    fn insert_question(&self, q: &Question) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let options_json = serde_json::to_string(&q.options)?;
        let depends_on_json = serde_json::to_string(&q.depends_on)?;
        let answer_json = match &q.answer {
            Some(a) => Some(serde_json::to_string(a)?),
            None => None,
        };
        conn.execute(
            "INSERT OR REPLACE INTO questions (id, session_id, batch_id, q_type, category, question, context, options_json, recommended_option, rationale, depends_on_json, status, answer_json, answer_version, display_order, message_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                q.id,
                q.session_id,
                q.batch_id,
                q.q_type.as_str(),
                q.category.as_str(),
                q.question,
                q.context,
                options_json,
                q.recommended_option,
                q.rationale,
                depends_on_json,
                q.status.as_str(),
                answer_json,
                q.answer_version,
                q.display_order,
                q.message_id,
            ],
        )?;
        Ok(())
    }

    fn update_question_status(&self, id: &str, status: QuestionStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE questions SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )?;
        Ok(())
    }

    fn update_question_answer(
        &self,
        id: &str,
        answer: &AnswerValue,
        version: u32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let answer_json = serde_json::to_string(answer)?;
        conn.execute(
            "UPDATE questions SET answer_json = ?1, answer_version = ?2, status = 'answered' WHERE id = ?3",
            params![answer_json, version, id],
        )?;
        Ok(())
    }

    fn mark_stale(&self, question_ids: &[String]) -> Result<()> {
        if question_ids.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        for id in question_ids {
            conn.execute(
                "UPDATE questions SET status = 'stale' WHERE id = ?1 AND status IN ('ready', 'generating')",
                params![id],
            )?;
        }
        Ok(())
    }

    fn get_answered_questions(&self, session_id: &str) -> Result<Vec<Question>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, batch_id, q_type, category, question, context, options_json, recommended_option, rationale, depends_on_json, status, answer_json, answer_version, display_order, message_id FROM questions WHERE session_id = ?1 AND status = 'answered' ORDER BY display_order ASC",
        )?;
        let rows = stmt.query_map(params![session_id], row_to_question)?;
        let mut questions = Vec::new();
        for row in rows {
            questions.push(row?);
        }
        Ok(questions)
    }

    fn get_decision_summary(&self, session_id: &str) -> Result<Vec<DecisionEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT question_id, question, answer, rationale, category FROM decision_summary WHERE session_id = ?1 ORDER BY display_order ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(DecisionEntry {
                question_id: row.get("question_id")?,
                question: row.get("question")?,
                answer: row.get("answer")?,
                rationale: row.get("rationale")?,
                category: QuestionCategory::from_str(
                    row.get::<_, String>("category")?.as_str(),
                ),
            })
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    fn upsert_decision_entry(&self, session_id: &str, entry: &DecisionEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Get current display_order or compute max
        let display_order: i32 = {
            let existing: Option<i32> = conn
                .query_row(
                    "SELECT display_order FROM decision_summary WHERE question_id = ?1",
                    params![entry.question_id],
                    |row| row.get(0),
                )
                .ok();
            if let Some(d) = existing {
                d
            } else {
                let max: i32 = conn
                    .query_row(
                        "SELECT COALESCE(MAX(display_order), 0) + 1 FROM decision_summary WHERE session_id = ?1",
                        params![session_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                max
            }
        };
        conn.execute(
            "INSERT OR REPLACE INTO decision_summary (question_id, session_id, question, answer, rationale, category, display_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.question_id,
                session_id,
                entry.question,
                entry.answer,
                entry.rationale,
                entry.category.as_str(),
                display_order,
            ],
        )?;
        Ok(())
    }

    fn insert_batch(&self, batch: &Batch) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let triggered_json = serde_json::to_string(&batch.triggered_by_answers)?;
        conn.execute(
            "INSERT INTO batches (id, session_id, batch_no, status, triggered_by_answers_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                batch.id,
                batch.session_id,
                batch.batch_no,
                batch.status.as_str(),
                triggered_json,
                batch.created_at,
            ],
        )?;
        Ok(())
    }

    fn update_batch_status(&self, id: &str, status: BatchStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE batches SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )?;
        Ok(())
    }

    fn get_max_batch_no(&self, session_id: &str) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let max: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(batch_no), 0) FROM batches WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(max)
    }

    fn get_max_display_order(&self, session_id: &str) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let max: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(display_order), 0) FROM questions WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(max)
    }

    fn count_ready_questions(&self, session_id: &str) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM questions WHERE session_id = ?1 AND status = 'ready'",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: rusqlite::Result<Option<String>> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            });
        Ok(result?)
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    fn get_all_settings(&self) -> Result<Settings> {
        let mut settings = Settings::default();
        if let Some(v) = self.get_setting("base_url")? {
            settings.base_url = v;
        }
        if let Some(v) = self.get_setting("api_key")? {
            settings.api_key = v;
        }
        if let Some(v) = self.get_setting("model_name")? {
            settings.model_name = v;
        }
        if let Some(v) = self.get_setting("temperature")? {
            settings.temperature = v.parse().unwrap_or(0.7);
        }
        if let Some(v) = self.get_setting("batch_size")? {
            settings.batch_size = v.parse().unwrap_or(5);
        }
        if let Some(v) = self.get_setting("max_concurrent_batches")? {
            settings.max_concurrent_batches = v.parse().unwrap_or(2);
        }
        if let Some(v) = self.get_setting("debounce_seconds")? {
            settings.debounce_seconds = v.parse().unwrap_or(10);
        }
        if let Some(v) = self.get_setting("questions_per_screen")? {
            settings.questions_per_screen = v.parse().unwrap_or(5);
        }
        if let Some(v) = self.get_setting("agent_tool")? {
            settings.agent_tool = v;
        }
        if let Some(v) = self.get_setting("agent_model")? {
            settings.agent_model = v;
        }
        if let Some(v) = self.get_setting("agent_effort")? {
            settings.agent_effort = v;
        }
        if let Some(v) = self.get_setting("agent_auto_approve")? {
            settings.agent_auto_approve = v != "0" && v != "false";
        }
        if let Some(v) = self.get_setting("prototype_auto_paused")? {
            settings.prototype_auto_paused = v == "1" || v == "true";
        }
        Ok(settings)
    }

    fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.set_setting("base_url", &settings.base_url)?;
        self.set_setting("api_key", &settings.api_key)?;
        self.set_setting("model_name", &settings.model_name)?;
        self.set_setting("temperature", &settings.temperature.to_string())?;
        self.set_setting("batch_size", &settings.batch_size.to_string())?;
        self.set_setting(
            "max_concurrent_batches",
            &settings.max_concurrent_batches.to_string(),
        )?;
        self.set_setting("debounce_seconds", &settings.debounce_seconds.to_string())?;
        self.set_setting(
            "questions_per_screen",
            &settings.questions_per_screen.to_string(),
        )?;
        self.set_setting("agent_tool", &settings.agent_tool)?;
        self.set_setting("agent_model", &settings.agent_model)?;
        self.set_setting("agent_effort", &settings.agent_effort)?;
        self.set_setting(
            "agent_auto_approve",
            if settings.agent_auto_approve { "1" } else { "0" },
        )?;
        self.set_setting(
            "prototype_auto_paused",
            if settings.prototype_auto_paused { "1" } else { "0" },
        )?;
        Ok(())
    }

    fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id],
        )?;
        Ok(())
    }

    fn save_session_summary(&self, id: &str, summary: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            params![summary, now, id],
        )?;
        Ok(())
    }

    fn save_prototype_version_meta(&self, id: &str, version: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET prototype_version = ?1, updated_at = ?2 WHERE id = ?3",
            params![version, now, id],
        )?;
        Ok(())
    }

    fn save_prototype_version(&self, version: &PrototypeVersion) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO prototype_versions (id, session_id, version, snapshot_json, feedback, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![version.id, version.session_id, version.version, version.snapshot_json, version.feedback, version.created_at],
        )?;
        Ok(())
    }

    fn get_prototype_versions(&self, session_id: &str) -> Result<Vec<PrototypeVersion>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, version, snapshot_json, feedback, created_at FROM prototype_versions WHERE session_id = ?1 ORDER BY version DESC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(PrototypeVersion {
                id: row.get(0)?,
                session_id: row.get(1)?,
                version: row.get(2)?,
                snapshot_json: row.get(3)?,
                feedback: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row?);
        }
        Ok(versions)
    }

    fn get_latest_prototype_snapshot(&self, session_id: &str) -> Result<Option<PrototypeVersion>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, version, snapshot_json, feedback, created_at FROM prototype_versions WHERE session_id = ?1 ORDER BY version DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![session_id], |row| {
            Ok(PrototypeVersion {
                id: row.get(0)?,
                session_id: row.get(1)?,
                version: row.get(2)?,
                snapshot_json: row.get(3)?,
                feedback: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    fn find_dependents(&self, session_id: &str, question_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, depends_on_json FROM questions WHERE session_id = ?1 AND status IN ('ready', 'generating', 'stale')",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let depends_on_json: String = row.get(1)?;
            Ok((id, depends_on_json))
        })?;
        let mut dependents = Vec::new();
        for row in rows {
            let (id, depends_on_json) = row?;
            let depends_on: Vec<String> =
                serde_json::from_str(&depends_on_json).unwrap_or_default();
            if depends_on.contains(&question_id.to_string()) {
                dependents.push(id);
            }
        }
        Ok(dependents)
    }

    fn get_question(&self, id: &str) -> Result<Option<Question>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, batch_id, q_type, category, question, context, options_json, recommended_option, rationale, depends_on_json, status, answer_json, answer_version, display_order, message_id FROM questions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_question)?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    fn count_answered_questions(&self, session_id: &str) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM questions WHERE session_id = ?1 AND status IN ('answered', 'skipped')",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    fn count_skipped_questions(&self, session_id: &str) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM questions WHERE session_id = ?1 AND status = 'skipped'",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    fn insert_message(&self, msg: &ChatMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let question_ids_json = serde_json::to_string(&msg.question_ids)?;
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, question_ids, tool_calls, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![msg.id, msg.session_id, msg.role, msg.content, question_ids_json, None::<String>, msg.created_at],
        )?;
        Ok(())
    }

    fn get_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, question_ids, created_at FROM messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let question_ids_json: String = row.get(4).unwrap_or_default();
            let question_ids: Vec<String> = serde_json::from_str(&question_ids_json).unwrap_or_default();
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                question_ids,
                created_at: row.get(5)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn add_message_id_to_question(&self, question_id: &str, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE questions SET message_id = ?1 WHERE id = ?2",
            params![message_id, question_id],
        )?;
        Ok(())
    }
}
