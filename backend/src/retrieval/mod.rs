use std::path::Path;
use std::sync::{Arc, Mutex};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, doc};

use crate::models::{DocumentChunk, EvidenceItem};

#[derive(Clone)]
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    f_chunk_id: Field,
    f_document_id: Field,
    f_document_name: Field,
    f_content: Field,
    f_page_number: Field,
    f_scope: Field,
}

impl SearchEngine {
    pub fn new<P: AsRef<Path>>(index_dir: P) -> Result<Self, String> {
        let path = index_dir.as_ref();
        std::fs::create_dir_all(path).map_err(|e| format!("Failed to create index dir: {e}"))?;

        let mut schema_builder = Schema::builder();
        let f_chunk_id = schema_builder.add_text_field("chunk_id", STRING | STORED);
        let f_document_id = schema_builder.add_text_field("document_id", STRING | STORED);
        let f_document_name = schema_builder.add_text_field("document_name", TEXT | STORED);
        let f_content = schema_builder.add_text_field("content", TEXT | STORED);
        let f_page_number = schema_builder.add_u64_field("page_number", STORED | INDEXED);
        let f_scope = schema_builder.add_text_field("scope", STRING | STORED);
        let schema = schema_builder.build();

        let index = if Index::open_in_dir(path).is_ok() {
            Index::open_in_dir(path).map_err(|e| format!("Failed to open index: {e}"))?
        } else {
            Index::create_in_dir(path, schema)
                .map_err(|e| format!("Failed to create index: {e}"))?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| format!("Failed to create index reader: {e}"))?;

        let writer = index
            .writer(50_000_000)
            .map_err(|e| format!("Failed to create index writer: {e}"))?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            f_chunk_id,
            f_document_id,
            f_document_name,
            f_content,
            f_page_number,
            f_scope,
        })
    }

    pub fn index_chunks(&self, doc_name: &str, chunks: &[DocumentChunk]) -> Result<(), String> {
        let mut writer = self.writer.lock().map_err(|e| e.to_string())?;

        for chunk in chunks {
            let page_num = chunk.page_number.unwrap_or(1) as u64;
            writer
                .add_document(doc!(
                    self.f_chunk_id => chunk.id.clone(),
                    self.f_document_id => chunk.document_id.clone(),
                    self.f_document_name => doc_name.to_string(),
                    self.f_content => chunk.content.clone(),
                    self.f_page_number => page_num,
                    self.f_scope => chunk.scope.clone(),
                ))
                .map_err(|e| format!("Error adding doc to index: {e}"))?;
        }

        writer
            .commit()
            .map_err(|e| format!("Error committing index: {e}"))?;
        Ok(())
    }

    pub fn delete_document_chunks(&self, doc_id: &str) -> Result<(), String> {
        let mut writer = self.writer.lock().map_err(|e| e.to_string())?;
        let term = Term::from_field_text(self.f_document_id, doc_id);
        writer.delete_term(term);
        writer
            .commit()
            .map_err(|e| format!("Error committing index deletion: {e}"))?;
        Ok(())
    }

    pub fn search(
        &self,
        query_str: &str,
        top_k: usize,
        scope: &str,
    ) -> Result<Vec<EvidenceItem>, String> {
        self.reader
            .reload()
            .map_err(|e| format!("Reader reload error: {e}"))?;
        let searcher = self.reader.searcher();

        let query_parser =
            QueryParser::for_index(&self.index, vec![self.f_content, self.f_document_name]);

        // Escape Tantivy query syntax characters to prevent parsing panics on arbitrary user input
        let sanitized = sanitize_query(query_str);
        let parsed_query = match query_parser.parse_query(&sanitized) {
            Ok(q) => q,
            Err(_) => {
                // Fallback to searching each word as terms
                let simple_query = sanitized
                    .split_whitespace()
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                query_parser
                    .parse_query(&simple_query)
                    .map_err(|e| format!("Failed to parse query: {e}"))?
            }
        };

        let top_docs = searcher
            .search(
                &parsed_query,
                &TopDocs::with_limit(top_k * 2).order_by_score(),
            )
            .map_err(|e| format!("Search error: {e}"))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| format!("Doc retrieval error: {e}"))?;

            let doc_scope = retrieved_doc
                .get_first(self.f_scope)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if doc_scope != scope && scope != "*" {
                continue;
            }

            let chunk_id = retrieved_doc
                .get_first(self.f_chunk_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let document_id = retrieved_doc
                .get_first(self.f_document_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let document_name = retrieved_doc
                .get_first(self.f_document_name)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let content = retrieved_doc
                .get_first(self.f_content)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let page_number = retrieved_doc
                .get_first(self.f_page_number)
                .and_then(|v| v.as_u64())
                .map(|p| p as usize);

            results.push(EvidenceItem {
                chunk_id,
                document_id,
                document_name,
                content,
                score,
                retrieval_method: "tantivy_bm25".to_string(),
                page_number,
                section: None,
            });

            if results.len() >= top_k {
                break;
            }
        }

        Ok(results)
    }
}

fn sanitize_query(input: &str) -> String {
    let special_chars = [
        '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\',
        '/',
    ];
    let mut clean = String::new();
    for c in input.chars() {
        if special_chars.contains(&c) {
            clean.push(' ');
        } else {
            clean.push(c);
        }
    }
    clean
}
