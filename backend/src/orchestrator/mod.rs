use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::evaluator::EvidenceEvaluator;
use crate::models::{Citation, EvidenceItem, Message};
use crate::retrieval::SearchEngine;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    StageChange {
        stage: String,
        status: String,
        detail: String,
        attempt: Option<usize>,
    },
    ToolEvent {
        tool_name: String,
        status: String,
        summary: String,
    },
    EvidenceEvaluation {
        is_sufficient: bool,
        candidate_count: usize,
        retained_count: usize,
        strategy: String,
        retry_reason: Option<String>,
        notes: String,
    },
    ContextReady {
        citations: Vec<Citation>,
        sufficiency: String,
    },
    Token {
        delta: String,
    },
    Done {
        answer: String,
        execution_time_ms: u128,
        total_attempts: usize,
    },
    Error {
        message: String,
        code: String,
    },
}

pub struct AgentOrchestrator {
    config: Arc<AppConfig>,
    db: Database,
    search_engine: SearchEngine,
    http_client: reqwest::Client,
}

impl AgentOrchestrator {
    pub fn new(config: Arc<AppConfig>, db: Database, search_engine: SearchEngine) -> Self {
        Self {
            config,
            db,
            search_engine,
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn run_agentic_loop(
        &self,
        query: &str,
        conversation_id: &str,
        event_sender: mpsc::Sender<AgentEvent>,
    ) {
        let start_time = Instant::now();

        // Stage 1: Planning
        let _ = event_sender
            .send(AgentEvent::StageChange {
                stage: "planning".to_string(),
                status: "in_progress".to_string(),
                detail: "Analyzing query structure and formulating retrieval plan".to_string(),
                attempt: Some(1),
            })
            .await;

        let mut current_query = query.to_string();
        let mut attempt = 0;
        let mut accumulated_evidence: Vec<EvidenceItem> = Vec::new();
        let max_attempts = self.config.max_retrieval_attempts;
        let mut is_sufficient = false;

        // Stage 2: Adaptive retrieval & evaluation control loop
        while attempt < max_attempts {
            attempt += 1;

            let _ = event_sender
                .send(AgentEvent::StageChange {
                    stage: "retrieving".to_string(),
                    status: "in_progress".to_string(),
                    detail: format!(
                        "Executing hybrid Tantivy BM25 retrieval for: \"{current_query}\""
                    ),
                    attempt: Some(attempt),
                })
                .await;

            let _ = event_sender
                .send(AgentEvent::ToolEvent {
                    tool_name: "vector_search".to_string(),
                    status: "started".to_string(),
                    summary: format!("Retrieving top candidate chunks for \"{current_query}\""),
                })
                .await;

            let retrieved =
                match self
                    .search_engine
                    .search(&current_query, self.config.top_k_chunks, "*")
                {
                    Ok(items) => items,
                    Err(err) => {
                        warn!("Search error on attempt {attempt}: {err}");
                        Vec::new()
                    }
                };

            let _ = event_sender
                .send(AgentEvent::ToolEvent {
                    tool_name: "vector_search".to_string(),
                    status: "completed".to_string(),
                    summary: format!("Found {} passage candidates", retrieved.len()),
                })
                .await;

            let initial_candidate_count = retrieved.len();

            // Deduplicate chunks by ID
            for item in retrieved {
                if !accumulated_evidence
                    .iter()
                    .any(|e| e.chunk_id == item.chunk_id)
                {
                    accumulated_evidence.push(item);
                }
            }

            // Stage: Evidence evaluator
            let _ = event_sender
                .send(AgentEvent::StageChange {
                    stage: "evaluating".to_string(),
                    status: "in_progress".to_string(),
                    detail:
                        "Verifying retrieved evidence completeness, coverage, and contradictions"
                            .to_string(),
                    attempt: Some(attempt),
                })
                .await;

            let eval = EvidenceEvaluator::evaluate(
                query,
                &accumulated_evidence,
                self.config.eval_min_confidence,
            );

            let _ = event_sender
                .send(AgentEvent::EvidenceEvaluation {
                    is_sufficient: eval.is_sufficient,
                    candidate_count: initial_candidate_count,
                    retained_count: accumulated_evidence.len(),
                    strategy: "hybrid".to_string(),
                    retry_reason: if eval.is_sufficient {
                        None
                    } else {
                        Some("Coverage below confidence threshold".to_string())
                    },
                    notes: eval.reason.clone(),
                })
                .await;

            if eval.is_sufficient {
                is_sufficient = true;
                info!("Evidence is sufficient on attempt {attempt}");
                break;
            }

            // Adaptive query rewriting
            if attempt < max_attempts {
                if let Some(rewrite) = eval.suggested_query_rewrite {
                    current_query = rewrite.clone();
                    let _ = event_sender
                        .send(AgentEvent::StageChange {
                            stage: "refining".to_string(),
                            status: "retrying".to_string(),
                            detail: format!("Broadening search terms to: \"{rewrite}\""),
                            attempt: Some(attempt + 1),
                        })
                        .await;
                }
            }
        }

        // Stage 3: Assembling context & citations
        let _ = event_sender
            .send(AgentEvent::StageChange {
                stage: "building_context".to_string(),
                status: "in_progress".to_string(),
                detail: format!(
                    "Compiling {} verified evidence passages into grounded prompt",
                    accumulated_evidence.len()
                ),
                attempt: Some(attempt),
            })
            .await;

        let mut citations = Vec::new();
        let mut context_text = String::new();

        for (idx, ev) in accumulated_evidence.iter().enumerate() {
            let page_text = match ev.page_number {
                Some(p) => format!("Page {p}"),
                None => "Section 1".to_string(),
            };

            citations.push(Citation {
                id: format!("c{}", idx + 1),
                title: ev.document_name.clone(),
                filename: ev.document_name.clone(),
                source_location: Some(page_text.clone()),
                content: ev.content.clone(),
                score: ev.score,
                verified: true,
            });

            context_text.push_str(&format!(
                "--- Citation [{}] from {} ({}) ---\n{}\n\n",
                idx + 1,
                ev.document_name,
                page_text,
                ev.content
            ));
        }

        let sufficiency_label = if accumulated_evidence.is_empty() {
            "insufficient"
        } else if !is_sufficient {
            "partial"
        } else {
            "sufficient"
        };

        let _ = event_sender
            .send(AgentEvent::ContextReady {
                citations: citations.clone(),
                sufficiency: sufficiency_label.to_string(),
            })
            .await;

        // Stage 4: Answer generation
        let _ = event_sender
            .send(AgentEvent::StageChange {
                stage: "generating".to_string(),
                status: "in_progress".to_string(),
                detail: "Synthesizing answer grounded exclusively in verified citations"
                    .to_string(),
                attempt: Some(attempt),
            })
            .await;

        let system_prompt = format!(
            "You are an expert, production-grade Agentic RAG assistant.\n\
             Answer the user's question using ONLY the retrieved context below.\n\
             Cite sources inline using [1], [2] corresponding to the citations.\n\
             If evidence is incomplete, explicitly acknowledge missing facts.\n\n\
             Retrieved Evidence Context:\n{}",
            if context_text.is_empty() {
                "No relevant documents found."
            } else {
                &context_text
            }
        );

        let full_answer = if !self.config.openrouter_api_key.is_empty() {
            let llm_result = self
                .call_openrouter_streaming(&system_prompt, query, event_sender.clone())
                .await;

            match llm_result {
                Ok(answer) => answer,
                Err(err) => {
                    warn!("LLM streaming failed, falling back to local grounded synthesis: {err}");
                    self.synthesize_grounded_answer(
                        query,
                        &accumulated_evidence,
                        is_sufficient,
                        event_sender.clone(),
                    )
                    .await
                }
            }
        } else {
            self.synthesize_grounded_answer(
                query,
                &accumulated_evidence,
                is_sufficient,
                event_sender.clone(),
            )
            .await
        };

        // Persist message with citations in database
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: full_answer.clone(),
            sources: Some(citations),
            sufficiency: Some(sufficiency_label.to_string()),
            created_at: chrono::Utc::now(),
        };
        let _ = self.db.save_message(&msg);

        let elapsed = start_time.elapsed().as_millis();

        let _ = event_sender
            .send(AgentEvent::StageChange {
                stage: "completed".to_string(),
                status: "completed".to_string(),
                detail: format!("Finished agentic RAG generation in {elapsed}ms"),
                attempt: Some(attempt),
            })
            .await;

        let _ = event_sender
            .send(AgentEvent::Done {
                answer: full_answer,
                execution_time_ms: elapsed,
                total_attempts: attempt,
            })
            .await;
    }

    async fn call_openrouter_streaming(
        &self,
        system_prompt: &str,
        user_query: &str,
        event_sender: mpsc::Sender<AgentEvent>,
    ) -> Result<String, String> {
        let payload = serde_json::json!({
            "model": self.config.openrouter_model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_query }
            ],
            "stream": true,
            "temperature": 0.2
        });

