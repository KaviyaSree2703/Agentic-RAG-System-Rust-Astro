use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_path: String,
    pub index_path: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub max_retrieval_attempts: usize,
    pub max_evaluation_retries: usize,
    pub top_k_chunks: usize,
    pub eval_min_confidence: f32,
    pub log_level: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .unwrap_or(8080);

        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let database_path =
            std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/rag.db".to_string());
        let index_path =
            std::env::var("INDEX_PATH").unwrap_or_else(|_| "./data/tantivy_index".to_string());
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
        let openrouter_model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "nvidia/nemotron-3-super-120b-a12b:free".to_string());
        let max_retrieval_attempts = std::env::var("MAX_RETRIEVAL_ATTEMPTS")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .unwrap_or(4);
        let max_evaluation_retries = std::env::var("MAX_EVALUATION_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap_or(3);
        let top_k_chunks = std::env::var("TOP_K_CHUNKS")
            .unwrap_or_else(|_| "6".to_string())
            .parse::<usize>()
            .unwrap_or(6);
        let eval_min_confidence = std::env::var("EVAL_MIN_CONFIDENCE")
            .unwrap_or_else(|_| "0.65".to_string())
            .parse::<f32>()
            .unwrap_or(0.65);
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Self {
            port,
            host,
            database_path,
            index_path,
            openrouter_api_key,
            openrouter_model,
            max_retrieval_attempts,
            max_evaluation_retries,
            top_k_chunks,
            eval_min_confidence,
            log_level,
        }
    }
}
