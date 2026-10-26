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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

/// Canonical v11 JSON Lines Claim Unit (Section 8.2 & 9.2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimUnit {
    pub u: u32,
    pub role: String, // "answer", "support", "caveat"
    pub text: String,
    pub cites: Vec<u32>,
    pub reqs: Vec<String>,
}

/// v11 Terminal Outcome states (Section 7.1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalOutcome {
    Answer,
    ReferToOwner { reason: String, owner: String },
    SocialReply { message: String },
    AskUser { question: String },
    FailGracefully { reason: String },
    UpstreamFailure { reason: String },
}

/// Pre-Router classification outcome (Section 8.1 & 8.2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RouteCategory {
    SocialGreeting,
    SocialThanks,
    DocumentQuery,
    GeneralKnowledge,
    LiveExactData,
    OpinionOrSubjective,
    OutOfDomain,
    ClarificationNeeded(String),
}

/// v11 Plan definition for PlanValidator (Section 8.3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubQuestion {
    pub id: String, // e.g. "q1", "q2"
    pub question: String,
    pub depends_on: Vec<String>,
    pub required_entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryPlan {
    pub original_query: String,
    pub subquestions: Vec<SubQuestion>,
    pub requires_exact: bool,
}

/// v11 Unified Event representation for SSE streaming & run_events persistence
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum V11EventPayload {
    #[serde(rename = "status")]
    Status {
        stage: String,
        status: String,
        detail: String,
        attempt: Option<usize>,
    },
    #[serde(rename = "token")]
    Token {
        delta: String,
    },
    #[serde(rename = "citations")]
    Citations {
        citations: Vec<Citation>,
        sufficiency: String,
    },
    #[serde(rename = "verification")]
    Verification {
        unit: u32,
        verified: bool,
        role: String,
        cites: Vec<u32>,
        checks: Vec<String>,
    },
    #[serde(rename = "clarification")]
    Clarification {
        question: String,
    },
    #[serde(rename = "done")]
    Done {
        answer: String,
        outcome: TerminalOutcome,
        execution_time_ms: u128,
        total_attempts: usize,
    },
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V11RunEvent {
    pub id: u64,
    pub run_id: String,
    pub event_type: String,
    pub payload: V11EventPayload,
    pub created_at: DateTime<Utc>,
}