        let res = self
            .http_client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.openrouter_api_key),
            )
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "http://localhost:4321")
            .header("X-Title", "Rig Knowledge Agent")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("OpenRouter API error {status}: {body}"));
        }

        let mut full_text = String::new();
        use futures::StreamExt;
        let mut byte_stream = res.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = byte_stream.next().await {
            let bytes = chunk_res.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find("\n\n") {
                let message = buffer[..pos].trim().to_string();
                buffer.drain(..pos + 2);

                for line in message.lines() {
                    let line = line.trim();
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" {
                            break;
                        }
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(delta) = val["choices"][0]["delta"]["content"].as_str() {
                                full_text.push_str(delta);
                                let _ = event_sender
                                    .send(AgentEvent::Token {
                                        delta: delta.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        Ok(full_text)
    }

    async fn synthesize_grounded_answer(
        &self,
        query: &str,
        evidence: &[EvidenceItem],
        is_sufficient: bool,
        event_sender: mpsc::Sender<AgentEvent>,
    ) -> String {
        let answer = if evidence.is_empty() {
            format!(
                "I searched the knowledge catalog for \"{query}\", but found no relevant documents. Please upload the relevant files to enable grounded answers."
            )
        } else if !is_sufficient {
            let top_chunk = &evidence[0];
            format!(
                "### Partial Grounding\nThe retrieved documents contain partial context regarding \"{query}\", but did not yield complete factual certainty.\n\n**Most relevant excerpt from {}:**\n> {}\n\nPlease upload additional documentation if a deeper answer is required.",
                top_chunk.document_name,
                top_chunk.content.chars().take(300).collect::<String>()
            )
        } else {
            let mut summary = format!(
                "### Grounded Synthesis for \"{query}\"\n\nBased on {} retrieved evidence chunk(s):\n\n",
                evidence.len()
            );
            for (idx, ev) in evidence.iter().take(3).enumerate() {
                summary.push_str(&format!(
                    "[{}] **{}** ({:?}):\n   {}\n\n",
                    idx + 1,
                    ev.document_name,
                    ev.page_number
                        .map(|p| format!("Page {p}"))
                        .unwrap_or_else(|| "Section 1".to_string()),
                    ev.content.chars().take(250).collect::<String>()
                ));
            }
            summary
        };

        for word in answer.split_inclusive(' ') {
            let _ = event_sender
                .send(AgentEvent::Token {
                    delta: word.to_string(),
                })
                .await;
            tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        }

        answer
    }
}
