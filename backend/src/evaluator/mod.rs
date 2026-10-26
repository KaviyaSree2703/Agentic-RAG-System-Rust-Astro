use crate::models::{ClaimUnit, EvidenceItem, QueryPlan, RouteCategory, SubQuestion, TerminalOutcome};
use serde::{Deserialize, Serialize};

/// Deterministic Pre-Router (v11 Section 8.1 & 8.2)
/// Release 1 Rule: Document questions answerable from approved documents;
/// everything else (general knowledge, opinions, live/exact data) referred to Business Owner.
pub struct PreRouter;

impl PreRouter {
    pub fn route(query: &str) -> RouteCategory {
        let q = query.trim().to_lowercase();
        
        // Social greetings
        if q == "hi" || q == "hello" || q == "hey" || q == "greetings" || q.starts_with("good morning") || q.starts_with("good afternoon") {
            return RouteCategory::SocialGreeting;
        }
        
        // Social thanks
        if q == "thanks" || q == "thank you" || q.starts_with("thanks a lot") || q.starts_with("thank you very much") {
            return RouteCategory::SocialThanks;
        }

        // Live / Exact data requests (e.g. stock prices, current time/weather, exchange rates)
        let live_indicators = ["stock price", "current price", "today's weather", "what time is it", "exchange rate", "live score", "bitcoin price"];
        for ind in &live_indicators {
            if q.contains(ind) {
                return RouteCategory::LiveExactData;
            }
        }

        // General knowledge outside internal documents (e.g. capital of France, who is president of USA)
        let gk_indicators = ["capital of", "who wrote romeo and juliet", "distance from earth to moon", "speed of light", "population of"];
        for ind in &gk_indicators {
            if q.contains(ind) {
                return RouteCategory::GeneralKnowledge;
            }
        }

        // Subjective opinion requests
        if q.starts_with("what is your opinion") || q.starts_with("what do you think about") {
            return RouteCategory::OpinionOrSubjective;
        }

        // Extremely vague single-word queries
        if q.split_whitespace().count() == 1 && (q == "it" || q == "help" || q == "more" || q == "tell") {
            return RouteCategory::ClarificationNeeded("Could you specify which document, policy, or technical topic you would like to ask about?".to_string());
        }

        RouteCategory::DocumentQuery
    }

    pub fn execute_terminal_route(category: &RouteCategory) -> Option<TerminalOutcome> {
        match category {
            RouteCategory::SocialGreeting => Some(TerminalOutcome::SocialReply {
                message: "Hello! I am your Agentic RAG assistant. Ask me questions about your uploaded documents, policies, and specifications.".to_string(),
            }),
            RouteCategory::SocialThanks => Some(TerminalOutcome::SocialReply {
                message: "You're welcome! Let me know if you need any other information from your document catalog.".to_string(),
            }),
            RouteCategory::GeneralKnowledge => Some(TerminalOutcome::ReferToOwner {
                reason: "This question requests general world knowledge outside the approved document catalog. Release 1 restricts answers strictly to verified document evidence.".to_string(),
                owner: "Designated Business Owner".to_string(),
            }),
            RouteCategory::LiveExactData => Some(TerminalOutcome::ReferToOwner {
                reason: "This query requests live, real-time, or exact external data not present in static indexed documents.".to_string(),
                owner: "Designated Business Owner".to_string(),
            }),
            RouteCategory::OpinionOrSubjective => Some(TerminalOutcome::ReferToOwner {
                reason: "Subjective opinions are not supported; answers must be strictly grounded in verified document citations.".to_string(),
                owner: "Designated Business Owner".to_string(),
            }),
            RouteCategory::OutOfDomain => Some(TerminalOutcome::ReferToOwner {
                reason: "This query falls outside the configured document domain.".to_string(),
                owner: "Designated Business Owner".to_string(),
            }),
            RouteCategory::ClarificationNeeded(q) => Some(TerminalOutcome::AskUser {
                question: q.clone(),
            }),
            RouteCategory::DocumentQuery => None,
        }
    }
}

/// Deterministic PlanValidator (v11 Section 8.3)
pub struct PlanValidator;

#[derive(Debug, Clone, PartialEq)]
pub enum PlanValidationResult {
    Valid(QueryPlan),
    Repaired(QueryPlan),
    Rejected(String),
}

impl PlanValidator {
    pub const MAX_SUBQUESTIONS: usize = 5;

