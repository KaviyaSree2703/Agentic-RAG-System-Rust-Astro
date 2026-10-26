use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use backend::api::{AppState, create_router};
use backend::config::AppConfig;
use backend::db::Database;
use backend::orchestrator::AgentOrchestrator;
use backend::retrieval::SearchEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(AppConfig::from_env());
    info!("Starting v11 Agentic RAG API Server on port {}", config.port);

    let db = Database::new(&config.database_path)
        .map_err(|e| format!("Failed to init SQLite DB: {e}"))?;

    let search_engine = SearchEngine::new(&config.index_path)
        .map_err(|e| format!("Failed to init Tantivy search engine: {e}"))?;

    let orchestrator = Arc::new(AgentOrchestrator::new(
        config.clone(),
        db.clone(),
        search_engine.clone(),
    ));

    let state = AppState {
        config: config.clone(),
        db,
        search_engine,
        orchestrator,
    };

    let app = create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    info!("v11 API server listening natively on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
