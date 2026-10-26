# Implementation Status: Agentic RAG System (v11 Migration)

Last Updated: 2026-10-26 (Execution Checkpoint)
Target Architecture: **Agentic RAG System Complete Design Document v11**

---

## 1. Executive Summary

This document tracks the incremental migration of the existing Rust + Astro Agentic RAG repository to Design v11 Release 1.
The existing repository maintains a modular Rust backend (`backend/`) and Astro frontend (`frontend/`).
All migrations preserve existing working capabilities, local data, and zero-Docker native execution.

---

## 2. Release Gates Status (G0–G10)

| Gate | Title | Target / Acceptance Criteria | Status | Evidence / Notes |
|---|---|---|---|---|
| **G0** | Decisions and Spikes | Formal acceptance of v11 spec; identification of Open Items (OI-1 to OI-3); local toolchain & runtime verified | **PASS** | Design v11 spec in workspace (`docs/DESIGN_v11.md`); MSVC toolchain pinned; native Windows runtime established. OI-1/2/3 documented as unresolved (synthetic data used). |
| **G1** | Baseline & Feature Evaluation | Functional baseline tests for parsing, chunking, retrieval, and evaluation pass; regression suite active | **IN PROGRESS** | Integration tests in `backend/tests/integration_tests.rs` pass (SQLite + Tantivy + PDF parser + Evaluator). Updating for v11 PlanValidator and Grounding Gate. |
| **G2** | Isolation & Access | Zero cross-tenant or cross-principal row/object leaks; live ACL enforcement; RLS fail-closed | **PLANNED** | TenantTx isolation abstractions and RLS migration scripts in progress. |
| **G3** | Grounding Bound | 0% unverified claim units emitted in strict/gated mode; verbatim quote matches; P0–C5 validation | **IN PROGRESS** | Claim unit schema, JSON Lines streaming parser, and verification gate checks P0–C5 specified and being implemented. |
| **G4** | Streaming & Replay | SSE `/v1/chat/stream` emits monotonic event IDs; `Last-Event-ID` replayed from `run_events`; 10s grace | **IN PROGRESS** | Unified SSE contract and event persistence layer being added. |
| **G5** | Load Profile | 10 concurrent streams, 15-stream burst, 50 distinct users without deadlock or token corruption | **PLANNED** | Load test script against native Axum server scheduled after API consolidation. |
| **G6** | Chaos & Dependency Failures | Graceful degradation on Redis absence; tool health gate detects search failures without losing state | **IN PROGRESS** | Tool health gate and fallback routes designed to avoid panics. |
| **G7** | Security Review | OIDC JWT verification; no prompt/secret leakage; strict tool registry (document-only) | **IN PROGRESS** | Compile-time tool restriction implemented; no forbidden web/SQL/CRM/MCP tools. |
| **G8** | Restore & Deletion | Document deletion cascades through chunks, search index, and citations; safe tombstoning | **PASS (Baseline)** | `delete_document` removes chunks and Tantivy index terms. Expanding for v11 deletion ledger. |
| **G9** | Cost & Token Controls | Max 60,000 token budget hard stop; max 5 attempts; 40s wall-clock limit; budget reservation | **IN PROGRESS** | Budget reservation and state machine loop bounds integrated into `AgentOrchestrator`. |
| **G10** | Data Handling & Sign-off | Production data blocked until named owners & hosting terms approved; synthetic data active | **PASS** | Enforced: production data integrations remain blocked per v11 D1/D10 until Open Items are formally closed. |

---

## 3. High-Risk Open Items (from v11 Section 0.3)

| ID | Open Item Description | Status | Current Mitigation |
|---|---|---|---|
| **OI-1** | Named Business Owner, Technical Owner, and Security Sign-off Deputies | Unresolved | Development uses system role `SystemAdmin`; document questions route to internal index; all general/live queries route to `refer_to_owner("Designated Business Owner")`. |
| **OI-2** | Nemotron 3 Super hosting decision & endpoint infrastructure | Unresolved | Local deterministic grounded fallback + synthetic mocked responses active; OpenRouter bridge supported for testing. |
| **OI-3** | Approved service region, privacy policy, and external data terms | Unresolved | Production data egress is disabled. Synthetic test documents only. |

---

## 4. Phase-by-Phase Implementation Progress

- [x] **Phase 0: Repository Audit & Gap Analysis**
  - Inspected `backend/Cargo.toml`, `backend/src/*`, `frontend/*`, and existing database/index structures.
  - Published `docs/MODERNIZATION_GAP_MATRIX.md`.
  - Identified toolchain fixes (Windows MSVC) and schema migration mechanisms.
- [x] **Phase 1: Stabilize Baseline & Reproduce Known Issues**
  - Added safe `schema_migrations` tracking in SQLite to resolve the missing `status` column error.
  - Resolved Tantivy 0.27 `TopDocs: Collector` API changes.
  - Verified lopdf PDF parser against real-world test PDFs.
  - Verified Astro frontend dev server (`http://localhost:4321`) and backend API (`http://127.0.0.1:8080`).
- [/] **Phase 2: Data Architecture Migration (PostgreSQL / SQLite Native Adapter)**
  - Schema definitions for tenants, documents, chunks, ACLs, conversations, messages, and run events.
  - Storage abstraction supporting native no-Docker environment.
- [/] **Phase 3: Identity & Access Isolation**
  - Tenant context abstractions (`TenantTx`).
  - OIDC claims verification logic.
- [/] **Phase 4: Two-Binary Architecture (API & Ingestion Worker)**
  - `backend/src/bin/api.rs`: Axum web & SSE server.
  - `backend/src/bin/worker.rs`: Durable ingestion queue worker.
  - DOCX, PPTX, and scanned PDF validation.
- [/] **Phase 5 & 6: Pre-Router & PlanValidator**
  - Deterministic pre-router for greetings/thanks (`social_reply`) and strict domain referrals (`refer_to_owner`).
  - `PlanValidator` with subquestions `q1..qN`, acyclic DAG validation, entity/date constraints.
- [/] **Phase 7: Bounded Agentic RAG Controller**
  - Explicit state machine: START -> ROUTE -> PLAN -> SELECT_TOOLS -> RESERVE_BUDGET -> EXECUTE_TOOLS -> TOOL_HEALTH_GATE -> NORMALISE -> ACL_RECHECK -> EVALUATE -> BUILD_CONTEXT -> GENERATE -> VERIFY_GATE -> PERSIST.
  - Hard limits: max 5 attempts, max 8 tool calls, 40s timeout.
- [/] **Phase 8: Evidence Evaluator & Adaptive Retries**
  - Evaluator checks subquestion coverage and verbatim substring quotes.
  - Stop early on sufficiency; query refinement on partial evidence.
- [/] **Phase 9: Claim-Unit Generation & Verification Gate**
  - Canonical JSON Lines claim units: `{"u": N, "role": "...", "text": "...", "cites": [...], "reqs": [...]}`.
  - Verification Gate checks P0–C5 before token emission.
- [/] **Phase 10: v11 API Contract & SSE Reconnect**
  - `/v1/chat/stream`, `/v1/search`, `/v1/documents`, `/v1/health`.
  - Monotonic SSE event IDs and `Last-Event-ID` replay.
- [/] **Phase 11: Astro Frontend Alignment**
  - Preserved Astro visual styling and components.
  - Updated SSE parser to consume v11 event stream and render claim-level citations.
- [ ] **Phase 12: Comprehensive Verification & End-to-End Testing**
  - Run test suite covering all gates and edge cases.
