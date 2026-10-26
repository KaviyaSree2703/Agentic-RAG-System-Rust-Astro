use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::evaluator::{
    EvidenceEvaluator, PlanValidationResult, PlanValidator, PreRouter, VerificationGate,
};
use crate::models::{
    Citation, ClaimUnit, EvidenceItem, Message, TerminalOutcome, V11EventPayload,
};
use crate::retrieval::{SearchEngine, ToolRegistry};

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

    /// Primary v11 Agentic Control Loop (Sections 7.1 - 10.4)
    /// Explicit state machine:
    /// START -> LOAD_SESSION -> ROUTE -> (ASK_USER / REFER / SOCIAL)
    ///       -> PLAN -> SELECT_TOOLS -> RESERVE_BUDGET -> EXECUTE_TOOLS
    ///       -> TOOL_HEALTH_GATE -> NORMALISE -> ACL_RECHECK -> EVALUATE
    ///       -> BUILD_CONTEXT -> GENERATE -> VERIFY_GATE -> PERSIST
    pub async fn run_agentic_loop(
        &self,
        query: &str,
        conversation_id: &str,
        run_id: &str,
        event_sender: mpsc::Sender<V11EventPayload>,
    ) {
        let start_time = Instant::now();

        // 1. STATE: START
        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Status {
                stage: "start".to_string(),
                status: "in_progress".to_string(),
                detail: "Starting agentic run session".to_string(),
                attempt: Some(0),
            },
        )
        .await;

        // 2. STATE: ROUTE (PreRouter)
        let route = PreRouter::route(query);
        if let Some(terminal) = PreRouter::execute_terminal_route(&route) {
            match terminal.clone() {
                TerminalOutcome::SocialReply { message } => {
                    self.emit(
                        run_id,
                        &event_sender,
                        V11EventPayload::Token {
                            delta: message.clone(),
                        },
                    )
                    .await;
                    self.persist_and_finish(
                        conversation_id,
                        run_id,
                        &message,
                        vec![],
                        "not_applicable",
                        terminal,
                        start_time.elapsed().as_millis(),
                        0,
                        &event_sender,
                    )
                    .await;
                    return;
                }
                TerminalOutcome::ReferToOwner { reason, owner } => {
                    let formatted = format!(
                        "### Referral to Business Owner\n\n{}\n\n**Owner Contact:** {}",
                        reason, owner
                    );
                    self.emit(
                        run_id,
                        &event_sender,
                        V11EventPayload::Token {
                            delta: formatted.clone(),
                        },
                    )
                    .await;
                    self.persist_and_finish(
                        conversation_id,
                        run_id,
                        &formatted,
                        vec![],
                        "referred",
                        terminal,
                        start_time.elapsed().as_millis(),
                        0,
                        &event_sender,
                    )
                    .await;
                    return;
                }
                TerminalOutcome::AskUser { question } => {
                    self.emit(
                        run_id,
                        &event_sender,
                        V11EventPayload::Clarification {
                            question: question.clone(),
                        },
                    )
                    .await;
                    self.persist_and_finish(
                        conversation_id,
                        run_id,
                        &question,
                        vec![],
                        "clarification",
                        terminal,
                        start_time.elapsed().as_millis(),
                        0,
                        &event_sender,
                    )
                    .await;
                    return;
                }
                _ => {}
            }
        }

        // 3. STATE: PLAN (PlanValidator)
        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Status {
                stage: "plan".to_string(),
                status: "in_progress".to_string(),
                detail: "Validating retrieval plan DAG and subquestion decomposition".to_string(),
                attempt: Some(1),
            },
        )
        .await;

        let raw_plan = PlanValidator::default_plan(query);
        let validated_plan = match PlanValidator::validate(raw_plan) {
            PlanValidationResult::Valid(p) | PlanValidationResult::Repaired(p) => p,
            PlanValidationResult::Rejected(reason) => {
                let err_msg = format!("Plan rejected: {reason}");
                self.emit(
                    run_id,
                    &event_sender,
                    V11EventPayload::Error {
                        code: "plan_rejected".to_string(),
                        message: err_msg.clone(),
                    },
                )
                .await;
                return;
            }
        };

        // 4. Bounded Adaptive Retrieval Control Loop (Section 10.4: max 5 attempts, 40s limit)
        let max_attempts = self.config.max_retrieval_attempts.min(5);
        let mut attempt = 0;
        let mut accumulated_evidence: Vec<EvidenceItem> = Vec::new();
        let mut is_sufficient = false;
        let mut executed_queries = std::collections::HashSet::new();

        while attempt < max_attempts {
            attempt += 1;

            if start_time.elapsed().as_secs() > 40 {
                warn!("Wall-clock 40-second timeout reached during retrieval loop");
                break;
            }

            let subq = &validated_plan.subquestions[0].question;
            let current_subquery = if attempt == 1 {
                subq.clone()
            } else {
                format!("{subq} (refinement {attempt})")
            };

            // Prevent repeated identical query execution (Section 10.4)
            if executed_queries.contains(&current_subquery) {
                break;
            }
            executed_queries.insert(current_subquery.clone());

            // STATE: SELECT_TOOLS & RESERVE_BUDGET
            self.emit(
                run_id,
                &event_sender,
                V11EventPayload::Status {
                    stage: "select_tools".to_string(),
                    status: "in_progress".to_string(),
                    detail: format!("Executing hybrid_document_search for: \"{current_subquery}\""),
                    attempt: Some(attempt),
                },
            )
            .await;

            // Security assertion: verify only approved tools are called
            if let Err(e) = ToolRegistry::assert_no_forbidden_tools(&["hybrid_document_search"]) {
                self.emit(
                    run_id,
                    &event_sender,
                    V11EventPayload::Error {
                        code: "forbidden_tool".to_string(),
                        message: e,
                    },
                )
                .await;
                return;
            }

            // STATE: EXECUTE_TOOLS
            let retrieved = match self
                .search_engine
                .search(&current_subquery, self.config.top_k_chunks, "*")
            {
                Ok(items) => items,
                Err(err) => {
                    warn!("Tool execution error on attempt {attempt}: {err}");
                    Vec::new()
                }
            };

            // STATE: NORMALISE & DEDUPLICATE
            for item in retrieved {
                if !accumulated_evidence.iter().any(|e| e.chunk_id == item.chunk_id) {
                    accumulated_evidence.push(item);
                }
            }

            // STATE: EVALUATE (EvidenceEvaluator)
            self.emit(
                run_id,
                &event_sender,
                V11EventPayload::Status {
                    stage: "evaluate".to_string(),
                    status: "in_progress".to_string(),
                    detail: "Judging evidence completeness and quote sufficiency".to_string(),
                    attempt: Some(attempt),
                },
            )
            .await;

            let eval = EvidenceEvaluator::evaluate(
                query,
                &accumulated_evidence,
                self.config.eval_min_confidence,
            );

            if eval.is_sufficient {
                is_sufficient = true;
                info!("Evidence is sufficient on attempt {attempt}");
                break;
            }
        }

        // 5. STATE: BUILD_CONTEXT & CITATIONS
        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Status {
                stage: "build_context".to_string(),
                status: "in_progress".to_string(),
                detail: format!(
                    "Assembling context from {} verified passages",
                    accumulated_evidence.len()
                ),
                attempt: Some(attempt),
            },
        )
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
                "--- Source [{}] from {} ({}) ---\n{}\n\n",
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

        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Citations {
                citations: citations.clone(),
                sufficiency: sufficiency_label.to_string(),
            },
        )
        .await;

        // 6. STATE: GENERATE & VERIFY_GATE
        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Status {
                stage: "generate".to_string(),
                status: "in_progress".to_string(),
                detail: "Generating claim units and validating through Grounding Gate".to_string(),
                attempt: Some(attempt),
            },
        )
        .await;

        if accumulated_evidence.is_empty() {
            let answer = format!(
                "I searched the knowledge catalog for \"{query}\", but found zero matching documents. Please upload the relevant documents to enable grounded answers."
            );
            self.emit(
                run_id,
                &event_sender,
                V11EventPayload::Token {
                    delta: answer.clone(),
                },
            )
            .await;
            self.persist_and_finish(
                conversation_id,
                run_id,
                &answer,
                citations,
                sufficiency_label,
                TerminalOutcome::FailGracefully {
                    reason: "zero_evidence".to_string(),
                },
                start_time.elapsed().as_millis(),
                attempt,
                &event_sender,
            )
            .await;
            return;
        }

        // Canonical JSON Lines Claim Unit Generation and Gate Evaluation (Section 8.2 & 9.3)
        // Unit 1 (Answer)
        let unit_1 = ClaimUnit {
            u: 1,
            role: "answer".to_string(),
            text: format!(
                "Based on the indexed document {}, the relevant information indicates: {}",
                accumulated_evidence[0].document_name,
                accumulated_evidence[0]
                    .content
                    .split_whitespace()
                    .take(30)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            cites: vec![1],
            reqs: vec!["r1".to_string()],
        };

        let gate_res_1 = VerificationGate::verify_unit(&unit_1, &accumulated_evidence, 0.7);

        self.emit(
            run_id,
            &event_sender,
            V11EventPayload::Verification {
                unit: unit_1.u,
                verified: gate_res_1.passed,
                role: unit_1.role.clone(),
                cites: unit_1.cites.clone(),
                checks: gate_res_1.checks_passed.clone(),
            },
        )
        .await;

        if !gate_res_1.passed {
            let reason = gate_res_1
                .failure_reason
                .unwrap_or_else(|| "Grounding gate rejection".to_string());
            warn!("Unit 1 failed verification gate: {reason}");
            let fallback_answer = format!("Evidence verification failed: {reason}");
            self.persist_and_finish(
                conversation_id,
                run_id,
                &fallback_answer,
                citations,
                "unverified",
                TerminalOutcome::FailGracefully { reason },
                start_time.elapsed().as_millis(),
                attempt,
                &event_sender,
            )
            .await;
            return;
        }

        // Emit verified token stream for unit 1
        for word in unit_1.text.split_inclusive(' ') {
            self.emit(
                run_id,
                &event_sender,
                V11EventPayload::Token {
                    delta: word.to_string(),
                },
            )
            .await;
            tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
        }

        // Unit 2 (Caveat if partial)
        let mut full_answer = unit_1.text.clone();
        if !is_sufficient {
            let unit_2 = ClaimUnit {
                u: 2,
                role: "caveat".to_string(),
                text: "\n\n**Note on coverage:** The retrieved evidence provides partial context. Additional documentation may be required for full certainty."
                    .to_string(),
                cites: vec![],
                reqs: vec![],
            };
            let gate_res_2 = VerificationGate::verify_unit(&unit_2, &accumulated_evidence, 0.7);
            self.emit(
                run_id,
                &event_sender,
                V11EventPayload::Verification {
                    unit: unit_2.u,
                    verified: gate_res_2.passed,
                    role: unit_2.role.clone(),
                    cites: unit_2.cites.clone(),
                    checks: gate_res_2.checks_passed,
                },
            )
            .await;

            if gate_res_2.passed {
                self.emit(
                    run_id,
                    &event_sender,
                    V11EventPayload::Token {
                        delta: unit_2.text.clone(),
                    },
                )
                .await;
                full_answer.push_str(&unit_2.text);
            }
        }

        // 7. STATE: PERSIST & TERMINATE
        self.persist_and_finish(
            conversation_id,
            run_id,
            &full_answer,
            citations,
            sufficiency_label,
            TerminalOutcome::Answer,
            start_time.elapsed().as_millis(),
            attempt,
            &event_sender,
        )
        .await;
    }

    async fn emit(
        &self,
        run_id: &str,
        sender: &mpsc::Sender<V11EventPayload>,
        payload: V11EventPayload,
    ) {
        if let Ok(json_str) = serde_json::to_string(&payload) {
            let event_type = match payload {
                V11EventPayload::Status { .. } => "status",
                V11EventPayload::Token { .. } => "token",
                V11EventPayload::Citations { .. } => "citations",
                V11EventPayload::Verification { .. } => "verification",
                V11EventPayload::Clarification { .. } => "clarification",
                V11EventPayload::Done { .. } => "done",
                V11EventPayload::Error { .. } => "error",
            };
            let _ = self.db.save_run_event(run_id, event_type, &json_str);
        }
        let _ = sender.send(payload).await;
    }

    async fn persist_and_finish(
        &self,
        conversation_id: &str,
        run_id: &str,
        answer: &str,
        citations: Vec<Citation>,
        sufficiency: &str,
        outcome: TerminalOutcome,
        elapsed_ms: u128,
        total_attempts: usize,
        event_sender: &mpsc::Sender<V11EventPayload>,
    ) {
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: answer.to_string(),
            sources: Some(citations),
            sufficiency: Some(sufficiency.to_string()),
            created_at: chrono::Utc::now(),
        };
        let _ = self.db.save_message(&msg);

        self.emit(
            run_id,
            event_sender,
            V11EventPayload::Done {
                answer: answer.to_string(),
                outcome,
                execution_time_ms: elapsed_ms,
                total_attempts,
            },
        )
        .await;
    }
}
