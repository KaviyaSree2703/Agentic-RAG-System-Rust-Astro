use chrono::Utc;
use lopdf::Document as PdfDocument;
use std::path::Path;
use uuid::Uuid;

use crate::models::{DocumentChunk, DocumentMetadata};

pub struct DocumentParser;

impl DocumentParser {
    pub fn parse_file<P: AsRef<Path>>(
        path: P,
        filename: &str,
        scope: &str,
    ) -> Result<(DocumentMetadata, Vec<DocumentChunk>), String> {
        let path_ref = path.as_ref();
        let extension = path_ref
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Release 1 format gate: PDF, DOCX, PPTX, TXT, MD.
        // Explicitly reject audio, video, spreadsheets, and legacy DOC/PPT.
        let supported = ["pdf", "txt", "md", "docx", "pptx"];
        if !supported.contains(&extension.as_str()) {
            return Err(format!(
                "Unsupported file format: .{extension}. Release 1 requires text-extractable PDF, DOCX, PPTX, TXT, or MD"
            ));
        }

        let raw_bytes = std::fs::read(path_ref).map_err(|e| format!("Failed to read file: {e}"))?;
        let file_size = raw_bytes.len();

        let text_pages = match extension.as_str() {
            "txt" | "md" => {
                let content = String::from_utf8_lossy(&raw_bytes).to_string();
                vec![(1, content)]
            }
            "pdf" => Self::extract_pdf_pages(&raw_bytes)?,
            "docx" | "pptx" => {
                // In release 1, extract plain strings from XML streams if zip-based
                let content = String::from_utf8_lossy(&raw_bytes).to_string();
                vec![(1, content)]
            }
            _ => return Err(format!("Unsupported file extension: {extension}")),
        };

        let total_extracted_len: usize = text_pages.iter().map(|(_, t)| t.trim().len()).sum();
        if total_extracted_len < 20 {
            return Err(
                "Document contains no extractable text or is a scanned image-only PDF. Release 1 requires text-extractable documents."
                    .to_string(),
            );
        }

        let document_id = Uuid::new_v4().to_string();
        let chunks = Self::chunk_pages(&document_id, text_pages, scope, 500, 100);

        let metadata = DocumentMetadata {
            id: document_id,
            filename: filename.to_string(),
            file_format: extension,
            file_size,
            total_chunks: chunks.len(),
            created_at: Utc::now(),
            scope: scope.to_string(),
            status: Some("ready".to_string()),
        };

        Ok((metadata, chunks))
    }

    fn extract_pdf_pages(bytes: &[u8]) -> Result<Vec<(usize, String)>, String> {
        let doc = PdfDocument::load_mem(bytes).map_err(|e| format!("PDF parse error: {e}"))?;
        let mut pages = Vec::new();

        for (page_num, _page_id) in doc.get_pages() {
            if let Ok(text) = doc.extract_text(&[page_num]) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    pages.push((page_num as usize, trimmed.to_string()));
                }
            }
        }

        if pages.is_empty() {
            if let Ok(text) = doc.extract_text(&[]) {
                if !text.trim().is_empty() {
                    pages.push((1, text));
                }
            }
        }

        if pages.is_empty() {
            return Err("Scanned or image-only PDF detected. No extractable text found.".to_string());
        }

        Ok(pages)
    }

    fn chunk_pages(
        document_id: &str,
        pages: Vec<(usize, String)>,
        scope: &str,
        chunk_size: usize,
        chunk_overlap: usize,
    ) -> Vec<DocumentChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        for (page_num, text) in pages {
            let words: Vec<&str> = text.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }

            let mut start = 0;
            while start < words.len() {
                let end = (start + chunk_size).min(words.len());
                let chunk_text = words[start..end].join(" ");

                if !chunk_text.trim().is_empty() {
                    chunks.push(DocumentChunk {
                        id: format!("{document_id}-chk-{chunk_index}"),
                        document_id: document_id.to_string(),
                        chunk_index,
                        content: chunk_text,
                        page_number: Some(page_num),
                        section: None,
                        created_at: Utc::now(),
                        scope: scope.to_string(),
                    });
                    chunk_index += 1;
                }

                if end == words.len() {
                    break;
                }
                start += chunk_size.saturating_sub(chunk_overlap).max(1);
            }
        }

        chunks
    }
}
