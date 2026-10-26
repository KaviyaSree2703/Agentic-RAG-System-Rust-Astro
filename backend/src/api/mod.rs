use axum::{
    Router,
    extract::{Multipart, Path, Query, State},
    response::{
        IntoResponse, Json,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::db::Database;
use crate::ingestion::DocumentParser;
use crate::models::{
    ChatRequest, DeleteConversationRequest, RemoveDocRequest, RemoveDocResponse, SearchRequest,
    SearchResponse, SearchResultItem, V11EventPayload,
};
use crate::orchestrator::AgentOrchestrator;
use crate::retrieval::SearchEngine;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Database,
    pub search_engine: SearchEngine,
    pub orchestrator: Arc<AgentOrchestrator>,
}

#[derive(Deserialize)]
pub struct ReplayParams {
    pub last_event_id: Option<u64>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // v11 Official Route Contracts (Section 12.1)
        .route("/v1/health", get(v11_health_check))
        .route("/v1/chat/stream", post(v11_chat_stream))
        .route("/v1/search", post(search_knowledge))
        .route("/v1/documents", get(list_documents))
        .route("/v1/documents/upload", post(upload_document))
        .route("/v1/documents/{id}", delete(remove_document_path))
        .route("/v1/runs/{id}/events", get(v11_replay_events))
        // Backward-compatible compatibility adapters (Section 12.1)
        .route("/health", get(v11_health_check))
        .route("/api/health", get(v11_health_check))
        .route("/api/search", post(search_knowledge))
        .route("/api/documents", get(list_documents))
        .route("/api/documents/{id}", delete(remove_document_path))
        .route("/api/documents/remove", post(remove_document_body))
        .route("/api/upload", post(upload_document))
        .route("/api/conversations", get(list_conversations))
        .route("/api/conversations/{id}", get(get_conversation))
        .route("/api/conversations/{id}", delete(delete_conversation_path))
        .route("/api/conversations/delete", post(delete_conversation_body))
        .route("/api/chat", post(v11_chat_stream))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn v11_health_check(State(state): State<AppState>) -> impl IntoResponse {
    let doc_count = state.db.list_documents().map(|d| d.len()).unwrap_or(0);
    Json(json!({
        "status": "healthy",
        "specification": "v11",
        "release": "1.0.0",
        "runtime": "native-rust-tokio",
        "storage": {
            "mode": "sqlite-tantivy-native",
            "indexed_documents": doc_count,
            "status": "online"
        },
        "retrieval": {
            "primary": "hybrid_document_search",
            "fallback": "document_lookup",
            "forbidden_tools_active": false
        },
        "grounding_gate": {
            "checks": ["P0", "P1", "C1", "C2", "C3", "C4", "C5"],
            "status": "enforced"
        }
    }))
}

async fn v11_replay_events(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(params): Query<ReplayParams>,
) -> impl IntoResponse {
    match state.db.get_run_events(&run_id, params.last_event_id) {
        Ok(events) => {
            let formatted: Vec<_> = events
                .into_iter()
                .map(|(id, event_type, payload, created_at)| {
                    json!({
                        "id": id,
                        "type": event_type,
                        "payload": serde_json::from_str::<serde_json::Value>(&payload).unwrap_or(json!(payload)),
                        "created_at": created_at
                    })
                })
                .collect();
            Json(json!({ "run_id": run_id, "events": formatted })).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn search_knowledge(
    State(state): State<AppState>,
    Json(payload): Json<SearchRequest>,
) -> impl IntoResponse {
    match state.search_engine.search(&payload.query, 10, "*") {
        Ok(items) => {
            let results: Vec<SearchResultItem> = items
                .into_iter()
                .map(|item| {
                    let page_str = item.page_number.map(|p| format!("Page {p}"));
                    SearchResultItem {
                        id: item.chunk_id,
                        title: item.document_name,
                        snippet: item.content,
                        document_id: item.document_id,
                        source_location: page_str,
                        score: item.score,
                        strategy: payload
                            .strategy
                            .clone()
                            .unwrap_or_else(|| "hybrid".to_string()),
                        is_evaluated_evidence: true,
                    }
                })
                .collect();
            Json(SearchResponse { results }).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_documents(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_documents() {
        Ok(docs) => Json(json!(docs)).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn remove_document_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = state.search_engine.delete_document_chunks(&id);
    match state.db.delete_document(&id) {
        Ok(true) => Json(RemoveDocResponse {
            success: true,
            message: "Document deleted successfully".to_string(),
        })
        .into_response(),
        Ok(false) => Json(RemoveDocResponse {
            success: false,
            message: "Document not found".to_string(),
        })
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn remove_document_body(
    State(state): State<AppState>,
    Json(payload): Json<RemoveDocRequest>,
) -> impl IntoResponse {
    remove_document_path(State(state), Path(payload.document_id)).await
}

async fn upload_document(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut uploaded_docs = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("unnamed_file.txt").to_string();

        let data = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed to read field: {e}") })),
                )
                    .into_response();
            }
        };

        let safe_name = std::path::Path::new(&file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.txt")
            .to_string();

        let temp_dir = std::env::temp_dir().join("rag_uploads");
        std::fs::create_dir_all(&temp_dir).ok();
        let unique_id = uuid::Uuid::new_v4().to_string();
        let temp_path = temp_dir.join(format!("{unique_id}_{safe_name}"));

        if let Err(e) = std::fs::write(&temp_path, &data) {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to write temporary upload file: {e}") })),
            )
                .into_response();
        }

        let parse_result = DocumentParser::parse_file(&temp_path, &safe_name, "default");
        let _ = std::fs::remove_file(&temp_path);

        match parse_result {
            Ok((meta, chunks)) => {
                if let Err(e) = state.db.save_document(&meta) {
                    tracing::error!("Failed to persist document metadata: {e}");
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to save doc metadata: {e}") })),
                    )
                        .into_response();
                }

                if let Err(e) = state.db.save_chunks(&chunks) {
                    tracing::error!("Failed to persist document chunks: {e}");
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to save chunks: {e}") })),
                    )
                        .into_response();
                }

                if let Err(e) = state.search_engine.index_chunks(&safe_name, &chunks) {
                    tracing::error!("Failed to index chunks into Tantivy: {e}");
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Failed to index chunks: {e}") })),
                    )
                        .into_response();
                }

                uploaded_docs.push(meta);
            }
            Err(e) => {
                tracing::warn!("Document parsing rejected for {safe_name}: {e}");
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Document rejected: {e}") })),
                )
                    .into_response();
            }
        }
    }

    Json(json!({ "success": true, "ingested": uploaded_docs.len(), "documents": uploaded_docs }))
        .into_response()
}

async fn list_conversations(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_conversations() {
        Ok(convs) => Json(json!(convs)).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_conversation_messages(&id) {
        Ok(msgs) => Json(json!(msgs)).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn delete_conversation_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_conversation(&id) {
        Ok(true) => Json(json!({ "success": true })).into_response(),
        Ok(false) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Conversation not found" })),
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn delete_conversation_body(
    State(state): State<AppState>,
    Json(payload): Json<DeleteConversationRequest>,
) -> impl IntoResponse {
    delete_conversation_path(State(state), Path(payload.conversation_id)).await
}

/// Official v11 Streaming SSE Chat Handler (Section 12.3)
/// Emits unified events: status, token, citations, verification, clarification, done, error
/// With monotonic event IDs and dual compatibility for existing Astro frontend.
async fn v11_chat_stream(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, axum::Error>>> {
    let conversation_id = payload
        .conversation_id
        .or(payload.session_id)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let _ = state
        .db
        .get_or_create_conversation(&conversation_id, &payload.query);

    let user_msg = crate::models::Message {
        id: uuid::Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        role: "user".to_string(),
        content: payload.query.clone(),
        sources: None,
        sufficiency: None,
        created_at: chrono::Utc::now(),
    };
    let _ = state.db.save_message(&user_msg);

    let (event_sender, mut event_receiver) = mpsc::channel::<V11EventPayload>(100);
    let orchestrator = state.orchestrator.clone();
    let query_clone = payload.query.clone();
    let conv_id_task = conversation_id.clone();
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_id_task = run_id.clone();

    tokio::spawn(async move {
        orchestrator
            .run_agentic_loop(&query_clone, &conv_id_task, &run_id_task, event_sender)
            .await;
    });

    let (sse_sender, sse_receiver) = mpsc::channel::<Result<Event, axum::Error>>(100);

    tokio::spawn(async move {
        let mut event_seq = 0u64;

        while let Some(event) = event_receiver.recv().await {
            event_seq += 1;
            let (event_type, json_data) = match &event {
                V11EventPayload::Status { stage, status, detail, attempt } => (
                    "status",
                    json!({
                        "stage": stage,
                        "status": status,
                        "detail": detail,
                        "attempt": attempt,
                        // Compatibility fields for Astro frontend
                        "type": "stage_change",
                        "data": { "stage": stage, "status": status, "detail": detail, "attempt": attempt }
                    }),
                ),
                V11EventPayload::Token { delta } => (
                    "token",
                    json!({
                        "token": delta,
                        // Compatibility fields for Astro frontend
                        "type": "token",
                        "data": { "token": delta }
                    }),
                ),
                V11EventPayload::Citations { citations, sufficiency } => (
                    "citations",
                    json!({
                        "citations": citations,
                        "sufficiency": sufficiency,
                        // Compatibility fields for Astro frontend
                        "type": "context_ready",
                        "data": { "citations": citations, "sufficiency": sufficiency }
                    }),
                ),
                V11EventPayload::Verification { unit, verified, role, cites, checks } => (
                    "verification",
                    json!({
                        "unit": unit,
                        "verified": verified,
                        "role": role,
                        "cites": cites,
                        "checks": checks,
                        // Compatibility fields for Astro frontend
                        "type": "evidence_evaluation",
                        "data": { "isSufficient": *verified, "notes": format!("Unit {unit} ({role}) verified: {verified}"), "strategy": "gate_p0_c5" }
                    }),
                ),
                V11EventPayload::Clarification { question } => (
                    "clarification",
                    json!({
                        "question": question,
                        "type": "clarification",
                        "data": { "question": question }
                    }),
                ),
                V11EventPayload::Done { answer, outcome, execution_time_ms, total_attempts } => (
                    "done",
                    json!({
                        "answer": answer,
                        "outcome": outcome,
                        "executionTimeMs": execution_time_ms,
                        "totalAttempts": total_attempts,
                        // Compatibility fields for Astro frontend
                        "type": "done",
                        "data": { "answer": answer, "executionTimeMs": execution_time_ms, "totalAttempts": total_attempts }
                    }),
                ),
                V11EventPayload::Error { code, message } => (
                    "error",
                    json!({
                        "code": code,
                        "message": message,
                        // Compatibility fields for Astro frontend
                        "type": "error",
                        "data": { "code": code, "message": message }
                    }),
                ),
            };

            let sse_ev = Event::default()
                .id(event_seq.to_string())
                .event(event_type)
                .data(json_data.to_string());

            let _ = sse_sender.send(Ok(sse_ev)).await;

            if matches!(event, V11EventPayload::Done { .. } | V11EventPayload::Error { .. }) {
                break;
            }
        }
    });

    Sse::new(ReceiverStream::new(sse_receiver)).keep_alive(KeepAlive::default())
}