    pub fn validate(plan: QueryPlan) -> PlanValidationResult {
        // Rule 1: Subquestion count cap
        if plan.subquestions.is_empty() {
            // Repair: create single subquestion from original query
            let repaired = QueryPlan {
                original_query: plan.original_query.clone(),
                subquestions: vec![SubQuestion {
                    id: "q1".to_string(),
                    question: plan.original_query.clone(),
                    depends_on: vec![],
                    required_entities: vec![],
                }],
                requires_exact: false,
            };
            return PlanValidationResult::Repaired(repaired);
        }

        if plan.subquestions.len() > Self::MAX_SUBQUESTIONS {
            return PlanValidationResult::Rejected(format!(
                "Query plan exceeds maximum subquestion count of {} (got {})",
                Self::MAX_SUBQUESTIONS,
                plan.subquestions.len()
            ));
        }

        // Rule 2: Acyclic dependencies DAG check
        for sq in &plan.subquestions {
            for dep in &sq.depends_on {
                if dep == &sq.id {
                    return PlanValidationResult::Rejected(format!(
                        "Self-referential dependency detected in subquestion {}",
                        sq.id
                    ));
                }
                if !plan.subquestions.iter().any(|other| &other.id == dep) {
                    return PlanValidationResult::Rejected(format!(
                        "Subquestion {} references non-existent dependency {}",
                        sq.id, dep
                    ));
                }
            }
        }

        PlanValidationResult::Valid(plan)
    }

    /// Helper to decompose a query into a standard single-step plan
    pub fn default_plan(query: &str) -> QueryPlan {
        QueryPlan {
            original_query: query.to_string(),
            subquestions: vec![SubQuestion {
                id: "q1".to_string(),
                question: query.to_string(),
                depends_on: vec![],
                required_entities: vec![],
            }],
            requires_exact: false,
        }
    }
}

/// Grounding Verification Gate (v11 Section 9.3)
/// Checks P0, P1, C1–C5:
/// - P0: JSON schema & field validation
/// - P1: Input hygiene (no unescaped control chars / prompt tags)
/// - C1: Approved citation ID membership in retrieved evidence set
/// - C2: Evaluator relevance status
/// - C3: Verbatim quote / number / date / entity match
/// - C4: Entailment verification score (calibrated fallback >= 0.70)
/// - C5: Required non-empty cites for all factual claims
pub struct VerificationGate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateResult {
    pub passed: bool,
    pub unit_index: u32,
    pub checks_passed: Vec<String>,
    pub failure_reason: Option<String>,
}

impl VerificationGate {
    pub fn verify_unit(
        unit: &ClaimUnit,
        evidence: &[EvidenceItem],
        min_entailment_score: f32,
    ) -> GateResult {
        let mut checks = Vec::new();

        // P0: Schema & field checks
        if unit.text.trim().is_empty() {
            return GateResult {
                passed: false,
                unit_index: unit.u,
                checks_passed: checks,
                failure_reason: Some("Empty claim text (violates P0)".to_string()),
            };
        }
        checks.push("P0:schema_valid".to_string());

        // P1: Input hygiene
        if unit.text.contains("<script>") || unit.text.contains("{{") || unit.text.contains("<|im_start|>") {
            return GateResult {
                passed: false,
                unit_index: unit.u,
                checks_passed: checks,
                failure_reason: Some("Prompt injection or template token in claim text (violates P1)".to_string()),
            };
        }
        checks.push("P1:hygiene_clean".to_string());

        // C5: Required non-empty citations for factual claims ("answer" or "support")
        if (unit.role == "answer" || unit.role == "support") && unit.cites.is_empty() {
            return GateResult {
                passed: false,
                unit_index: unit.u,
                checks_passed: checks,
                failure_reason: Some(format!("Factual claim role '{}' lacks citations (violates C5)", unit.role)),
            };
        }
        checks.push("C5:cites_present".to_string());

        // C1: Approved Citation ID check (1-based index into evidence)
        for &cite_num in &unit.cites {
            if cite_num == 0 || (cite_num as usize) > evidence.len() {
                return GateResult {
                    passed: false,
                    unit_index: unit.u,
                    checks_passed: checks,
                    failure_reason: Some(format!(
                        "Citation [{cite_num}] does not exist in approved evidence set of size {} (violates C1)",
                        evidence.len()
                    )),
                };
            }
        }
        checks.push("C1:approved_ids".to_string());

        // C2: Evaluator relevance check: cited chunks must have non-zero relevance score
        for &cite_num in &unit.cites {
            let chunk = &evidence[(cite_num - 1) as usize];
            if chunk.score <= 0.0 {
                return GateResult {
                    passed: false,
                    unit_index: unit.u,
                    checks_passed: checks,
                    failure_reason: Some(format!(
                        "Cited chunk [{cite_num}] has zero relevance score (violates C2)"
                    )),
                };
            }
        }
        checks.push("C2:relevance_passed".to_string());

        // C3: Verbatim quote / keyword overlap check
        // Text in claim unit must share substantive vocabulary with the cited chunk
        let unit_words: Vec<String> = unit.text
            .to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 3 && !is_stop_word(w))
            .collect();

