use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use backend::config::AppConfig;
use backend::db::Database;
use backend::ingestion::DocumentParser;
use backend::retrieval::SearchEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "worker=info,backend=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(AppConfig::from_env());
    info!("Starting v11 Agentic RAG Ingestion Worker Binary");

    let db = Database::new(&config.database_path)
        .map_err(|e| format!("Failed to init SQLite DB: {e}"))?;

    let search_engine = SearchEngine::new(&config.index_path)
        .map_err(|e| format!("Failed to init Tantivy search engine: {e}"))?;

    info!("Ingestion Worker active. Polling durable queue for pending documents...");

    loop {
        match db.claim_next_ingestion_job() {
            Ok(Some((job_id, filename, file_path))) => {
                info!("Claimed ingestion job {job_id} for {filename}");

                match DocumentParser::parse_file(&file_path, &filename, "default") {
                    Ok((meta, chunks)) => {
                        if let Err(e) = db.save_document(&meta) {
                            error!("Failed to save document metadata: {e}");
                            let _ = db.finish_ingestion_job(&job_id, "failed", Some(&e.to_string()));
                            continue;
                        }

                        if let Err(e) = db.save_chunks(&chunks) {
                            error!("Failed to save chunks: {e}");
                            let _ = db.finish_ingestion_job(&job_id, "failed", Some(&e.to_string()));
                            continue;
                        }

                        if let Err(e) = search_engine.index_chunks(&filename, &chunks) {
                            error!("Failed to index chunks: {e}");
                            let _ = db.finish_ingestion_job(&job_id, "failed", Some(&e));
                            continue;
                        }

                        info!("Successfully processed ingestion job {job_id} ({} chunks)", chunks.len());
                        let _ = db.finish_ingestion_job(&job_id, "completed", None);
                    }
                    Err(e) => {
                        error!("Ingestion job {job_id} parse rejection: {e}");
                        let _ = db.finish_ingestion_job(&job_id, "rejected", Some(&e));
                    }
                }
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => {
                error!("Error polling ingestion queue: {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
