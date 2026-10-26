use backend::db::Database;
use backend::evaluator::EvidenceEvaluator;
use backend::ingestion::DocumentParser;
use backend::models::{DocumentChunk, DocumentMetadata, EvidenceItem};
use backend::retrieval::SearchEngine;

#[test]
fn test_database_crud_and_conversations() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::new(&db_path).unwrap();

    let doc_meta = DocumentMetadata {
        id: "doc-123".to_string(),
        filename: "test_report.txt".to_string(),
        file_format: "txt".to_string(),
        file_size: 1024,
        total_chunks: 2,
        created_at: chrono::Utc::now(),
        scope: "default".to_string(),
        status: Some("ready".to_string()),
    };
    db.save_document(&doc_meta).unwrap();

    let docs = db.list_documents().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "doc-123");

    // Conversation & message persistence
    let conv = db
        .get_or_create_conversation("conv-abc", "Agentic RAG Test")
        .unwrap();
    assert_eq!(conv.id, "conv-abc");

    let convs = db.list_conversations().unwrap();
    assert_eq!(convs.len(), 1);

    // Delete document
    let deleted = db.delete_document("doc-123").unwrap();
    assert!(deleted);
    let docs_after = db.list_documents().unwrap();
    assert_eq!(docs_after.len(), 0);
}

#[test]
fn test_document_parser_and_chunking() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("sample.txt");
    let content = "Agentic RAG enables autonomous evidence retrieval, validation, query rewriting, and citation verification.";
    std::fs::write(&file_path, content).unwrap();

    let (meta, chunks) = DocumentParser::parse_file(&file_path, "sample.txt", "default").unwrap();
    assert_eq!(meta.filename, "sample.txt");
    assert!(!chunks.is_empty());
    assert!(chunks[0].content.contains("Agentic RAG enables autonomous"));
}

#[test]
fn test_search_engine_indexing_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let index_dir = dir.path().join("tantivy_idx");
    let engine = SearchEngine::new(&index_dir).unwrap();

    let chunks = vec![
        DocumentChunk {
            id: "chk-1".to_string(),
            document_id: "doc-1".to_string(),
            chunk_index: 0,
            content: "Rust is a systems programming language with memory safety and zero-cost abstractions.".to_string(),
            page_number: Some(1),
            section: None,
            created_at: chrono::Utc::now(),
            scope: "default".to_string(),
        },
        DocumentChunk {
            id: "chk-2".to_string(),
            document_id: "doc-1".to_string(),
            chunk_index: 1,
            content: "Astro is a modern web framework that delivers fast, content-focused static and dynamic sites.".to_string(),
            page_number: Some(2),
            section: None,
            created_at: chrono::Utc::now(),
            scope: "default".to_string(),
        },
    ];

    engine.index_chunks("sample_guide.txt", &chunks).unwrap();

    let results = engine
        .search("memory safety abstractions", 5, "default")
        .unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].chunk_id, "chk-1");
    assert!(results[0].content.contains("memory safety"));
}

#[test]
fn test_evidence_evaluator_guardrails() {
    let query = "What are the core memory safety benefits of Rust?";
    let evidence_matching = vec![EvidenceItem {
        chunk_id: "chk-1".to_string(),
        document_id: "doc-1".to_string(),
        document_name: "rust_intro.md".to_string(),
        content:
            "Rust provides guaranteed memory safety through its borrow checker and ownership model."
                .to_string(),
        score: 12.5,
        retrieval_method: "tantivy_bm25".to_string(),
        page_number: Some(1),
        section: None,
    }];

    let eval_pass = EvidenceEvaluator::evaluate(query, &evidence_matching, 0.5);
    assert!(eval_pass.is_sufficient);
    assert!(eval_pass.confidence_score >= 0.5);

    // Test insufficient evidence triggering rewrite suggestion
    let evidence_empty: Vec<EvidenceItem> = Vec::new();
    let eval_fail = EvidenceEvaluator::evaluate(query, &evidence_empty, 0.5);
    assert!(!eval_fail.is_sufficient);
    assert!(eval_fail.suggested_query_rewrite.is_some());
}

#[test]
fn test_database_migration_adds_status_safely() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("legacy.db");

    // 1. Create a legacy database without 'status' column
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE documents (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                file_format TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                total_chunks INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                scope TEXT NOT NULL
            );
            INSERT INTO documents VALUES (
                'legacy-1', 'old_file.pdf', 'pdf', 2048, 4, '2026-01-01T00:00:00Z', 'default'
            );
            "#,
        )
        .unwrap();
    }

    // 2. Open via Database::new which applies run_migrations
    let db = Database::new(&db_path).unwrap();

    // 3. Verify legacy data survived and acquired default 'ready' status
    let docs = db.list_documents().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "legacy-1");
    assert_eq!(docs[0].status.as_deref(), Some("ready"));

    // 4. Verify new document with custom status can be saved and retrieved
    let new_doc = DocumentMetadata {
        id: "new-2".to_string(),
        filename: "new_file.pdf".to_string(),
        file_format: "pdf".to_string(),
        file_size: 4096,
        total_chunks: 8,
        created_at: chrono::Utc::now(),
        scope: "default".to_string(),
        status: Some("ready".to_string()),
    };
    db.save_document(&new_doc).unwrap();

    let docs_updated = db.list_documents().unwrap();
    assert_eq!(docs_updated.len(), 2);
}

#[test]
fn test_pdf_extraction_and_chunking() {
    let sample_pdf_path =
        std::path::Path::new("D:/RIG-Knowledge-Agent-main/sample_documents/deepseek_r1.pdf");
    if sample_pdf_path.exists() {
        let (meta, chunks) =
            DocumentParser::parse_file(sample_pdf_path, "deepseek_r1.pdf", "default").unwrap();
        assert_eq!(meta.filename, "deepseek_r1.pdf");
        assert_eq!(meta.file_format, "pdf");
        assert_eq!(meta.status.as_deref(), Some("ready"));
        assert!(!chunks.is_empty());
        assert!(
            chunks
                .iter()
                .any(|c| c.content.to_lowercase().contains("deepseek")
                    || c.content.to_lowercase().contains("model"))
        );
    }
}