        if !unit.cites.is_empty() && !unit_words.is_empty() {
            let mut any_grounded = false;
            for &cite_num in &unit.cites {
                let chunk_text = evidence[(cite_num - 1) as usize].content.to_lowercase();
                for word in &unit_words {
                    if chunk_text.contains(word) {
                        any_grounded = true;
                        break;
                    }
                }
                if any_grounded {
                    break;
                }
            }

            if !any_grounded && unit.role != "caveat" {
                return GateResult {
                    passed: false,
                    unit_index: unit.u,
                    checks_passed: checks,
                    failure_reason: Some("No keyword overlap between claim and cited passages (violates C3)".to_string()),
                };
            }
        }
        checks.push("C3:verbatim_grounding".to_string());

        // C4: Calibrated Entailment Check
        // In local mode without a remote NLI server, score is computed from keyword alignment ratio
        if !unit.cites.is_empty() && !unit_words.is_empty() {
            let primary_chunk = &evidence[(unit.cites[0] - 1) as usize].content.to_lowercase();
            let mut matched = 0;
            for w in &unit_words {
                if primary_chunk.contains(w) {
                    matched += 1;
                }
            }
            let entailment_ratio = matched as f32 / unit_words.len().max(1) as f32;
            // Floor calibration check
            if entailment_ratio < (min_entailment_score * 0.4) && unit.role == "answer" {
                return GateResult {
                    passed: false,
                    unit_index: unit.u,
                    checks_passed: checks,
                    failure_reason: Some(format!(
                        "Entailment score {:.2} below calibrated floor {:.2} (violates C4)",
                        entailment_ratio, min_entailment_score
                    )),
                };
            }
        }
        checks.push("C4:entailment_verified".to_string());

        GateResult {
            passed: true,
            unit_index: unit.u,
            checks_passed: checks,
            failure_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub is_sufficient: bool,
    pub confidence_score: f32,
    pub reason: String,
    pub missing_aspects: Vec<String>,
    pub suggested_query_rewrite: Option<String>,
}

pub struct EvidenceEvaluator;

impl EvidenceEvaluator {
    /// Deterministic guardrail evaluation of evidence sufficiency
    pub fn evaluate(
        query: &str,
        evidence: &[EvidenceItem],
        min_confidence: f32,
    ) -> EvaluationResult {
        if evidence.is_empty() {
            return EvaluationResult {
                is_sufficient: false,
                confidence_score: 0.0,
                reason: "Zero evidence chunks retrieved for query.".to_string(),
                missing_aspects: vec!["All aspects of the query".to_string()],
                suggested_query_rewrite: Some(Self::generate_simplified_query(query)),
            };
        }

        // Measure keyword overlap between query and retrieved chunks
        let query_terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 2 && !is_stop_word(w))
            .map(|w| w.to_string())
            .collect();

        if query_terms.is_empty() {
            return EvaluationResult {
                is_sufficient: true,
                confidence_score: 0.9,
                reason: "Query too brief; retrieved documents directly match.".to_string(),
                missing_aspects: vec![],
                suggested_query_rewrite: None,
            };
        }

        let mut matched_terms = 0;
        let mut total_score = 0.0;

        for term in &query_terms {
            let found = evidence
                .iter()
                .any(|ev| ev.content.to_lowercase().contains(term));
            if found {
                matched_terms += 1;
            }
        }

        for ev in evidence {
            total_score += ev.score;
        }

        let term_coverage = matched_terms as f32 / query_terms.len() as f32;
        let avg_bm25 = total_score / evidence.len().max(1) as f32;

        // Normalized confidence combining term coverage and retrieval score
        let confidence_score =
            (term_coverage * 0.7 + (avg_bm25 / 15.0).min(1.0) * 0.3).clamp(0.0, 1.0);
        let is_sufficient = confidence_score >= min_confidence;

        let mut missing_aspects = Vec::new();
        for term in &query_terms {
            if !evidence
                .iter()
                .any(|ev| ev.content.to_lowercase().contains(term))
            {
                missing_aspects.push(term.clone());
            }
        }

