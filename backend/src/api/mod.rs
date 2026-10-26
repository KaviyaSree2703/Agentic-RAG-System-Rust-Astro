use axum::{
    Router,
    extract::{Multipart, Path, State},
    response::{
        IntoResponse, Json,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, post},
};
use futures::stream::Stream;
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
    SearchResponse, SearchResultItem,
};
use crate::orchestrator::{AgentEvent, AgentOrchestrator};
use crate::retrieval::SearchEngine;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Database,
    pub search_engine: SearchEngine,
    pub orchestrator: Arc<AgentOrchestrator>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .route("/api/search", post(search_knowledge))
        .route("/api/documents", get(list_documents))
        .route("/api/documents/{id}", delete(remove_document_path))
        .route("/api/documents/remove", post(remove_document_body))
        .route("/api/upload", post(upload_document))
        .route("/api/conversations", get(list_conversations))
        .route("/api/conversations/{id}", get(get_conversation))
        .route("/api/conversations/{id}", delete(delete_conversation_path))
        .route("/api/conversations/delete", post(delete_conversation_body))
        .route("/api/chat", post(chat_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "version": "1.0.0",
        "modelProvider": "Gemini / OpenRouter / RIG Native",
        "embeddingModel": "Tantivy-BM25-Hybrid",
        "vectorBackend": "Native Tantivy Index + SQLite",
        "retrievalStrategies": ["hybrid", "semantic", "keyword"],
        "uptimeSeconds": 120
    }))
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

        let ext = std::path::Path::new(&safe_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let allowed_exts = ["pdf", "txt", "md", "json", "csv", "log"];
        if !allowed_exts.contains(&ext.as_str()) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Unsupported file format: .{ext}. Allowed: PDF, TXT, MD, JSON, CSV") })),
            )
                .into_response();
        }

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
                tracing::warn!("Document parsing failed for {safe_name}: {e}");
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed to parse document: {e}") })),
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
    let messages = state.db.get_conversation_messages(&id).unwrap_or_default();
    Json(json!({ "id": id, "messages": messages })).into_response()
}

async fn delete_conversation_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_conversation(&id) {
        Ok(true) => Json(json!({ "success": true })).into_response(),
        Ok(false) => {
            Json(json!({ "success": false, "error": "Conversation not found" })).into_response()
        }
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

async fn chat_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let conversation_id = payload
        .conversation_id
        .or(payload.session_id)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let _ = state
        .db
        .get_or_create_conversation(&conversation_id, &payload.query);

    // Save user message
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

    let (event_sender, mut event_receiver) = mpsc::channel::<AgentEvent>(100);
    let orchestrator = state.orchestrator.clone();
    let query_clone = payload.query.clone();
    let conv_id_task1 = conversation_id.clone();

    tokio::spawn(async move {
        orchestrator
            .run_agentic_loop(&query_clone, &conv_id_task1, event_sender)
            .await;
    });

    let (sse_sender, sse_receiver) = mpsc::channel::<Result<Event, axum::Error>>(100);

    tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            match event {
                AgentEvent::StageChange {
                    stage,
                    status,
                    detail,
                    attempt,
                } => {
                    let data = json!({
                        "type": "stage_change",
                        "data": {
                            "stage": stage,
                            "status": status,
                            "detail": detail,
                            "attempt": attempt
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                }
                AgentEvent::ToolEvent {
                    tool_name,
                    status,
                    summary,
                } => {
                    let data = json!({
                        "type": "tool_event",
                        "data": {
                            "toolName": tool_name,
                            "status": status,
                            "summary": summary
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                }
                AgentEvent::EvidenceEvaluation {
                    is_sufficient,
                    candidate_count,
                    retained_count,
                    strategy,
                    retry_reason,
                    notes,
                } => {
                    let data = json!({
                        "type": "evidence_evaluation",
                        "data": {
                            "isSufficient": is_sufficient,
                            "candidateCount": candidate_count,
                            "retainedCount": retained_count,
                            "strategy": strategy,
                            "retryReason": retry_reason,
                            "notes": notes
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                }
                AgentEvent::ContextReady {
                    citations,
                    sufficiency,
                } => {
                    let data = json!({
                        "type": "context_ready",
                        "data": {
                            "sufficiency": sufficiency,
                            "citations": citations
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                }
                AgentEvent::Token { delta } => {
                    let data = json!({
                        "type": "token",
                        "data": {
                            "token": delta
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                }
                AgentEvent::Done {
                    answer,
                    execution_time_ms,
                    total_attempts,
                } => {
                    let data = json!({
                        "type": "done",
                        "data": {
                            "answer": answer,
                            "executionTimeMs": execution_time_ms,
                            "totalAttempts": total_attempts
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                    break;
                }
                AgentEvent::Error { message, code } => {
                    let data = json!({
                        "type": "error",
                        "data": {
                            "message": message,
                            "code": code
                        }
                    });
                    let _ = sse_sender
                        .send(Ok(Event::default().data(data.to_string())))
                        .await;
                    break;
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(sse_receiver)).keep_alive(KeepAlive::default())
}
