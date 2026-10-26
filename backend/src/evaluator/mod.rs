use crate::models::EvidenceItem;
use serde::{Deserialize, Serialize};

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