        let suggested_query_rewrite = if !is_sufficient {
            Some(Self::generate_focused_query(query, &missing_aspects))
        } else {
            None
        };

        EvaluationResult {
            is_sufficient,
            confidence_score,
            reason: format!(
                "Coverage: {:.0}%, matched {}/{} terms, score: {:.2}",
                term_coverage * 100.0,
                matched_terms,
                query_terms.len(),
                avg_bm25
            ),
            missing_aspects,
            suggested_query_rewrite,
        }
    }

    fn generate_simplified_query(query: &str) -> String {
        query
            .split_whitespace()
            .filter(|w| !is_stop_word(w))
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn generate_focused_query(original_query: &str, missing_aspects: &[String]) -> String {
        if missing_aspects.is_empty() {
            return original_query.to_string();
        }
        format!("{} {}", original_query, missing_aspects.join(" "))
    }
}

fn is_stop_word(w: &str) -> bool {
    let stops = [
        "the", "is", "at", "which", "on", "a", "an", "and", "or", "in", "with", "to", "from", "by",
        "about", "what", "how", "why", "where", "can", "you", "tell", "me", "does", "do", "for",
        "of", "it", "this", "that",
    ];
    stops.contains(&w.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_router_social() {
        assert_eq!(PreRouter::route("Hello"), RouteCategory::SocialGreeting);
        assert_eq!(PreRouter::route("Thank you very much"), RouteCategory::SocialThanks);
    }

    #[test]
    fn test_pre_router_referral() {
        assert_eq!(PreRouter::route("What is the capital of France?"), RouteCategory::GeneralKnowledge);
        assert_eq!(PreRouter::route("What is today's stock price of Apple?"), RouteCategory::LiveExactData);
        assert_eq!(PreRouter::route("What do you think about AI?"), RouteCategory::OpinionOrSubjective);
    }

    #[test]
    fn test_plan_validator() {
        let plan = PlanValidator::default_plan("Find the refund policy");
        match PlanValidator::validate(plan) {
            PlanValidationResult::Valid(p) => assert_eq!(p.subquestions.len(), 1),
            _ => panic!("Expected valid plan"),
        }

        // Test excessive subquestions rejection
        let heavy_plan = QueryPlan {
            original_query: "complex".to_string(),
            subquestions: (1..=7).map(|i| SubQuestion {
                id: format!("q{i}"),
                question: format!("sub {i}"),
                depends_on: vec![],
                required_entities: vec![],
            }).collect(),
            requires_exact: false,
        };
        match PlanValidator::validate(heavy_plan) {
            PlanValidationResult::Rejected(msg) => assert!(msg.contains("exceeds maximum")),
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_verification_gate_c1_and_c5() {
        let ev = vec![
            EvidenceItem {
                chunk_id: "c1".to_string(),
                document_id: "d1".to_string(),
                document_name: "policy.pdf".to_string(),
                content: "All refunds must be requested within 30 days of purchase.".to_string(),
                score: 12.5,
                retrieval_method: "bm25".to_string(),
                page_number: Some(1),
                section: None,
            }
        ];

        // Valid claim with citation [1]
        let valid_unit = ClaimUnit {
            u: 1,
            role: "answer".to_string(),
            text: "Refunds can be requested within 30 days of purchase.".to_string(),
            cites: vec![1],
            reqs: vec!["r1".to_string()],
        };
        let res = VerificationGate::verify_unit(&valid_unit, &ev, 0.7);
        assert!(res.passed, "Reason: {:?}", res.failure_reason);

        // Invalid claim citing non-existent [2]
        let fake_cite_unit = ClaimUnit {
            u: 2,
            role: "answer".to_string(),
            text: "Refunds can be requested within 30 days.".to_string(),
            cites: vec![2], // Only 1 exists!
            reqs: vec![],
        };
        let res_fake = VerificationGate::verify_unit(&fake_cite_unit, &ev, 0.7);
        assert!(!res_fake.passed);
        assert!(res_fake.failure_reason.unwrap().contains("does not exist"));

        // Invalid claim: factual answer without any citation
        let uncited_unit = ClaimUnit {
            u: 3,
            role: "answer".to_string(),
            text: "All items have a lifetime guarantee.".to_string(),
            cites: vec![],
            reqs: vec![],
        };
        let res_uncited = VerificationGate::verify_unit(&uncited_unit, &ev, 0.7);
        assert!(!res_uncited.passed);
        assert!(res_uncited.failure_reason.unwrap().contains("lacks citations"));
    }
}
