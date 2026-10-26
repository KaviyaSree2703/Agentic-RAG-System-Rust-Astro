use chrono::{DateTime, Utc};
use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::models::{Citation, Conversation, DocumentChunk, DocumentMetadata, Message};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        db.run_migrations()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL,
                description TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                file_format TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                total_chunks INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                scope TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'ready'
            );

            CREATE TABLE IF NOT EXISTS document_chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                page_number INTEGER,
                section TEXT,
                created_at TEXT NOT NULL,
                scope TEXT NOT NULL,
                FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                user_id TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sources_json TEXT,
                sufficiency TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS run_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS ingestion_jobs (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                file_path TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'queued',
                error_message TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_doc ON document_chunks(document_id);
            CREATE INDEX IF NOT EXISTS idx_msgs_conv ON messages(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_run_events_run ON run_events(run_id);
            CREATE INDEX IF NOT EXISTS idx_ingestion_jobs_status ON ingestion_jobs(status);
            "#,
        )?;
        Ok(())
    }

    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if documents table has status column
        let mut has_status = false;
        {
            let mut stmt = conn.prepare("PRAGMA table_info(documents)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            for col in rows.flatten() {
                if col.eq_ignore_ascii_case("status") {
                    has_status = true;
                    break;
                }
            }
        }

        if !has_status {
            conn.execute(
                "ALTER TABLE documents ADD COLUMN status TEXT NOT NULL DEFAULT 'ready'",
                [],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO schema_migrations (version, applied_at, description)
                 VALUES (1, ?1, 'Add status column to documents table')",
                params![Utc::now().to_rfc3339()],
            )?;
        }

        // Check if messages table has sufficiency column
        let mut has_sufficiency = false;
        {
            let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            for col in rows.flatten() {
                if col.eq_ignore_ascii_case("sufficiency") {
                    has_sufficiency = true;
                    break;
                }
            }
        }

        if !has_sufficiency {
            conn.execute("ALTER TABLE messages ADD COLUMN sufficiency TEXT", [])?;
            conn.execute(
                "INSERT OR IGNORE INTO schema_migrations (version, applied_at, description)
                 VALUES (2, ?1, 'Add sufficiency column to messages table')",
                params![Utc::now().to_rfc3339()],
            )?;
        }

        Ok(())
    }

    // Document operations
    pub fn save_document(&self, doc: &DocumentMetadata) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO documents (id, filename, file_format, file_size, total_chunks, created_at, scope, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
             total_chunks = excluded.total_chunks,
             file_size = excluded.file_size,
             status = excluded.status",
            params![
                doc.id,
                doc.filename,
                doc.file_format,
                doc.file_size as i64,
                doc.total_chunks as i64,
                doc.created_at.to_rfc3339(),
                doc.scope,
                doc.status.as_deref().unwrap_or("ready")
            ],
        )?;
        Ok(())
    }

    pub fn list_documents(&self) -> Result<Vec<DocumentMetadata>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, file_format, file_size, total_chunks, created_at, scope, status
             FROM documents ORDER BY created_at DESC",
        )?;

        let doc_iter = stmt.query_map([], |row| {
            let created_at_str: String = row.get(5)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(DocumentMetadata {
                id: row.get(0)?,
                filename: row.get(1)?,
                file_format: row.get(2)?,
                file_size: row.get::<_, i64>(3)? as usize,
                total_chunks: row.get::<_, i64>(4)? as usize,
                created_at,
                scope: row.get(6)?,
                status: row.get(7)?,
            })
        })?;

        let mut docs = Vec::new();
        for doc in doc_iter {
            docs.push(doc?);
        }
        Ok(docs)
    }

    pub fn delete_document(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM document_chunks WHERE document_id = ?1",
            params![id],
        )?;
        let affected = conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // Chunk operations
    pub fn save_chunks(&self, chunks: &[DocumentChunk]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO document_chunks (id, document_id, chunk_index, content, page_number, section, created_at, scope)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;

            for chunk in chunks {
                let page_number = chunk.page_number.map(|p| p as i64);
                stmt.execute(params![
                    chunk.id,
                    chunk.document_id,
                    chunk.chunk_index as i64,
                    chunk.content,
                    page_number,
                    chunk.section,
                    chunk.created_at.to_rfc3339(),
                    chunk.scope
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // Conversation operations
    pub fn get_or_create_conversation(&self, id: &str, title: &str) -> Result<Conversation> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, user_id)
             VALUES (?1, ?2, ?3, ?4, 'default_user')
             ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at",
            params![id, title, now, now],
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, user_id FROM conversations WHERE id = ?1",
        )?;
        let conv = stmt.query_row(params![id], |row| {
            let created_at_str: String = row.get(2)?;
            let updated_at_str: String = row.get(3)?;
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                    .unwrap()
                    .with_timezone(&Utc),
                user_id: row.get(4)?,
            })
        })?;
        Ok(conv)
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, created_at, updated_at, user_id FROM conversations ORDER BY updated_at DESC")?;

        let convs = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(2)?;
                let updated_at_str: String = row.get(3)?;
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                    user_id: row.get(4)?,
                })
            })?
            .filter_map(|res| res.ok())
            .collect();

        Ok(convs)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![id],
        )?;
        let affected = conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn save_message(&self, msg: &Message) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let sources_json = msg
            .sources
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, sources_json, sufficiency, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                msg.id,
                msg.conversation_id,
                msg.role,
                msg.content,
                sources_json,
                msg.sufficiency,
                msg.created_at.to_rfc3339()
            ],
        )?;

        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![msg.created_at.to_rfc3339(), msg.conversation_id],
        )?;

        Ok(())
    }

    pub fn get_conversation_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, role, content, sources_json, sufficiency, created_at
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;

        let msgs = stmt
            .query_map(params![conversation_id], |row| {
                let sources_json: Option<String> = row.get(4)?;
                let sources: Option<Vec<Citation>> =
                    sources_json.and_then(|j| serde_json::from_str(&j).ok());
                let created_at_str: String = row.get(6)?;

                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    sources,
                    sufficiency: row.get(5)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap_or_default()
                        .with_timezone(&Utc),
                })
            })?
            .filter_map(|res| res.ok())
            .collect();

        Ok(msgs)
    }

    // v11 Persistent Run Events (Section 10.3 & 12.3)
    pub fn save_run_event(&self, run_id: &str, event_type: &str, payload_json: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO run_events (run_id, event_type, payload_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![run_id, event_type, payload_json, now],
        )?;
        let last_id = conn.last_insert_rowid() as u64;
        Ok(last_id)
    }

    pub fn get_run_events(&self, run_id: &str, after_id: Option<u64>) -> Result<Vec<(u64, String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let min_id = after_id.unwrap_or(0) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, event_type, payload_json, created_at FROM run_events
             WHERE run_id = ?1 AND id > ?2 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![run_id, min_id], |row| {
            let id_i64: i64 = row.get(0)?;
            Ok((id_i64 as u64, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    }

    // v11 Durable Ingestion Jobs (Section 6.3)
    pub fn enqueue_ingestion_job(&self, id: &str, filename: &str, file_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO ingestion_jobs (id, filename, file_path, status, attempts, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', 0, ?4, ?5)",
            params![id, filename, file_path, now, now],
        )?;
        Ok(())
    }

    pub fn claim_next_ingestion_job(&self) -> Result<Option<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, file_path FROM ingestion_jobs
             WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let filename: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE ingestion_jobs SET status = 'processing', attempts = attempts + 1, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
            Ok(Some((id, filename, file_path)))
        } else {
            Ok(None)
        }
    }

    pub fn finish_ingestion_job(&self, id: &str, status: &str, error_message: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE ingestion_jobs SET status = ?1, error_message = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, error_message, now, id],
        )?;
        Ok(())
    }
}

