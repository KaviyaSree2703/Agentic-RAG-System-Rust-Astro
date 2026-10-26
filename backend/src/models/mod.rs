use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: String,
    pub filename: String,
    pub file_format: String,
    #[serde(alias = "size_bytes")]
    pub file_size: usize,
    pub total_chunks: usize,
    #[serde(alias = "uploaded_at")]
    pub created_at: DateTime<Utc>,
    pub scope: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub page_number: Option<usize>,
    pub section: Option<String>,
    pub created_at: DateTime<Utc>,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub chunk_id: String,
    pub document_id: String,
    pub document_name: String,
    pub content: String,
    pub score: f32,
    pub retrieval_method: String,
    pub page_number: Option<usize>,
    pub section: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub id: String,
    pub title: String,
    pub filename: String,
    #[serde(rename = "sourceLocation", skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
    pub content: String,
    pub score: f32,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub sources: Option<Vec<Citation>>,
    pub sufficiency: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub query: String,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub strategy: Option<String>,
    pub active_doc_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub document_id: String,
    pub source_location: Option<String>,
    pub score: f32,
    pub strategy: String,
    pub is_evaluated_evidence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveDocRequest {
    pub document_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveDocResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteConversationRequest {
    pub conversation_id: String,
}
