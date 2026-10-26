# Agentic RAG System: Complete Design Document

*Version 11 · Rust backend · Conceptual reference: the RAG vs Agentic RAG diagram (User → LLM App → Agent Orchestrator → Tools → Evidence → Evaluator → Context Builder → LLM → Response)*

## Document control

| Item | Value |
| --- | --- |
| Version | v11 (supersedes v1 to v10, including the externally supplied v10). Complete: §1 to §28 plus Appendices A to F are all present |
| Status | **Not complete: three high-risk open items remain (§0.3: OI-1 owner names, OI-2 model hosting, OI-3 region and provider terms).** Stage 0 can start on synthetic data now. Every decision has a rule, owner, test, pass limit and fallback (§0). **Nothing is deployable until the built system passes gates G0 to G10 (§28.10) and the load profile in §28.11.** No design review can guarantee zero defects |
| Backend | Rust: modular monolith with an API binary and an ingestion worker binary |
| Basis | v8 text, the owner's answers to the completion checklist (26 Oct 2026), the v10 design and two reviews of it, the v7 strategic plan, and all earlier reviews (Appendices A to F) |

**Honesty note.** A design document cannot make software deployable; only a built system that passes the gates in §28.10 is. The Rust and SQL samples in this document are contracts written to be compiled and tested; they could not be compiled in the authoring environment (no access to the crate registry), so Spike S9 compiles and tests every sample before the dependent code is merged. Crate capabilities, pgvector features, model accuracy and every latency, sizing or effort figure here are targets or starting assumptions, not measured facts. Each is tied to a Stage 0 spike with a recorded default and fallback (§24.4) or to a release gate (§19, §28). Items marked **[verify]** must be confirmed against the current crate or product version before the dependent code is merged.

**What changed in v11**

| Area | Change |
| --- | --- |
| Self-contained | One document again: v10's decision register is merged into v8's detailed contracts as §0. Nothing refers to an older version. Spike IDs (S) and decision IDs (D) no longer collide, and gates G0 to G10 are defined in full with numbers (§28.10) |
| Owner decisions | Scope (internal IT team; PDF, Word, PowerPoint; English), routing (documents only; everything else referred to the Business Owner), OIDC access, models (nomic-embed-text-v1.5; Nemotron 3 Super), stream and retry rules, workload (10 concurrent, 50 random users), limits (1% production, 2% staging), pinned Rust 1.99.1 (§0) |
| Routing | `direct_answer`, the `light` context mode and model-knowledge answers are removed. New outcome `refer_to_owner`. No web, SQL, API, CRM or MCP tools in release 1 (§3.4, §4, §7, §8) |
| Models | 768-dimension embeddings with task prefixes; LLM listwise reranking; the answer model also evaluates, so judge independence is limited and the `high_risk` and `regulated` tiers are off in release 1 (§12.6, §13.5, §21) |
| Streaming | Retry up to 3 times only before any text is shown; up to 3 reconnects with replay from Postgres; 10 s grace before cancelling (§13.4, §15.2, §15.3) |
| Access | Group, role and tenant-wide grants evaluated through one `acl_self` policy over the caller's principal set; pinned versions removed; full table, view and storage inventory with a test for every item (§16.3, §16.3a) |
| Data | Retention and deletion limits for every data type; deletion drill covers restored backups (§16.8) |
| Load and gates | New numeric workload with open-arrival rates; environment limits; every gate named and measurable (§28.10, §28.11) |
| Models checked | nomic-embed-text-v1.5 is English-only (768 dimensions, 8,192 tokens); Nemotron 3 Super needs about 8 H100 GPUs to self-host and its NVIDIA hosted API is a trial service, which creates OI-2 (§0.3) |

**What changed in v8**

| Area | Change |
| --- | --- |
| Membership freshness | Groups and roles come from an authoritative membership lookup with a bounded maximum age and change events, not from JWT claims. Defined behaviour when the membership service is down (§16.2) |
| RLS threat boundary | States what RLS settings protect against and what they do not, separates the SQL tool's database role, and names the stronger fallback if a compromised application role is in scope (§16.3) |
| Requirement coverage | New PlanValidator stage covering conditions, comparisons, constraints and requested output, not only entities and dates; incomplete coverage yields a clarification or a labelled partial answer, never a complete-looking one (§4, §5 rule F) |
| Parser invariants | Code enforces `u=1`, `role=answer` first, consecutive `u`, and no later `answer`. JSON Lines is the only format in release 1; the delimiter fallback is removed (§13.0, §24.4) |
| Lease semantics | All lease times use `clock_timestamp()`; bounded lock, statement and transaction timeouts; the takeover-versus-slow-write behaviour is stated and tested (§9.5) |
| Reservation ownership | Reserve-and-spawn is one non-awaiting step; dropping a reservation never refunds; retry reservations are tied to actual starts (§6.3) |
| Format consistency | One end-to-end format test from provider stream to client rendering (§14.4) |
| Delivery plan | Phase 0 boundary record, hardening exit criteria before any agent feature, agentic features added one at a time behind flags, and a small starting workspace (§24, §20.1) |

**What changed in v7**

| Area | Change |
| --- | --- |
| Authorisation | The `chunks` RLS policy now evaluates the **live ACL, deletion and current-version rules itself**, so a revoked or deleted row never reaches application code. `allowed_principals` is demoted to an index prefilter. The policy no longer depends on a `SECURITY DEFINER` function; `acl_entries` carries a self-grants-only policy so the check cannot enumerate other principals (§12.2, §16.3) |
| Conversation lock | The fence and lease live in **Postgres** (one monotonic column advanced in the statement that grants the lease). Writes must match the active owner, fence and unexpired lease. Redis is no longer in the correctness path (§9.5) |
| Budget and retry | The wave reservation is passed into the call; only retries reserve. `min_useful_time` and `RetryPolicy.max_transport_retries` are defined. The backoff sleep is cancellation-aware, and the deadline and cancellation are checked before a retry reserves (§6.3, §8.1) |
| Generation and gate | One canonical format: JSON-line claim units with structured `cites`. The gate parses and schema-validates each unit; it does not split sentences or read inline markers (§13.0, §13.2, §14, §15.2, §22 P10) |
| Grounding bypass | The model cannot emit `meta`. Every model unit is treated as factual and needs valid citations. The only uncited text is system-generated from code templates (§13.0) |
| Load target | A numeric, reproducible load profile (concurrent streams, request mix, tenants, corpus, duration, pass criteria) defines gate G5 and the sizing check (§28.11) |
| Spikes | S9 (compile and test the Rust contracts), S10 (live-ACL policy performance), S11 (fencing under failover), S12 (gate parser fuzzing) (§24.2, §24.4) |
| Claims | Status wording corrected; Appendix D records the validation of v6 (§27.4) |

**What changed in v6**

| Area | Change |
| --- | --- |
| Evaluator failure | **Fails closed.** Invalid evaluator output is never replaced by a rerank-score answer. After repair and one re-run, the outcome is `fail_gracefully(evaluation_unavailable)` (§5, §6.4) |
| Authorisation | RLS now enforces **tenant and effective principal ACL** with fail-closed helper functions; tests cover user, group and role boundaries, not only tenants (§16.3, §12.2) |
| Cache keys | Keyed by the full effective authorisation context plus an authorisation epoch; ACLs are always rechecked before cached results are exposed (§16.4) |
| Router | Corrected example; a deterministic `PlanValidator`; every required sub-question must map to an authorised, compatible source or be reported as blocked, never dropped (§4) |
| Router fallback | Conservative fallback that cannot answer live or exact questions from documents; new answer rule (E) requires system-of-record or fresh evidence for exact and live sub-questions (§4, §5) |
| "Just guess" | Removed as an evidence bypass. An assumption never satisfies a required evidence rule (§11.2) |
| Grounding screen | Renamed and bounded: it is a screen, not proof. Self-contained claim units, an answer-first rule, a coherence rule, risk tiers with false-pass targets (§13.0, §13.5) |
| Evaluation power | Sample sizes per slice, minimum detectable effects, multiple-comparison control, and locked holdouts (§19.8) |
| Rust | Orphaned blocking work bounds and cancellable parsing (§20.5); retry budget reservation (§6.3); `upstream_failure` outcome (§3.4, §13.4); idempotency and lock fencing (§9.5); SSE middleware contract (§15.3, §21); exact RLS test list (§16.3) |
| Decision defaults | Each spike has a default, a fallback and a switch criterion (§24.4) |
| Deployment | Environments, topology, sizing method, release, SLOs, DR, runbooks and measurable go-live gates (§28) |

**What changed in v5**

| Area | Change |
| --- | --- |
| Completeness | v4 stopped at §8 while referring to §9 to §27. All sections are now written (§9 to §27) |
| Answer decision | Replaced "one relevant item above a floor" with a per-sub-question support rule that requires verbatim-quote evidence checked by code (§5) |
| Citations | A full citation contract: stable anchors, claim-to-evidence checks, behaviour on missing or invalid citations, and tests (§14) |
| Streaming | Sentence-gated verified streaming. Nothing unverified is emitted in `strict` or `gated` mode. Latency effect stated honestly (§13) |
| Security | Enforceable controls: RLS transaction pattern for pooled connections, ACL defence in depth, cache isolation, credential boundaries, injection defences (§16) |
| Tool contract | Two-layer error model (provider errors vs internal faults), partial results, freshness, pagination, authorisation, and empty-result semantics (§8) |
| Retrieval and ingestion | Concrete identifiers, ACL model, versioning and deletion, hybrid fusion, reranker calibration, dead-letter handling (§12) |
| Budgets | Per-sub-question coverage, a multi-hop plan, atomic budget reservation, and a retry-action matrix by failure type (§6) |
| Memory | Storage, scope, expiry, provenance and revalidation rules; a resumable clarification lifecycle (§10, §11) |
| Rust | A challenges register with mitigations and verifying spikes (Appendix B), async/cancellation/error/blocking rules (§20) |
| Pipelines | A catalogue of separate pipelines for every technical area (§22) |

## 0. Release 1 decisions, ownership and open items

This section is the binding record for release 1. Where any later section is older or looser, this section wins, and the later sections have been aligned to it. Every decision has **one rule, one owner, one test, one pass limit and one fallback**. Values marked *proposed* are defaults the Business Owner accepted on 26 October 2026 by replying to the completion checklist; they can be changed only through the change rule in D1.

### 0.1 Decisions

| ID | Rule | Owner | Test | Pass limit | Fallback |
| --- | --- | --- | --- | --- | --- |
| **D1 Owners** | Roles: **Business Owner** (the customer's owner of the manufacturing organisation; a non-technical person; approves scope, risk, spend and data handling; is the person users are referred to), **Technical Lead**, **Security Owner**, **Data and Records Owner**, **AI Evaluation Owner**, **Platform and SRE Owner**, **Privacy and Legal Owner**. Each role has one named person and one named deputy. **Change rule:** the Business Owner approves any change to D2, D3, D6 (limits), D7 and D9. The Security Owner and Technical Lead jointly approve any change to D4 and D8. The Business Owner cannot waive a security or isolation test. Everything put to the Business Owner is written in plain language with a one-line "what this means" | Business Owner | The Stage 0 checklist lists a person and a deputy for every role, and the decision log (§0.2) has an entry for every decision | 100% of roles have a named person and deputy before any production data is used | Stage 0 and staging run on synthetic data only. No production data, no go-live |
| **D2 Scope** | Users: the customer's internal IT team (reference size 50 users). Documents: **PDF** (text-extractable), **Word** (`.docx`; legacy `.doc` is converted) and **PowerPoint** (`.pptx`; legacy `.ppt` is converted). Language: **English only**, because the chosen embedding model is English-only (D5). Out of scope for release 1: scanned or image-only files, spreadsheets, audio, video. The system **refuses or refers** any question outside the domain expertise of the uploaded documents | Business Owner | Scope suite: every accepted format and a rejected-format set; at least 100 out-of-domain questions | Accepted formats ingest and every rejected file gets a stated reason. At least 95% of out-of-domain questions end in `refer_to_owner(out_of_scope)`. No out-of-domain question gets a factual answer | Pilot with named test users and synthetic documents only |
| **D3 Routing** | One rule everywhere (§3.4, §4, §7, §13, §15): **document questions are answered from documents with citations. Everything else is referred to the Business Owner** with a fixed template: general knowledge, opinions, anything outside the documents' domain, and any exact or live data. The system never answers from the model's own knowledge. No web, database, API, CRM or MCP tool exists in release 1. A greeting or thanks gets a fixed template reply with no model call | Business Owner and Technical Lead | Routing matrix of at least 50 cases per class (document, general, live, exact, out-of-domain, greeting, injection) and a registry check that no excluded tool is compiled in | 100% of deterministic cases route as specified. At least 95% of model-assisted document-versus-other cases route correctly. 0 answers from model knowledge | Ask one clarifying question, then `refer_to_owner` |
| **D4 Access** | **OIDC** is the only identity source. The token is verified (signature, issuer, audience, expiry). Tenant and user come from the verified token. Groups and roles come from the OIDC provider and must be no more than **5 minutes old**: either the claims in a token issued within the last 300 s (only if the provider re-evaluates membership at every issuance; verified in S16), or a call to the provider's `userinfo` endpoint. Older context fails closed for group and role grants. A caller's effective principal set is `user`, its `group`s, its `role`s and `tenant-wide`. Document access is decided by **live ACL rows in Postgres, checked by RLS** (§16.3). A caller can see only the ACL rows whose principal is in its own set, so group, role and tenant-wide grants work and nobody else's grants are visible. Every table, view and storage path is listed with its access test in §16.3a | Security Owner | The ACL suite (§16.3 tests 1 to 12) run as production-like roles against real Postgres, plus the S16 membership-freshness test with the customer's provider | 0 unauthorised rows or objects in every test. Every item in §16.3a has at least one passing test (checked by CI) | Block release. If the shared-role threat is unacceptable, use per-tenant roles or databases (§16.3) |
| **D5 Models** | **Embedding:** `nomic-ai/nomic-embed-text-v1.5`, open weights (Apache-2.0), 768 dimensions, 8,192-token input, English only, required prefixes `search_document: ` and `search_query: `, run on our own hardware so no document text leaves our network. **Answering, reranking and evidence evaluation:** NVIDIA **Nemotron 3 Super** (`nvidia/nemotron-3-super-120b-a12b`, 120 billion parameters, 12 billion active, NVIDIA Nemotron Open Model License). Exact model revisions are recorded in `models.lock`; a model change goes through the D7 gates. **Hosting and provider for Nemotron 3 Super is not yet decided (open item OI-2, high risk)** | Technical Lead, with the Business Owner for cost and the Privacy and Legal Owner for data terms | Spikes S14 (quality, language, speed, cost, data handling) and S15 (LLM reranking) | Embedding Recall@10 at least 0.90 on the retrieval slice of the golden set. Reranking improves nDCG@10 by at least 0.03 over no reranking, or it is switched off. Answer quality, speed and cost meet D6 and D7. Data terms approved (D9) | Embeddings: another open model behind the `Embedder` trait. Reranking: a local cross-encoder. Answering: a second host of the same model, or a different approved model |
| **D6 Output and stream** | **Format:** JSON Lines claim units (§13.0), plain language, consulting-level: concise guidance with citations, at most 8 units and 400 words (4,000 tokens), and at most 40 words quoted verbatim from any source and at most 3 quotes per answer; detail beyond that is a pointer to the document and page. **Invalid stream data:** abort, retry the generation up to **3 times** (4 tries in total), only while **no answer text has been shown**; after 3 failed retries return `fail_gracefully(verification_failed)`. **After any answer text has been shown there is no retry**: the stream ends with a terminal `incomplete` marker. **Disconnect or slow client:** the client may reconnect up to **3 times** per run and resumes from its last event; the server waits 10 s after a drop or a 5 s slow-reader timeout before cancelling | Technical Lead and Platform and SRE Owner | Parser fuzzing (S12), end-to-end format test (§14.4), deployed-ingress stream tests (a) to (g) (§15.3) | No emitted unit violates the schema. No retry after exposure. A reconnect within the grace resumes with no lost or duplicated unit. A run with no reconnect within the grace is cancelled within 12 s | Buffered `strict` mode with non-streaming structured output; long-poll replay transport |
| **D7 Test and release limits** | **Workload:** 10 concurrent users in steady state, and 50 distinct users arriving at random (§28.11). **Residual-risk limits:** unsupported-claim pass-through and failed-run rate at most **1% in production, 2% in staging**, tracked but not gated in development. Unauthorised access is **0 in every environment**. All named gates G0 to G10 and their numbers are in §28.10 and §28.11, with nothing referred to an older design | Business Owner (limits), AI Evaluation Owner (measurement) | Gates G0 to G10 | Every gate passes in staging at the staging limits before promotion, and in production at the production limits | A failed gate blocks release or keeps the feature disabled; it is never a documentation waiver |
| **D8 Rust** | Toolchain **pinned to Rust 1.99.1** (released 15 October 2026: [release post](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)), edition 2024, `Cargo.lock` committed, CI uses `cargo --locked`. Error, cancellation, timeout, queue and database rules are in §6.3, §8.2, §9, §15.3, §20 and §28.2. **Every code sample is compiled and tested in CI before dependent code merges** (S9) | Technical Lead | CI job `contracts` builds and tests every sample with `-D warnings` | 100% of samples compile and pass their tests on the pinned toolchain | Do not merge dependent code; correct the document and the sample together |
| **D9 Data handling** | Retention and deletion limits for every data type are in §16.8 (proposed values accepted). One approved service region for API, database, object storage and telemetry (**region name not yet recorded, OI-3**). Provider terms: no training on our content, retention at most 30 days, preferably zero. Deletion is tested on active data, caches and restored backups | Privacy and Legal Owner, Data and Records Owner | Deletion drill (§16.8) and a provider-terms review | Search rows and caches purged within 60 s of deletion, objects within 24 h, backups age out in 35 days, and a restore re-applies the deletion ledger before serving. Provider terms approved | Keep the tombstone and deny access. Suspend ingestion or model calls that cannot meet the terms |
| **D10 Approval record** | Each decision and its test result is written to the decision log (§0.2). Open items are listed in §0.3. **The design is not marked complete while any high-risk open item is open** | Business Owner | The log has an entry per decision, with a test status | No missing entries; no high-risk item open at sign-off | Status stays "not complete" |

### 0.2 Decision log

| ID | Decision | Approved by | Date | Test status |
| --- | --- | --- | --- | --- |
| D1 to D10 | As in §0.1, with the answers given in the completion checklist | Business Owner (name to be recorded, OI-1) | 26 Oct 2026 | Not yet run. This document claims no test result |

### 0.3 Open items

| ID | Open item | Risk | Needed from | Default until closed |
| --- | --- | --- | --- | --- |
| OI-1 | Name and deputy for every role in D1 | **High** | Business Owner | Synthetic data only |
| OI-2 | Where and by whom Nemotron 3 Super is hosted. NVIDIA's model page lists a minimum of 8 H100-80GB GPUs for self-hosting. The NVIDIA hosted API catalogue is a trial service under trial terms and **must not receive customer data**. A managed host or a self-hosted cluster must be chosen, with its region and data terms recorded | **High** | Business Owner, Technical Lead, Privacy and Legal Owner | No production data may be sent to any model host |
| OI-3 | Name of the approved service region, and the signed provider data terms (no training, retention at most 30 days) | **High** | Privacy and Legal Owner | No production data |
| OI-4 | Corpus size and number of customer tenants. Assumed for sizing: 1 to 3 tenants, at most 2,000 documents, at most 200,000 chunks | Medium | Business Owner | The assumption in §28.3 |
| OI-5 | Meaning of "1% production, 2% staging, rest development": taken as the unsupported-claim pass-through and failed-run limits in D7. Confirm | Medium | Business Owner | As in D7 |
| OI-6 | What "permitted level of information for consulting" allows. Taken as the D6 limits (concise guidance, 40-word quotes). Confirm | Medium | Business Owner | As in D6 |
| OI-7 | "Retry 3 times" taken as 3 retries after the first failure (4 tries). Confirm | Medium | Business Owner | As in D6 |
| OI-8 | Whether the customer's OIDC provider re-evaluates group and role membership every time it issues a token. If not, `userinfo` is called per request | Medium | Security Owner | `userinfo` per request |
| OI-9 | Per-run and per-day cost caps. They depend on the hosting chosen in OI-2 | Medium | Business Owner | Token budget only (§6.1) |
| OI-10 | Approval of the isolated converter that turns legacy `.doc` and `.ppt` into `.docx` and `.pptx` (LibreOffice headless, no network) | Low | Security Owner | Legacy formats rejected until approved |

**Status: not complete.** OI-1, OI-2 and OI-3 are high-risk and open, so under D10 this design may not be marked complete. Stage 0 on synthetic data can start now. **Design completion would not prove the system is ready to deploy**: the built system must still pass gates G0 to G10 (§28.10).

## 1. Purpose, scope and principles

**Goal.** A production Agentic RAG service that answers questions over internal documents and, in later stages, live business systems, with cited and verifiable answers, honest handling of insufficient evidence, and multi-turn conversation.

**In scope.** Document ingestion, hybrid retrieval, the agentic loop (router, tool selection, evaluator, retry, context building), multi-turn memory, citations, streaming chat API, authentication and multi-tenant isolation, observability, evaluation.

**Out of scope.** Model fine-tuning, image or audio retrieval, tools that write to external systems (all tools are read-only), autonomous long-running agents.

**Principles**

1. **Baseline first.** Build and measure a good traditional RAG. Each agentic feature ships only if it beats the baseline on the golden set (§19, §24).
2. **Bounded autonomy.** The LLM proposes, code disposes. Limits, permissions and transitions are enforced in Rust, never only in prompts.
3. **Provenance everywhere.** No evidence without source, version, location and score.
4. **Nothing that fails the grounding screen reaches the user** in `strict` and `gated` modes (§13). The screen lowers the rate of unsupported claims to a measured, tier-specific bound; it does not prove truth (§13.5).
5. **Fail honestly.** Insufficient evidence produces an explicit, useful answer, never a guess.
6. **Security by construction.** Tenant and ACL enforcement lives in the data layer and in typed Rust APIs that cannot be bypassed by accident.
7. **Simple until measured.** Anything in §25 waits for evidence that it is needed.

## 2. Reference architecture

| Component | Role in the system |
| --- | --- |
| User | Sends a task and receives a grounded, cited answer. |
| LLM App | Product shell: chat UI, auth, sessions, streaming, citation display. Owns no reasoning logic. |
| Agent Orchestrator | The control loop: plans, selects tools, calls them, acts on the Evaluator's verdict, decides to retry or stop. A state machine in which an LLM makes bounded decisions. |
| Tools and Retrieval Sources | Registry of capabilities behind one interface: vector DB, documents, web search, APIs, CRM, MCP servers. |
| Vector Database | Semantic index over chunked content. |
| Documents | Source corpus (release 1: PDF, Word, PowerPoint) with metadata and originals. |
| Web Search | Fresh public information. **Not in release 1.** |
| APIs / CRM / external systems | Exact, authoritative structured data. **Not in release 1.** |
| MCP servers | Standard protocol for plugging in third-party tools. **Not in release 1.** |
| Retrieved Evidence | Normalised tool output with provenance. |
| Evaluator Agent | Separate block that judges evidence against the task. Its verdict drives the red loop ("no: rewrite query or switch source, max 5 tries"). |
| Context Builder | Turns approved evidence into the compact, cited augmented prompt. |
| LLM | Generates the answer from the supplied context only. It never answers from its own knowledge in release 1. |

**Traditional RAG:** User → Query → Retriever → Top-K chunks → Prompt Builder → LLM → Answer. One fixed pass, always retrieves, one index, raw query, no quality check.

**Agentic RAG:** User → LLM App → Orchestrator → select tools → retrieve → Evaluator → (rewrite / widen / switch source, back to Orchestrator) → Context Builder → LLM → Verification Gate → Answer.

**Why it is better for complex queries:** routing (skip retrieval or pick the right source), multi-source and multi-hop questions, self-correction of bad retrieval, honest "insufficient evidence" answers, and cost control by retrieving only when needed. **Cost:** more latency and LLM calls, which is why every loop is bounded (§6, §17) and why each agentic feature must beat the baseline (§24).

## 3. Target architecture

### 3.1 Main flow (numbers match the diagram)

```text
 User
  | (1) query                                              ^ (9) response
  v                                                        |
 LLM App (chat UI, auth, sessions, streaming, citations) --+
  | (2) task                                               ^ (8) verified answer stream
  v                                                        |
 Agent Orchestrator  <-----------------------------+   Final LLM -> Verification Gate (Citation Gate)
  | (3) select tools                               |        ^ (7) augmented prompt
  v                                                |        |
 Tools & Retrieval Sources                         |   Context Builder
  [Vector DB | Docs | Web search | APIs | CRM | MCP]|        ^
  | (4) fetch evidence                             |        | (6) YES: evidence OK
  v                                                |        |
 Tool Health Gate -> Retrieved Evidence (Normaliser)|        |
  | (5) check vs task                              |        |
  v                                                |        |
 Evaluator Agent (separate block) -----------------+--------+
        NO: rewrite query / widen / switch source
        (returns to the Orchestrator; max_attempts default 3, hard cap 5)
```

Supporting services: Router, Session and Memory Service, Tool Registry, Budget Manager, Observability.

- The Evaluator never calls tools; it only returns a verdict.
- The Orchestrator alone decides what happens next.
- The Tool Health Gate and the Normaliser sit between Tools and the Evaluator.
- The Context Builder alone assembles prompts, on every path (§7).
- The Verification Gate sits between the LLM and the client on every grounded path (§13).

### 3.2 Diagram-to-plan mapping

| Diagram step | What happens | Plan owner |
| --- | --- | --- |
| 1 | User sends query | Chat UI, `POST /v1/chat/stream` (§15) |
| 2 | LLM App sends task to Orchestrator | API layer to Orchestrator (§9) |
| 3 | Orchestrator selects tools | Router and Tool Registry (§4, §8) |
| 4 | Tools fetch evidence | Tools, Tool Health Gate, Normaliser (§8) |
| 5 | Evidence checked against the task | Evaluator Agent (§5) |
| 6 | YES: evidence goes to Context Builder | Orchestrator on `answer` (§5, §7) |
| 7 | Augmented prompt goes to LLM | Context Builder to Final LLM (§7, §9) |
| 8 | Answer goes back to LLM App | Verification Gate, citations, stream (§13, §14) |
| 9 | Response goes to User | Chat UI (§15) |
| Red loop | NO: rewrite or switch source, max tries | Evaluator verdict to Orchestrator (§5, §6) |

### 3.3 Deployment topology

```text
[Browser: Next.js] --HTTPS/SSE--> [API binary (axum)] --> Postgres (pgvector, FTS, sessions, RLS)
                                       |                --> Redis (caches, rate limits, membership cache; disposable)
                                       |                --> Object storage (original files, web snapshots)
                                       |                --> LLM host (Nemotron 3 Super: answering, reranking, evaluation; OI-2) over HTTPS
                                       |                --> OIDC identity provider (token keys, userinfo)
[Ingestion worker binary] <-- Postgres job queue; may call a parsing sidecar (optional, any language) over HTTP
```

One Cargo workspace, two binaries, one database. Do not split into microservices until a measured scaling or ownership need appears. Local inference (nomic embeddings, NLI) runs in-process in the API and worker binaries, or in one dedicated inference process if CPU contention requires it (§20.5).

### 3.4 Orchestrator state graph and terminal outcomes

```text
START -> LOAD_SESSION -> ROUTE

ROUTE --open pending_clarification, message answers it--> MERGE_CLARIFICATION -> ROUTE (re-plan, resume budget)
ROUTE --materially ambiguous----------------------------> ASK_USER
ROUTE --greeting or thanks-----------------------------> REPLY(fixed template) -> PERSIST -> END
ROUTE --general, live, exact or out of scope-----------> REFER_TO_OWNER(reason) -> PERSIST -> END   (fixed template, no model call)
ROUTE --follow-up candidate-----------------------------> REUSE_EVIDENCE
           revalidate (ACL, version, age) + coverage check pass -> BUILD_CONTEXT(full) -> GENERATE
           any check fails ------------------------------------> SELECT_TOOLS
ROUTE --retrieval---------------------------------------> PLAN (sub-question DAG) -> SELECT_TOOLS

SELECT_TOOLS -> RESERVE_BUDGET -> EXECUTE_TOOLS -> COLLECT -> TOOL_HEALTH_GATE
  gate: some or all tools ok       -> NORMALISE -> ACL_RECHECK -> EVALUATE
  gate: zero evidence returned     -> deterministic verdict (no LLM) -> rewrite or switch
  gate: all failed, retriable      -> EXECUTE_TOOLS (transport retry, not an attempt, max 1)
  gate: all failed, not retriable  -> switch source if budget remains, else FAIL(sources_unavailable)
  gate: internal fault             -> mark tool faulted for this run, alert, treat as unavailable

EVALUATE --answer---------------------------------------> BUILD_CONTEXT(full) -> GENERATE
EVALUATE --rewrite_query | retrieve_more | switch_source
          (attempts left, hops left) -> next attempt parameters (§6) -> SELECT_TOOLS
          (limits reached) -> FAIL
EVALUATE --ask_user ------------------------------------> ASK_USER
EVALUATE --output invalid after repair and one re-run---> FAIL(evaluation_unavailable)   (fail closed; no answer from this attempt)

ASK_USER -> PERSIST(pending_clarification) -> END
FAIL -> BUILD_CONTEXT(gaps) -> GENERATE(template=insufficient_evidence or partial_findings)

GENERATE -> VERIFY_GATE (grounded modes) -> STREAM -> PERSIST -> END
Any state: cancellation token fired -> ABORT -> PERSIST(partial trace, on a non-cancelled task) -> END
Any state: unrecoverable internal error -> INTERNAL_ERROR -> PERSIST -> END
```

| Outcome | Path | What the user sees |
| --- | --- | --- |
| `answer` | EVALUATE → BUILD_CONTEXT(full) → GENERATE → VERIFY_GATE | Cited, verified answer |
| `refer_to_owner(reason)` | ROUTE → REFER_TO_OWNER | A fixed message that the question cannot be answered from the approved documents and that the Business Owner can help. Reasons: `general_question`, `out_of_scope`, `live_data`. No model call, no citations |
| `social_reply` | ROUTE → REPLY | A fixed greeting or thanks template; no model call |
| `ask_user` | ROUTE or EVALUATE → ASK_USER | One short clarifying question; no long answer |
| `fail_gracefully(reason)` | FAIL → BUILD_CONTEXT(gaps) → GENERATE → VERIFY_GATE | Honest answer: what was searched, what was found (verified findings only), what is missing, which sources were unavailable, next steps. Reasons: `insufficient_evidence`, `sources_unavailable`, `no_accessible_information`, `budget_exhausted`, `verification_failed`, `evaluation_unavailable`, `evidence_required` (every reason's message also names the Business Owner as the next step) |
| `aborted` | Any state → ABORT | Nothing (client disconnected); trace saved |
| `internal_error` | Any state → INTERNAL_ERROR | A safe error event with a run ID; alert raised (our bug or invariant violation only) |
| `upstream_failure(provider)` | Any state → UPSTREAM_FAILURE | A safe message that a provider (LLM, embedding, search) failed and the request can be retried; partial output marked incomplete. Counted and alerted separately from `internal_error` |

`no_accessible_information` is used for both "denied" and "not found" when revealing the difference would leak the existence of a resource (§16.2).

### 3.6 Release 1 architecture diagram

The same diagram is shipped as `architecture.mmd` and `architecture.png` next to this document. It follows the reference diagram's nine steps and the red loop, with the release 1 limits (documents only, referral for everything else).

```mermaid
flowchart TD
  U["User: internal IT team"] -->|"1 question"| APP["LLM App: chat UI, OIDC sign-in, SSE stream, citations"]
  IDP[("OIDC provider: token keys, userinfo")] <-->|"identity, groups, roles (max age 5 min)"| APP
  APP -->|"2 task"| ORCH["Agent Orchestrator: bounded state machine"]
  ORCH --> ROUTER{"Router + PlanValidator + requirement coverage"}
  ROUTER -->|"greeting or thanks"| SOC["social_reply: fixed template"]
  ROUTER -->|"general, live, exact, or out of scope"| REF["refer_to_owner: fixed template naming the Business Owner"]
  ROUTER -->|"document question"| SEL["3 select tool: hybrid document search"]
  SEL --> EMB["nomic-embed-text-v1.5 (768-d, local)"]
  SEL --> PG[("Postgres: pgvector + full-text, live-ACL RLS")]
  SEL --> RR["LLM listwise rerank: Nemotron 3 Super"]
  RR -->|"4 evidence"| NORM["Tool Health Gate + Normaliser + ACL recheck"]
  NORM -->|"5 check against task"| EVAL["Evaluator: rules A to F, exact-quote check, fails closed"]
  EVAL -->|"NO: rewrite or widen, default 3 attempts, hard cap 5"| ORCH
  EVAL -->|"6 YES"| CB["Context Builder: full or gaps mode"]
  EVAL -->|"limits reached"| FAIL["fail_gracefully + refer to owner"]
  CB -->|"7 augmented prompt"| LLM["Nemotron 3 Super: JSON Lines claim units"]
  LLM --> GATE["Grounding Gate: P0, P1, C1 to C5, local NLI"]
  GATE -->|"8 verified units"| APP
  APP -->|"9 response"| U
  SOC --> APP
  REF --> APP
  FAIL --> APP
  APP -.->|"events for reconnect"| EVT[("Postgres run_events, 24 h")]
```

### 3.5 Sources by stage

| Source | Stage | Why |
| --- | --- | --- |
| Hybrid document search (vector + keyword + rerank) | 1 | Core path and the baseline |
| Document store (raw files, metadata) | 1 | Citations, re-chunking |
| Web search | 4 | Current information; adds injection and SSRF risk |
| SQL/database | 4 | Exact data; read-only role and query guard |
| REST APIs | 4 | Typed wrappers |
| CRM / business systems | 5 | Per-user delegated credentials |
| MCP servers | 5 | Plug-in layer once the `Tool` trait is stable |

All sources implement the same `Tool` trait, so later stages never change the Orchestrator, Evaluator or Context Builder. **Release 1 registers only hybrid document search and document lookup.** The web, SQL, REST, CRM and MCP rows are designs for later releases; their code is feature-gated off and a CI check fails if any of them is registered.

## 4. Agent decision-making

Routing runs in two steps to save latency and cost:

1. **Rule pre-router (no LLM):** pending clarification present, greeting or thanks (short message matching a small pattern list), explicit commands. If it resolves the turn, the LLM router is skipped.
2. **LLM router** (small, fast model, structured JSON) returns a plan:

```json
{
  "intent": "policy_lookup",
  "needs_retrieval": true,
  "sources": ["documents"],
  "requirements": [
    {"id": "r1", "text": "State the refund window", "kind": "entity"},
    {"id": "r2", "text": "State any exception for digital goods", "kind": "condition"}
  ],
  "sub_questions": [
    {"id": "q1", "text": "What is the refund window?", "required": true, "depends_on": [], "reqs": ["r1"],
     "data_class": "document", "source": "documents", "query": "refund window policy"},
    {"id": "q2", "text": "Does it differ for digital goods?", "required": true, "depends_on": [], "reqs": ["r2"],
     "data_class": "document", "source": "documents", "query": "refund policy exception for digital goods"}
  ],
  "is_followup": false,
  "ambiguous": false,
  "clarifying_question": null,
  "clarification_reply": false,
  "standalone_query": "Using the refund policy, state the refund window and any digital-goods exception"
}
```

`data_class` is one of `document` (policy or content questions), `exact` (records, figures, status from a system of record), `live` (current or time-sensitive information) or `general` (stable model knowledge). **In release 1 only `document` has a source.** A request classified `exact`, `live` or `general`, or outside the documents' domain, is not planned at all: it ends in `refer_to_owner` (§3.4). Each sub-question carries its own `source` and `query`; tools are called with those, and `standalone_query` is used for memory, display and the `TASK` line.

**PlanValidator (deterministic code, runs on every router output).**

| Check | Rule | On failure |
| --- | --- | --- |
| Schema | Valid against the `schemars` schema, `deny_unknown_fields`, at most `max_sub_questions`, acyclic `depends_on` | One repair call with the validation errors |
| Source union | `sources` equals the union of the sub-questions' `source` values | Repair |
| Source exists and is authorised | Each `source` is a registered, enabled tool that the Principal's scopes allow | The sub-question becomes `Blocked(no_source)`. It is **never silently dropped**: it stays in the plan, the Evaluator cannot mark it supported, and the outcome is `fail_gracefully(no_accessible_information)` with partial findings or `ask_user` |
| Class and source compatibility | `exact` needs a tool with capability `Exact` or `Structured` and authority `SystemOfRecord`; `live` needs capability `Live`; `document` needs `Semantic`; `general` is not permitted in release 1 (the request becomes `refer_to_owner(general_question)`) | Repair, then `Blocked(no_source)` |
| Entity coverage | Every named entity, number and date in each sub-question text appears in that sub-question's `query` or in `standalone_query` (normalised match) | Repair |
| Requirement coverage | The plan carries `requirements[]` (each `{id, text, kind: entity \| condition \| comparison \| constraint \| output}`, each mapped to sub-question ids or to the `output_spec`). A deterministic extractor independently finds requirement markers in the original request: conjunctions that add asks, comparison words (compare, versus, difference, better), conditionals (if, unless, only when), quantifiers and limits (at least, within the last, top N), exclusions, and output-format asks (table, list, steps, summary length). **Every marker must be covered by a requirement**, and every requirement by a sub-question or the `output_spec`. A second cheap-model call (different from the router, schema-constrained) checks the mapping against the original text | Repair once; then `ask_user` if the user can resolve it; else the run continues but is marked `coverage = partial` (below) |
| Marker consistency | If the rule classifier found exact or live markers (below) but the plan has no `exact` or `live` sub-question | Repair, then `ask_user` |

**Partial coverage is never presented as complete.** When a requirement stays unmapped or unsupported, the answer carries `coverage = partial` in the `done` event and a system-generated statement naming what was not addressed ("This answer does not cover: the comparison with the 2024 policy"). Requirements of kind `output` (format, length) are applied in the Context Builder's instructions and are **not** verified by the grounding gate; the document does not claim they are.

If repair fails, the **conservative fallback** applies. It never answers an exact or live question from documents.

| Rule-classifier signal | Fallback behaviour |
| --- | --- |
| Greeting or thanks | `social_reply` (fixed template) |
| Exact or live markers: configured ID patterns, words such as status, balance, order, ticket, invoice, latest, today, current, price | `refer_to_owner(live_data)` in release 1 (no authoritative-source tools exist) |
| Document markers: the request carries `document_ids`, or the text names "our", "the uploaded", "this document", "policy" and no exact or live markers | Hybrid document retrieval, one `document` sub-question, `degraded_plan = true`. The Evaluator rules (§5) still apply, so an answer still needs verbatim-quoted evidence |
| No positive signal | `ask_user` |

**Decision rules (in order)**

| Condition | Action |
| --- | --- |
| Unexpired `pending_clarification` and the message answers it | Merge with the original plan, re-route (§11) |
| Greeting or thanks | Fixed template reply (`social_reply`); no model call |
| Follow-up possibly answerable from the previous turn's evidence | `REUSE_EVIDENCE` (§10.4): revalidation and coverage check; pass means no retrieval, fail means retrieve |
| General knowledge, opinions, or anything outside the documents' domain | `refer_to_owner(general_question or out_of_scope)`; the model never answers from its own knowledge |
| Mentions "our / the uploaded / this document", policy, internal terms | Hybrid document retrieval |
| Exact figures, status, records, or any live data | `refer_to_owner(live_data)`; never answered from documents |
| "Latest / today / current / price / news" | `refer_to_owner(live_data)`; no web search in release 1 |
| Ambiguous, one dominant reading | Proceed under the stated interpretation. The interpretation is shown to the user; **evidence is still required for every claim** (an assumption about meaning is not evidence) |
| Ambiguous, materially different readings | `ask_user`, or retrieve across sources if cheap |
| Poor retrieval | Rewrite query |
| Conflicting evidence | Retrieve from a higher-authority source |
| All tools failed | Tool Health Gate rules (§8.4), not the Evaluator |
| Insufficient at the limits | `fail_gracefully` |

Authority when sources disagree: live system of record (API/DB) > internal documents (newest version) > web (later release). Model knowledge is never a source in release 1.

**Plan rules.** The sub-question list is a directed acyclic graph. Independent sub-questions run in parallel in the same wave. A sub-question with `depends_on` runs in a later wave and may use typed values extracted from earlier results (§16.6). The plan is capped (default 5 sub-questions, 2 hops) and rejected if cyclic.

## 5. Evaluator Agent

The Evaluator is a separate block. It receives evidence, returns a verdict, and never calls tools or mutates state. The Orchestrator acts on the verdict.

**Inputs**

- Standalone task and the router's sub-questions (with `required` flags).
- Normalised evidence (text, source, scores, date, authority), delimited as untrusted data.
- Attempt history: queries, sources and parameters already tried.
- `tool_status` per call, for example `{ "documents": "ok", "web": "timeout", "crm": "denied" }`. Failed sources are listed explicitly, so a failed source's silence is never read as "not found".

**Division of labour with the Tool Health Gate.** The gate handles availability before evaluation. Total failure and zero results never reach the LLM judge. When some sources failed, the Evaluator marks affected sub-questions `blocked` with the source, not `missing`.

**Output (strict JSON, schema-validated)**

```json
{
  "sub_questions": [
    {"id": "q1", "status": "supported",
     "support": [{"evidence_id": "e3", "quote": "Refunds are accepted within 30 days of delivery"}]},
    {"id": "q2", "status": "missing", "blocked_by": null},
    {"id": "q3", "status": "blocked", "blocked_by": "crm:timeout"}
  ],
  "conflicts": [],
  "relevant_evidence_ids": ["e3", "e7"],
  "irrelevant_evidence_ids": ["e1", "e2"],
  "recommended_action": "rewrite_query",
  "suggested_query": "refund policy exceptions for digital goods",
  "clarifying_question": null,
  "confidence": 0.58
}
```

**Answer decision (v5), computed by code, not by the model's self-reported confidence.**

```text
answer iff ALL of:
 (A) every REQUIRED sub-question has status = supported
 (B) for each supported sub-question, every support entry satisfies:
       - evidence_id is in the current evidence set and was marked relevant
       - quote is a verbatim substring of that evidence text (after whitespace and case normalisation)
         and is at most 40 words
       - evidence rerank score >= the calibrated floor for the reranker model in use
 (C) conflicts is empty, or every conflict is resolved by the authority rule (§4) and recorded
 (D) no required sub-question is blocked by a failed source or by a missing authorised source
 (E) for each `exact` sub-question, the supporting evidence has authority SystemOfRecord and `as_of` within the tool's freshness TTL;
     for each `live` sub-question, the evidence comes from a `Live` tool within its TTL;
 (F) every requirement in the validated plan (§4) is mapped to a supported required sub-question or to the `output_spec`; otherwise the answer is marked `coverage = partial` and states what it does not cover. A partial answer is never presented as complete.
     Document evidence can never satisfy an `exact` or `live` sub-question
```

- Rule (B) makes the Evaluator's sufficiency claim checkable. A hallucinated quote or ID fails the check mechanically.
- Optional sub-questions (`required: false`) may be missing; the final answer then states the gap.
- If any required sub-question is not supported and the limits are reached, the outcome is `fail_gracefully` with verified partial findings, not a guess.
- The model's `confidence` is logged and used only as a tie-breaker or to select cautious wording. Any numeric threshold on it must be calibrated on the golden set first.

| Action | Diagram path | Trigger |
| --- | --- | --- |
| `answer` | YES (step 6) | Decision rule above satisfied |
| `rewrite_query` | NO (red loop) | Mostly irrelevant evidence, or terms mismatch the corpus vocabulary |
| `retrieve_more` | NO (red loop) | Relevant but partial: widen K, parents and neighbours |
| `switch_source` | NO (red loop) | Wrong source type, blocked source, or a conflict needing another authority |
| `ask_user` | Exit (extension) | Only the user can supply the information, or the request is truly ambiguous |
| `fail_gracefully` | Exit (extension) | Limits reached with insufficient evidence |

The model's recommended action is validated against remaining limits and the retry matrix (§6.4). An action that is not allowed is replaced by the nearest allowed one.

**Reliability and cost**

- Separate prompt, temperature 0, strict JSON-schema validation with one repair retry.
- **The Evaluator fails closed.** Rerank scores can shortlist evidence but can never establish that a sub-question is supported, validate a quote, or resolve a conflict, so they are never used to produce an `answer` verdict. Invalid output follows this ladder, all inside one `EVALUATE` step and within the token budget: (1) one repair call with the validation errors; (2) one re-run with the alternate evaluator model or prompt; (3) if the output is still invalid, the attempt ends with `fail_gracefully(evaluation_unavailable)`. The user is told that sufficiency could not be assessed, is shown the titles of the sources found, and is invited to retry. No answer is generated from that attempt's evidence. The event is counted as `evaluator_unavailable` and alerted on.
- A deterministic answer path may be introduced only as the deferred fast path (§25), and only if it implements the full rules (A) to (F), including quote and conflict checks, on its own.
- The evaluator is a different prompt and, when affordable, a different model from the generator to reduce correlated errors. The choice is made by the evaluator calibration test (§19.4), not by assumption.
- The LLM judge sees only the shortlist (top reranked items), never all candidates.
- Optional later optimisation (Stage 4 or later, only if measured): a deterministic fast path that skips the LLM judge when scores are far above the floor, all entities are present and there is one sub-question. It ships only if it matches human labels on the golden set (§25).

## 6. Retry, budgets and multi-hop semantics

### 6.1 Limits

The diagram's "max 5 tries" is a ceiling; the default is lower.

```yaml
agent_limits:
  max_attempts: 3             # default; per-tenant or per-request override
  max_attempts_hard_cap: 5    # matches the reference diagram; never exceeded; enforced in code
  max_hops: 2                 # dependent waves within one attempt; not attempts
  max_sub_questions: 5
  max_query_rewrites: 2
  max_source_switches: 1      # per sub-question
  max_tool_calls: 8           # every tool execution, parallel or not, including transport retries
  max_transport_retries: 1    # per tool, per attempt; not an attempt
  resume_max_attempts: 2      # after a clarification
  max_wall_clock_s: 40        # grounded answers; tune from Stage 1 measurements
  max_generation_retries: 3   # invalid stream data, only before any answer text is shown (D6); not attempts
  max_output_tokens: 4000     # per answer (D6)
  token_budget: 60000         # per request, all model calls combined; a hard stop
```

**Token budget check.** Router about 2k tokens; each evaluator call up to 9k (8k evidence plus 1k out), at most 3 calls, 27k; generation 9k; each generation retry 9k, at most 3, 27k. The worst case is about 65k, so the 60k hard stop turns it into `budget_exhausted` instead of an overspend; a typical single-attempt run is about 20k. Cost in money depends on the hosting chosen in OI-2, so the per-run and per-day cost caps are configuration set after OI-2 (OI-9).

`max_attempts` is clamped to `max_attempts_hard_cap` in code. Attempts 4 and 5 exist only in the deferred deep-research mode (§25). Rewrites, widen steps and source switches are bounded by `max_attempts - 1`, because each one consumes the next attempt.

### 6.2 Attempt definition

One attempt is one full pass over all unresolved sub-questions: select tools, reserve budget, execute, gate, evaluate. A verdict never creates a pass of its own; it sets the parameters of the next attempt.

| Verdict | Effect on the next attempt |
| --- | --- |
| `rewrite_query` | Use `suggested_query` for the affected sub-questions (counts against `max_query_rewrites`) |
| `retrieve_more` | `widen = true`: K 20 to 40, expand to parents and neighbours |
| `switch_source` | Change the tool set (counts against `max_source_switches`) |

Verdicts can combine in one attempt (rewrite plus widen). Transport retries are not attempts. Dependent waves (hops) inside one attempt are not attempts but count toward tool calls and wall-clock.

```text
Attempt 1: original standalone query, hybrid search (K=20 -> rerank -> keep 6)
   wording mismatch          -> Attempt 2: rewrite (synonyms, acronym expansion)
   relevant but partial      -> Attempt 2: same query + widen
   both                      -> Attempt 2: rewrite + widen
   Attempt 2 insufficient    -> Attempt 3: switch source (web / API / DB) or split sub-queries
   Attempt 3 insufficient    -> ask_user if the gap is user-answerable, else fail_gracefully
```

### 6.3 Per-sub-question state and budgets

```rust
pub struct SubQuestionState {
    pub id: SqId,
    pub required: bool,
    pub status: SqStatus,            // Pending | Supported | Missing | Blocked(SourceId) | Conflicted
    pub evidence: Vec<EvidenceId>,
    pub tried: HashSet<TriedPair>,   // (normalised query, tool, params) never repeated
    pub rewrites_used: u8,
    pub switches_used: u8,
}

pub struct Budget {                  // shared by every task in a run
    attempts_used: AtomicU8, tool_calls_used: AtomicU32, tokens_used: AtomicU64,
    deadline: tokio::time::Instant, caps: AgentLimits,
}
impl Budget {
    /// Atomically reserves n tool calls before any task is spawned (compare-exchange loop).
    pub fn try_reserve_calls(&self, n: u32) -> Result<CallReservation, BudgetExceeded>;
    pub fn try_reserve_tokens(&self, n: u64) -> Result<TokenReservation, BudgetExceeded>;
}
```

Rules:

- All calls of a wave are reserved **before** spawning. A wave that does not fit is trimmed by priority (required sub-questions first), never partially started and then abandoned. Each spawned call receives its own `CallReservation`; **`call_with_retry` never reserves for the first attempt**.
- Reservations are consumed even if the call is cancelled, so cancellation cannot be used to exceed budgets. **Dropping a `CallReservation` never refunds it.** Reserving a wave and spawning its tasks is one non-awaiting step (`reserve_wave` returns the reservations and the caller spawns immediately), so there is no cancellation point between reserving and starting; a run cancelled before that step reserved nothing.
- **Only retries reserve additional budget.** A retry proceeds through these steps in order, and stops at the first failing one: (1) the run is not cancelled; (2) the backoff plus the tool's `min_useful_time` fits in the time remaining before the run deadline; (3) the backoff sleep, which is itself cancellable; (4) the run is still not cancelled; (5) one more call is reserved with `try_reserve_calls(1)`. Reserving last means a cancelled or timed-out retry never consumes budget.
- Configuration, per tool: `ToolSpec.min_useful_time` (default 500 ms) and `RetryPolicy { max_transport_retries: u8 (default 1, never above 2), base_backoff, max_backoff }` (§8.1). The retry-action matrix in §6.4 decides which failures may retry at all.

```rust
pub struct RetryPolicy { pub max_transport_retries: u8, pub base_backoff: Duration, pub max_backoff: Duration }

fn backoff(p: &RetryPolicy, retry_no: u8, server_hint: Option<Duration>) -> Duration {
    let exp = p.base_backoff.saturating_mul(1u32 << retry_no.min(6));
    server_hint.unwrap_or(exp).min(p.max_backoff)          // jitter is added by the caller's RNG
}

/// `first` is the reservation made for this call when its wave was reserved.
async fn call_with_retry(tool: &dyn Tool, args: &Value, ctx: &ToolContext, budget: &Budget,
                         first: CallReservation) -> CallOutcome {
    let spec = tool.spec();
    let mut retries: u8 = 0;
    let mut _held = first;                                  // reservations are never refunded
    loop {
        let remaining = ctx.deadline.saturating_duration_since(Instant::now());
        if remaining < spec.min_useful_time { return CallOutcome::Deadline; }
        let call = tokio::time::timeout(spec.timeout.min(remaining), tool.execute(args.clone(), ctx));
        let res = tokio::select! { biased; _ = ctx.cancel.cancelled() => return CallOutcome::Cancelled, r = call => r };
        match res {
            Ok(Ok(out)) => return CallOutcome::Output(out),
            Ok(Err(ToolError::Provider(e))) if e.is_transient() && retries < spec.retry.max_transport_retries => {
                retries += 1;
                let wait = backoff(&spec.retry, retries, e.retry_after());
                if ctx.cancel.is_cancelled() { return CallOutcome::Cancelled; }                         // step 1
                let remaining = ctx.deadline.saturating_duration_since(Instant::now());
                if wait + spec.min_useful_time > remaining { return CallOutcome::Deadline; }            // step 2
                tokio::select! { biased;                                                                // step 3
                    _ = ctx.cancel.cancelled() => return CallOutcome::Cancelled,
                    _ = tokio::time::sleep(wait) => {} }
                if ctx.cancel.is_cancelled() { return CallOutcome::Cancelled; }                         // step 4
                _held = match budget.try_reserve_calls(1) { Ok(r) => r, Err(_) => return CallOutcome::BudgetExhausted }; // step 5
            }
            Ok(Err(e)) => return CallOutcome::Failed(e),
            Err(_elapsed) => return CallOutcome::Timeout,
        }
    }
}
```

- **Tests (Spike S9, with Tokio paused time).** (1) Total reservations equal wave reservations plus performed retries, never more. (2) Cancellation during backoff returns `Cancelled` within 50 ms and consumes no extra reservation. (3) A retry whose backoff does not fit before the deadline returns `Deadline` without sleeping. (4) With `max_transport_retries = 1`, a second transient failure is final. (5) A property test over random failure sequences never exceeds the call cap. (6) Cancellation after a reservation but before the first await consumes exactly that reservation and no more.
- Each sub-question has its own status. Only unresolved sub-questions are retried; supported ones are kept and not re-queried.
- **Partial success.** A sub-question is `Supported` only under the answer rule (B). A mix of supported and unsupported required sub-questions never yields `answer`; it continues the loop, and at the limits yields `fail_gracefully` carrying the verified supported findings.
- Tool failures affect only the sub-questions mapped to that call; other sub-questions proceed.

### 6.4 Retry-action matrix by failure type

| Observation | Transport retry | Rewrite | Widen | Switch source | Ask user | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Timeout or 5xx (transient) | Once | No | No | Yes, after the retry fails | No | The query is not the problem |
| Rate limited | Once, honouring `Retry-After` if it fits the deadline | No | No | Yes | No | Counts against provider concurrency limits |
| Denied or unauthorised | No | No | No | Only to sources the user may use | No | Fail with `no_accessible_information`; do not reveal why |
| Zero results, `NoMatch` semantics | No | Yes | Yes | Yes | If the gap is user-answerable | Deterministic verdict, no LLM call |
| Zero results, `NotFoundIsAnswer` semantics | No | No | No | No | No | "Not found" is evidence; it flows to the answer (§8.3) |
| Low relevance | No | Yes | No | Yes | No | |
| Relevant but partial | No | No | Yes | Maybe | No | |
| Conflicting evidence | No | No | No | To a higher-authority source | No | Else present both sides with dates and authority |
| Invalid tool arguments from the router | No | No | No | No | No | One schema repair; not an attempt |
| Internal fault (bug, panic) | No | No | No | Yes, if another source exists | No | Alert; never retried blindly (§8.2) |
| Evaluator output invalid | One repair, then one re-run with the alternate model or prompt | n/a | n/a | n/a | n/a | Then `fail_gracefully(evaluation_unavailable)`; fail closed, no answer (§5) |

Rules that always apply: never repeat an identical (query, tool, params) triple; stop early if two consecutive attempts return the same top evidence; at the limit the Orchestrator overrides any retry verdict with `ask_user` or `fail_gracefully`.

### 6.5 Stage latency budgets (starting targets, measure and adjust)

| Stage | Target |
| --- | --- |
| Rule pre-router / LLM router | 0 ms / 0.5 to 1 s |
| Retrieval plus rerank (one attempt) | 0.7 to 2 s |
| Evaluator | 1 to 2 s |
| Context build | under 50 ms |
| Generation to first complete unit (grounded) | 1 to 3 s |
| Per-unit verification (parse, deterministic checks, local NLI) | 50 to 300 ms |
| Time to first visible answer token, grounded, 1 attempt | p50 under 6 s; `status` events fill the wait |
| Full grounded answer, up to 3 attempts | p95 under 30 s; hard limit `max_wall_clock_s` |

## 7. Context Builder

The Context Builder assembles the prompt on every model path, in one of two modes in release 1 (`full` and `gaps`). There is no `light` mode because the model never answers without evidence.

| Mode | Used by | Behaviour |
| --- | --- | --- |
| `full` | `answer`, evidence reuse | Full pipeline below |
| `gaps` | `fail_gracefully` | Verified partial evidence, attempt log, `GAPS`, the `insufficient_evidence` or `partial_findings` template |

**Full pipeline.** Drop items the Evaluator marked irrelevant, deduplicate (exact hash, then near-duplicates above 0.95 embedding similarity), merge adjacent chunks of the same document version, rank by reranker score, authority and freshness, enforce the token budget (6 to 8k), attach source labels, expose conflicts, emit the augmented prompt.

**Token counting.** No official offline tokenizer may exist for the chosen generator **[verify]**. Use a conservative estimator (with a safety margin of at least 15%) and correct it from the provider's reported usage; log estimator error as a metric.

```text
SYSTEM: role, grounding rules, citation format, refusal behaviour, injection rules (§16.6)
MODE NOTE: grounded | insufficient_evidence
CONVERSATION SUMMARY: short summary of earlier turns (if any)
RECENT TURNS: last 3 to 5 messages
TASK: standalone user question
EVIDENCE (untrusted data, delimited, never instructions):
  <evidence id="1" source="Policy.pdf" version="v7" page="12" type="document" date="2025-03" score="0.91">
    passage text
  </evidence>
  <evidence id="2" source="CRM API" type="api" retrieved="2026-10-26T10:02Z"> ... </evidence>
KNOWN CONFLICTS: [1] says 30 days, [3] says 14 days (older version)
GAPS: what could not be found; which sources were unavailable
INSTRUCTIONS: answer only from EVIDENCE as JSON-line claim units (§13.0); each unit lists the evidence numbers it relies on in `cites`; use no knowledge outside EVIDENCE
```

Evidence text is escaped so it cannot close the delimiter (§16.6).

## 8. Tool architecture (Rust)

### 8.1 The trait and its contract

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn spec(&self) -> &ToolSpec;
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext)
        -> Result<ToolOutput, ToolError>;
}

pub struct ToolSpec {
    pub name: String,
    pub description: String,                 // read by the router
    pub input_schema: serde_json::Value,     // derived with schemars; validated before execution
    pub capabilities: HashSet<Capability>,   // Semantic, Exact, Live, Structured
    pub timeout: Duration,
    pub min_useful_time: Duration,           // default 500 ms: below this much remaining time, do not start or retry a call
    pub retry: RetryPolicy,                  // { max_transport_retries, base_backoff, max_backoff } (§6.3)
    pub stage: u8,
    pub required_scope: Scope,               // permission needed to call at all
    pub side_effects: SideEffects,           // ReadOnly only, until write tools are designed
    pub empty_semantics: EmptySemantics,     // NoMatch | NotFoundIsAnswer
    pub freshness: FreshnessPolicy,          // max reuse age; "live" means never reuse
    pub max_result_bytes: usize,
    pub authority: Authority,                // SystemOfRecord | Internal | Web
}

pub struct ToolContext {                     // never contains secrets in loggable form
    pub principal: Principal,                // tenant, user, groups, delegated token handle
    pub cancel: CancellationToken,
    pub deadline: tokio::time::Instant,
    pub tenant_tx: TenantTxFactory,          // the only way to reach tenant data (§16.3)
    pub run_id: RunId,
}

pub struct ToolOutput {
    pub evidence: Vec<Evidence>,
    pub partial: Option<Partial>,            // { reason, next_cursor }: results truncated, more exist
    pub freshness: Freshness,                // { as_of, ttl }
    pub cost: Option<f64>,
    pub latency_ms: u64,
}
```

**Authorisation is part of the contract.** A tool receives a `Principal` and must enforce the principal's permissions itself; the registry only filters which tools the router may see. Tools cannot be given the ability to act for a different principal.

**Pagination and partial results.** A tool returns at most `max_result_bytes`. If more exists it sets `partial`. The Orchestrator may request a next page only through `retrieve_more` (a widen step), never implicitly. A partial result can satisfy an answer only if the support rule (§5, B) is met by the returned portion; the Context Builder records that results were truncated.

**Freshness.** Evidence carries `as_of`. The Context Builder shows the date for live sources, and evidence reuse (§10.4) rejects evidence older than its `ttl`.

### 8.2 Two-layer error model

Expected provider failures and programming faults are different things and are kept distinct.

```rust
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("provider error")]  Provider(#[from] ProviderError),   // expected, classified
    #[error("internal error")]  Internal(#[source] anyhow::Error), // bug or invariant violation
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum ProviderError {
    Timeout, RateLimited { retry_after: Option<Duration> },
    Unavailable { status: Option<u16> }, BadRequest, Denied, NotFound,
}
```

- Tools return `Result`; nothing is swallowed inside a tool. Panics are caught at the task boundary (`JoinSet` join errors).
- The Orchestrator's `collect` step maps every task result, including timeouts, join errors and cancellation, to one `CallRecord { tool, reservation, outcome }` where `outcome` is `Output`, `Provider(kind)`, `Timeout`, `Cancelled`, `Internal(error_id)` or `Panicked(error_id)`. This single function is the only place where the nested `Result<Result<_, ToolError>, Elapsed>` and `JoinError` cases are handled, and it is unit-tested exhaustively.
- Mapping to `ToolStatus` (for the gate and the Evaluator) and to user-safe text happens at this boundary. `Internal` and `Panicked` details go to logs and traces with an `error_id`; the user and the Evaluator see only a generic "source unavailable". No internal message or SQL text is ever exposed.
- Internal faults raise an alert, are counted per tool, and trip the tool's circuit breaker; they are never retried blindly.

### 8.3 Empty-result semantics

| `empty_semantics` | Meaning of zero results | Handling |
| --- | --- | --- |
| `NoMatch` | The query matched nothing in a fuzzy source (documents, web) | Deterministic verdict: rewrite, widen or switch |
| `NotFoundIsAnswer` | An authoritative lookup found no record (order, customer, ticket) | Becomes evidence "no record found for X as of T" and flows to the answer; no retry. When revealing absence would leak existence, it is reported as `no_accessible_information` (§16.2) |

### 8.4 Tool Health Gate rules

| Status | Gate action |
| --- | --- |
| `Ok` | Pass to the Normaliser |
| `Empty` (`NoMatch`) | Deterministic "insufficient" verdict, no LLM call, then rewrite or switch |
| `Empty` (`NotFoundIsAnswer`) | Pass as evidence |
| `Timeout`, transient `Provider` error | One transport retry, not an attempt |
| `RateLimited` | Retry once if `retry_after` fits the deadline, else treat as unavailable |
| `Denied` | No retry. Source dropped for this run; audit entry written |
| `Internal`, `Panicked` | Source marked faulted for this run; alert |
| All tools unavailable | `FAIL(sources_unavailable)` |

### 8.5 Discovery, invocation and registration

- Tools register at startup into a `ToolRegistry` (`HashMap<String, Arc<dyn Tool>>`) through an explicit `register_tools(cfg)` function, filtered by configuration, stage, tenant and principal scope.
- The router sees only name, description, capabilities and input schema. The Orchestrator validates arguments against the schema before execution and rejects unknown fields.
- Independent calls run on a `JoinSet`, each wrapped in `tokio::time::timeout` and given a child `CancellationToken`. Transport retries use `backon`. Every tool must be cancel-safe (§20.3).
- The Normaliser converts each `ToolOutput` into `Evidence` with IDs and provenance, then `ACL_RECHECK` removes any item the principal may not see (§12.2).
- **Adding a tool without touching core logic:** implement `Tool`, register it, add a configuration entry, pass the contract tests (§19, P15). MCP servers are wrapped by an `McpTool` adapter built on `rmcp` **[verify]** behind our own `McpClient` trait (§20.7).

Tools: `HybridSearchTool` and `DocumentSearchTool` (metadata and filename lookup) are the **only tools registered in release 1**. `WebSearchTool`, `DatabaseQueryTool`, `RestApiTool`, `CrmTool` and `McpTool` are later-release designs, feature-gated off and absent from the registry.

### 8.6 Evidence

```rust
pub struct Evidence {
    pub id: EvidenceId,                  // run-scoped (e12); rendered to the model as [n]
    pub text: Arc<str>,
    pub anchor: SourceAnchor,            // stable, resolvable location (§14.1)
    pub tool: String,
    pub authority: Authority,
    pub retrieval_score: f32,
    pub rerank_score: Option<f32>,
    pub rerank_model: Option<ModelId>,
    pub as_of: OffsetDateTime,
    pub doc_date: Option<OffsetDateTime>,
}
```

## 9. Orchestrator runtime and run lifecycle

### 9.1 Implementation shape

The orchestrator is a hand-written state machine over a Rust `enum` (`State`), driven by a single `async fn run(state, ctx) -> Outcome` loop. Transitions are exhaustive `match` arms, so the compiler forces every state and outcome to be handled. Limits are enforced in code through the `Budget` (§6.3). Each transition emits a `tracing` span and a `run_steps` row, so any run can be replayed from its trace.

### 9.2 Run lifecycle

```text
1. Authenticate; derive Principal; acquire the per-conversation lock (one active run per conversation)
2. Create run row (status=running), CancellationToken, Budget, event channel
3. Spawn the run task; return the SSE stream wired to the event channel
4. Run the state machine (§3.4), checkpointing RunState to Redis at each transition (TTL)
5. At a terminal state: persist messages, citations, verification result, trace (on a task that is NOT cancelled by client disconnect)
6. Release the conversation lock; close the stream with `done` or `error`
```

### 9.3 Concurrency rules

- One active run per conversation, enforced by a fenced lock (§9.5). A second request gets `409` with the active `run_id`, or is queued if configured.
- Idempotency: `POST /v1/chat*` accepts an `Idempotency-Key` with the exact semantics in §9.5.
- Per-tenant concurrency limit (semaphore) and per-provider concurrency limits prevent noisy neighbours from exhausting capacity (§17.4).
- Persisting the trace and final message runs on a separately spawned, non-cancellable task with its own timeout, so a client disconnect never loses the audit trail.

### 9.4 Cancellation propagation

The run owns a root `CancellationToken`. Children go to every tool call, the LLM client and the Verification Gate. The token fires on client disconnect (§15.3), explicit cancel, deadline expiry or shutdown. Each await point that matters is wrapped in `tokio::select!` against `token.cancelled()`. Dropping an in-flight `reqwest` future closes the connection to the provider; the provider call is stopped as far as the provider honours it **[verify billing behaviour on cancel]**.

### 9.5 Idempotency keys and the fenced conversation lock

**Idempotency**

| Aspect | Rule |
| --- | --- |
| Scope | `(tenant_id, user_id, route, Idempotency-Key)`; key length at most 128 characters. The web client always sends one; other clients may omit it (no dedupe) |
| Fingerprint | SHA-256 of the canonical request body (sorted keys, normalised whitespace) stored with the key |
| First request | Insert into `idempotency_keys` with `INSERT ... ON CONFLICT DO NOTHING`; the inserter owns the run. This is race-safe because the unique index decides the winner |
| Same key, same fingerprint | `in_progress`: attach the caller to the run's event stream (replay from Redis); `completed`: return the stored final result. Retained 24 h |
| Same key, different fingerprint | `422 idempotency_key_reuse`; nothing is executed |
| Failure | A run that ends `aborted` or `internal_error` releases the key as `failed` so a retry with the same key starts a new run |

**Conversation lock with a database-issued fence**

The lease, the owner and the fence all live in Postgres, in the `conversations` row (`fence bigint NOT NULL DEFAULT 0`, `lease_owner uuid NULL`, `lease_expires_at timestamptz NULL`). All time comparisons use **database wall-clock time via `clock_timestamp()`**, never application clocks and never `now()` (which is the transaction start time and would make a long transaction compare against a stale instant).

```sql
-- acquire (one short transaction)
UPDATE conversations
   SET fence = fence + 1, lease_owner = $run_id, lease_expires_at = clock_timestamp() + interval '30 seconds'
 WHERE id = $id AND (lease_owner IS NULL OR lease_expires_at <= clock_timestamp())
RETURNING fence;          -- no row: another run holds a live lease -> 409 with the active run_id

-- heartbeat every 10 s
UPDATE conversations SET lease_expires_at = clock_timestamp() + interval '30 seconds'
 WHERE id = $id AND lease_owner = $run_id AND fence = $fence AND lease_expires_at > clock_timestamp();
                          -- 0 rows: lease lost -> cancel the run (`lock_lost`); the run stops writing

-- every persistent write by the run (messages, memory, pending_clarifications, run summary):
-- the first statement of the same transaction
UPDATE conversations SET updated_at = now()
 WHERE id = $id AND lease_owner = $run_id AND fence = $fence AND lease_expires_at > clock_timestamp();
                          -- 0 rows: abort the transaction

-- release
UPDATE conversations SET lease_owner = NULL, lease_expires_at = NULL
 WHERE id = $id AND lease_owner = $run_id AND fence = $fence;
```

- **Why this is safe.** The fence is advanced in the same statement that grants the lease, and it only increases; it is never reset. Acquire and every write take the same row lock, so a new owner cannot be granted the lease while a write transaction of the old owner is in flight, and an old owner's write after takeover finds a different owner and fence and aborts. A run whose lease has expired also cannot write, even if nobody has taken over.
- **Durability depends on synchronous replication.** A committed fence survives a Postgres failover only with the synchronous standby required in §28.2. With asynchronous replication a failover could roll back a fence advance; this is why the standby must be synchronous, and why Spike S11 tests it.
- **Redis is not in the correctness path.** It may serve a fast `409` pre-check and the event replay stream (§15.2). Flushing or failing over Redis cannot create two writers. Postgres advisory locks are not used (session-level, unsafe with pooled connections).
- **Bounded transactions.** The lease statements run in their own short transactions with `lock_timeout = 2 s` and `statement_timeout = 2 s`. Every write transaction sets `lock_timeout = 2 s`, `statement_timeout = 5 s` and `idle_in_transaction_session_timeout = 5 s`, and the application aborts any write transaction open longer than 10 s, so a stalled writer cannot hold the row lock past the lease margin (30 s).
- **Slow write versus takeover.** The lease check is the transaction's first statement and takes the row lock until commit. A new owner's acquire waits on that lock (up to `lock_timeout`, then retries or returns `409`), so **a stale run cannot commit after takeover and two runs never interleave writes**. A write whose lease check ran while the lease was valid but which commits after the lease's nominal expiry is serialised before the takeover, never after it. A write that starts after takeover or after expiry finds 0 rows and aborts. Spike S11 includes these three cases.
- **Heartbeat margin.** Heartbeats at 10 s against a 30 s lease tolerate two lost heartbeats. A run that misses its heartbeat treats the lease as lost and stops; a stalled pod therefore cannot write after expiry.
- **Tests (S11 and CI).** (1) Two runs racing to acquire: exactly one wins. (2) Force-expire the lease, let run B acquire, then run A attempts a write: rejected. (3) The fence strictly increases across 1,000 acquire and release cycles and across a Postgres failover drill. (4) Flushing Redis mid-run changes nothing about who may write. (5) A write transaction delayed across lease expiry while another run tries to acquire: the acquire waits, the delayed write commits first or aborts, and there is no interleaving. (6) A write that begins after takeover aborts. (7) A lease statement that cannot get the row lock within `lock_timeout` fails cleanly.
- A janitor job marks runs whose lease has expired as `aborted` and releases their idempotency keys.

## 10. Memory and session state

### 10.1 Layers

**Release 1:** there is no long-lived semantic memory and no `memory_items` table. Only the bounded conversation history and the pending clarification exist. Older evidence is never trusted just because it appeared earlier; reuse needs current ACL, version, freshness, authorisation epoch and citation revalidation (§10.4), and is switched off until its own gate passes (§24.1).

| Layer | Content | Stored in | Lifetime |
| --- | --- | --- | --- |
| Recent turns | Last 3 to 5 messages verbatim | Postgres `messages` | With the conversation |
| Rolling summary | Summary of older turns | Postgres `conversation_memory` | Regenerated every N turns; expires with the conversation |
| Focus slots | `active_documents`, `mentioned_entities`, `last_run_id`, `last_evidence_refs` | Postgres (durable), Redis (hot) | With the conversation |
| Evidence references | Evidence IDs and anchors, not copies of text | Postgres `run_evidence` | Retention window (§16.8) |
| Run state | Full retrieved text, tried pairs, budget, stream buffer | Redis | Minutes (run TTL) |

### 10.2 Scope and provenance rules

- Every memory record carries `tenant_id`, `user_id`, `conversation_id`, `kind`, `source_run_id`, `created_at`, `expires_at`. Queries always filter by all three identifiers inside a tenant transaction (§16.3). Memory is never shared across users or conversations.
- Summaries are generated only from user messages and **verified** assistant answers. Raw retrieved or web text is never copied into memory. Evidence is stored by reference (document version ID and anchor).
- Memory content is treated as untrusted-derived data when placed in a prompt: delimited and covered by the same injection rules as evidence (§16.6). A summary job screens output for instruction-like text and drops it.
- Summaries have a maximum length. Facts in memory are never used as grounding for factual claims; only evidence is. A claim based only on memory is not citable and is handled as model-knowledge or re-retrieved.

### 10.3 Standalone query and focus resolution

Every turn the Router turns the message plus focus slots and summary into a standalone query. Example:

```text
T1: What is this document about?        focus=doc_17; retrieve summary chunks
T2: What are its main risks?            "its" -> doc_17; new retrieval filtered to doc_17
T3: Which section has the highest risk? standalone from T2 answer; REUSE_EVIDENCE likely passes
T4: Explain that risk simply.           REUSE_EVIDENCE passes; no retrieval; rewrite for a lay audience
```

### 10.4 Evidence reuse: revalidation and coverage check

Reuse is allowed only if all checks pass; any failure means normal retrieval.

| Check | Rule |
| --- | --- |
| Access | The principal still has access to every source document (fresh ACL check) |
| Version | Every referenced `document_version_id` is still current and not deleted |
| Age | Evidence age is within the tool's `freshness.ttl`; live or API sources are not reused beyond their TTL |
| Coverage | Every required sub-question of the new standalone query has a stored evidence item above the calibrated rerank floor when scored against the new query. The reranker (§12.6) is re-run on the stored passages at temperature 0; this costs one model call |
| Quote check | The Evaluator rule (§5, B) is applied with stored passages; if the Evaluator is needed to map sub-questions it runs in a single-item "lite" call |

### 10.5 Expiry and cleanup

Pending clarifications expire (§11). Summaries and focus slots are deleted with the conversation. Evidence references follow retention (§16.8). A background job removes expired rows; deletion of a user or tenant cascades to all memory records.

## 11. Clarification lifecycle (`ask_user` and resume)

### 11.1 Creating a pending clarification

When the verdict or the router's plan is `ask_user`, the `ASK_USER` node:

1. Produces one short question (and optional choices) from `clarifying_question` or the unsupported sub-questions.
2. Persists a `pending_clarifications` row: `id`, `tenant_id`, `user_id`, `conversation_id`, `run_id`, `question`, `reason`, `options`, `plan` (router plan with sub-question states), `standalone_query`, `evidence_refs`, `tried_pairs`, `attempts_used`, `chain_depth`, `created_at`, `expires_at`, `status = open`.
3. Emits a `clarification` event and ends the run with outcome `ask_user`.

### 11.2 Resuming

On the next message in the conversation the rule pre-router loads the open row (one open row per conversation at most) and the Router classifies the reply.

| Reply type | Action |
| --- | --- |
| Answers the question | Merge into the saved plan and standalone query; mark `resolved`; resume at `PLAN` with `tried_pairs` carried over and `resume_max_attempts` budget; `chain_depth` incremented |
| Starts a new topic | Mark `abandoned`; process as a new question |
| Declines to answer, or says to guess or proceed anyway | Mark `resolved`. **An assumption is not evidence.** If the question is general or outside the documents, return `refer_to_owner`. If the ambiguity was only about interpretation, proceed under the stated interpretation and still require evidence for every claim. If it needs internal records, policies or other grounded claims, do not guess: return `fail_gracefully(evidence_required)` stating what information or source is needed |
| Row expired (default 24 h) | Mark `expired`; treat as new |

### 11.3 Safeguards

- At most one clarification per question chain (`chain_depth` limit 1): a resumed run answers or fails gracefully; it cannot ask again.
- The row is bound to the tenant, user and conversation. Another user cannot resume it.
- The reply text is untrusted: it is merged by the Router into a validated plan and is not executed as instructions.
- If the access level or the document version changed between turns, the resumed run revalidates (§10.4) before reusing any carried evidence references.

## 12. Retrieval and ingestion

### 12.1 Ingestion pipeline (with dead-letter handling)

```text
POST /v1/documents -> validate (type, size, magic bytes, malware scan) -> hash -> dedupe check
 -> store original in object storage -> insert document + document_version (state=queued) + ingestion_job
 -> worker claims job (FOR UPDATE SKIP LOCKED, visibility timeout)
 -> stages, each idempotent and keyed by (document_version_id, stage):
      parse -> clean -> structure detect -> chunk -> enrich metadata -> embed (batched, cached by chunk hash)
      -> write chunks + vectors + tsvector under visible=false
 -> quality checks (chunk count, empty text ratio, language)
 -> publish: ONE transaction sets version.state=ready, flips is_current, marks chunks visible, supersedes the old version
 -> on failure: retry with backoff up to max_attempts; then move to ingestion_dead_letter with stage, error, payload reference
```

Rules:

- **Accepted formats (release 1).** Text-extractable PDF, `.docx` and `.pptx` are parsed natively. Legacy `.doc` and `.ppt` are converted to `.docx` and `.pptx` by an isolated converter sidecar (LibreOffice headless **[verify]**, approval OI-10) running in a container with no network, a read-only filesystem and CPU, memory and time limits; the original is kept and the converted copy is hashed. A PowerPoint file produces one parent per slide (title, body and speaker notes); tables are chunked row-group-wise. Charts and pictures without extractable text are skipped and reported on the document. Scanned PDFs are rejected as `needs_ocr`. Text detected as non-English is rejected as `unsupported_language`.
- **Atomic visibility.** A version is searchable only after the publish transaction commits. A crash at any earlier stage leaves invisible rows that a cleanup job removes. Reprocessing deletes the version's invisible rows first (idempotent).
- **Dead letter.** After the maximum attempts, the job and its error go to `ingestion_dead_letter`; the document shows `failed` with a reason code; an operator can replay or discard. Alerts fire on dead-letter growth.
- **Parser limits.** Maximum file size, page count, decompressed size (zip-bomb protection), per-file CPU time and memory; parsing runs in a worker process with resource limits or in a sandboxed sidecar. A parser crash fails one job, never the worker fleet.
- **Reprocessing.** A new embedding model or chunking change creates a new derived index generation (§18.3); live search keeps reading the old generation until the new one passes the golden set.

### 12.2 Data model, identifiers and ACLs

| Entity | Key fields |
| --- | --- |
| `documents` | `id`, `tenant_id`, `title`, `source_uri`, `current_version_id`, `deleted_at` (tombstone), `acl_version` |
| `document_versions` | `id`, `document_id`, `version_no`, `content_hash`, `object_key`, `normalized_text_key`, `state` (`queued/processing/ready/failed/superseded/deleted`), `created_at` |
| `parents` | `id`, `document_version_id`, `section_path`, `page_start`, `page_end`, `char_start`, `char_end` |
| `chunks` | `id`, `parent_id`, `document_id` (immutable copy), `document_version_id`, `tenant_id`, `ordinal`, `text`, `char_start`, `char_end` (offsets into the version's normalised text), `page`, `content_hash`, `tsv`, `embedding`, `embedding_model_id`, `allowed_principals text[]` (**index prefilter only, never authority**), `acl_version`, `visible` |
| `acl_entries` | `tenant_id`, `document_id`, `principal_id` (text in the same form as `app.principal_ids()`: `user:{id}`, `group:{id}`, `role:{id}`, `tenant:{id}:all`), `granted_at`. Indexed on `(document_id, principal_id)` and `(principal_id)` |

- **Stable identifiers.** Version text is immutable. A chunk is identified by `chunk_id`; a citation location is `(document_version_id, char_start, char_end)`. Offsets refer to the stored normalised text, so a citation resolves identically forever (§14).
- **ACL enforcement.** The authority is `acl_entries`, `documents.deleted_at` and `documents.current_version_id`. (1) The RLS policy on `chunks` evaluates all three **live, inside the database** (§16.3), so a chunk the principal may not see is never returned to application code, including right after a revocation or a deletion. (2) The search SQL repeats `allowed_principals && $principal_ids` **only as an index-friendly prefilter**. That column can lag: lag can hide a newly granted document until re-propagation (target at most 60 s, alert at 5 minutes), which is an availability effect; it can never expose a revoked row because (1) still applies. (3) `ACL_RECHECK` remains as defence in depth for rows that reach the run **without passing through the policy in the current transaction**: cached result IDs, reused evidence and memory. It is not the primary revocation control. (4) Citation resolution re-checks (§14.3).
- **Deletion.** Setting `deleted_at` hides the document at once (filter plus ACL recheck). A purge job then removes chunks, vectors, derived caches and object-storage files and keeps citation tombstones ("source removed") for old messages (§16.8).
- **Versions.** Only the `current` and `ready` version is searched. Release 1 does **not** support pinning a conversation to an older version; it can be added later only with its own reviewed provenance and authorisation contract.

### 12.3 Chunking

Structure-aware (headings, tables and lists kept intact). Child chunks of 200 to 400 tokens for matching, mapped to parents of about 800 to 1,200 tokens returned as context, 10 to 15% overlap for unstructured text. Tables are chunked row-group-wise with the header repeated. Chunking parameters are versioned (§18) and evaluated on the golden set before change.

### 12.4 Vector search (and the filtered-search caveat)

- Store: PostgreSQL with `pgvector` and an HNSW index. Cosine distance. Vectors are `vector(768)` (nomic-embed-text-v1.5); documents are embedded with the prefix `search_document: ` and queries with `search_query: `. At the assumed corpus size (200,000 chunks, about 0.6 GB of raw vectors) exact filtered search may already be fast enough, which Spike S2 decides.
- **Caveat.** An approximate index scan that is followed by a filter (tenant, ACL, document scope, `visible`) can return fewer than K results and lose recall, especially for selective filters. Mitigations, chosen by a **filter selectivity planner**:
  - Very selective filters (candidate chunk count below a threshold, for example under tens of thousands): run an **exact** scan over the filtered set.
  - Otherwise: HNSW with a raised `ef_search` and, where supported, pgvector's iterative index scan **[verify the pgvector version and setting names]**.
  - Large tenants: partial indexes or partitions per tenant.
- Stage 0 spike S2 measures recall@K under realistic filters against an exact-search ground truth. If recall is unacceptable, move vectors to a dedicated engine behind the `VectorIndex` trait (§25).
- Tenant isolation holds regardless of index choice because filtering and RLS are applied in the same transaction (§16.3).

### 12.5 Keyword search (honest naming)

Postgres full-text search with `ts_rank_cd` is **not BM25**. v5 calls it *keyword search*. It is good for exact terms, IDs and acronyms but its ranking is weaker than BM25. Upgrade path, only if the golden set shows keyword recall is the bottleneck: a BM25-capable engine (a Postgres BM25 extension, or an external Tantivy-based index) behind the same `KeywordIndex` trait **[verify extension maturity before adoption]**. Language configuration is stored per document (§12.8).

### 12.6 Fusion and reranking

```text
Standalone query -> filters (tenant RLS, ACL, doc scope, visible, current version)
 -> in parallel: vector top-N (20 to 30) + keyword top-N (20 to 30)
 -> Reciprocal Rank Fusion (k = 60, weights tunable)
 -> LLM listwise rerank (Nemotron 3 Super) of the fused top 20; each passage gets a relevance grade 0 to 3
 -> calibrated floor -> keep 5 to 8
 -> expand to parent sections and neighbours -> Evidence objects
```

**Reranker grade calibration.** The model returns a grade from 0 to 3 for each candidate in one schema-constrained call. Grades are model-specific and are not probabilities. The floor (default: grade at least 2) is **fitted on the development split of the golden set** to hit a target precision, versioned with the model revision, and invalidated by any model or prompt change (§18). Borderline grades are left to the Evaluator. If Spike S15 shows the LLM rerank does not improve nDCG@10 by at least 0.03, or its latency or cost is too high, the fallback is a local cross-encoder behind the same `Reranker` trait.

### 12.7 Expansion

`retrieve_more` doubles K, adds neighbouring chunks (ordinal ±1 or 2) and the parent section, within the token budget.

### 12.8 Languages and parsing choices

- Release 1 is **English only**: the embedding model is English-only and document language is checked at ingestion (§12.1). Full-text search uses the `english` configuration.
- Parsing in Rust covers text-based PDF, DOCX and PPTX (via zip and XML). Legacy `.doc` and `.ppt` go through the converter sidecar. Complex tables may need a parsing sidecar; OCR is not in release 1. The `Parser` trait hides the choice. Spike S1 compares parsers on the real corpus before the choice is frozen.

## 13. Generation, grounding and streaming

### 13.1 Modes

| Mode | Behaviour | Use |
| --- | --- | --- |
| `strict` | Buffer the whole answer, verify every unit, regenerate once if checks fail, then stream only verified text | Regulated or high-risk tenants |
| `gated` (default) | **Unit-gated verified streaming.** Each claim unit is verified before it is emitted. Nothing unverified is ever emitted | Default |
| `fast` | Units are emitted after the parse and the cheap deterministic ID check only; entailment runs after the stream and results are shown as flags. Opt-in per tenant, labelled "verifying" in the UI | Low-risk, latency-sensitive uses |

**There are no ungrounded answers in release 1** (§3.4). Modes enabled in release 1: `gated` (default) and `strict`; `fast` is off. **`gaps` answers** (insufficient evidence) go through the gate like any grounded answer, so the "what I found" part is verified.

### 13.0 Generation format: self-contained claim units

In grounded modes the generator does not write free prose. It emits **claim units**, one per line, as JSON Lines. If Spike S7 shows the provider cannot stream JSON lines reliably, release 1 uses the provider's non-streaming structured-output mode with `strict` buffering instead; it does not switch to a second streaming format:

```json
{"u":1,"role":"answer","text":"Refunds are accepted within 30 days of delivery.","cites":[1],"reqs":["r1"]}
{"u":2,"role":"support","text":"Digital goods are refundable only if not downloaded.","cites":[3],"reqs":["r2"]}
{"u":3,"role":"caveat","text":"The 2024 policy lists 14 days for sale items.","cites":[2],"reqs":["r1"]}
```

**Release 1 supports JSON Lines only.** There is one canonical format per release; a second format needs a new release, its own tests and a new golden-set run. Each line is one JSON object with exactly the fields `u` (integer), `role` (`answer`, `support` or `caveat`), `text` (at most 400 characters and two sentences) and `cites` (an array of 1 to 8 integers, each an evidence number present in the prompt) and `reqs` (an array of 1 to 5 requirement IDs from the validated plan; an unknown ID makes the unit invalid). Unknown fields are rejected (`deny_unknown_fields`), and **Invariants enforced in code:** unit 1 must have `u = 1` and `role = answer`; every later unit must have `u` exactly one greater than the previous and `role` of `support` or `caveat` (a second `answer` is invalid); at most `max_units` units (default 12). A malformed, oversize, out-of-order, duplicate or invariant-violating line is a **failed unit**. If unit 1 fails, nothing is emitted (answer-first rule); a failed later unit is withheld and the stream continues only while the invariants for later units can still be applied to the following lines. This is the only format the gate reads: it does **not** split sentences and does **not** read inline `[n]` markers. Every unit is **self-contained** (no unresolved pronouns, no "as above", no dependence on another unit to be understood).

**No model-controlled exemption.** The model cannot emit `meta`. Every unit it emits is treated as factual, needs at least one citation, and goes through C1 to C4. There is no classifier that decides whether a unit "needs" a citation, and no role is exempt: a unit with an unknown role or empty `cites` is withheld. Text that cannot be cited (conversational framing, greetings) is not allowed in grounded mode.

**System-generated statements.** The only uncited text in a grounded answer is produced by code from fixed templates, never by the model: the gap statement ("I could not find information on X", built from sub-question status `Missing`), the blocked-source statement (from `Blocked`), the neutral caveat appended when a caveat unit is withheld (§13.2), and the `low_confidence` banner. Each carries `origin=system` in the stream and states only what the system did or failed to find, never a claim about what the sources say.

**Answer-first rule.** Nothing is emitted until unit 1 passes the checks. If it fails, nothing has been sent yet: regenerate once (strict style), and if it fails again return `fail_gracefully(verification_failed)` with the verified partial findings. Later units that fail are withheld; because units are self-contained, omitting them does not break the reading of the rest.

### 13.2 The Grounding Gate (gated mode)

```text
LLM token stream (cancellable)
 -> line framer (one JSON object per line; maximum line bytes and maximum unit count)
 -> for each complete line:
      P0  parse and schema-validate (§13.0): fields; invariants (u = 1 with role answer first, then consecutive u with role support or caveat); cites non-empty integers; every reqs ID exists in the plan
      P1  hygiene: remove bracketed numbers and URLs from `text` (never trusted as citations; counted as `format_violation`)
      C1  every id in `cites` is in this run's approved evidence set
      C2  cited evidence was marked relevant by the Evaluator
      C3  numbers, dates, currencies and named entities in `text` appear in the cited evidence (normalised match)
      C4  entailment: local NLI cross-encoder on (cited passages -> text) >= calibrated threshold
      C5  `cites` is non-empty for every model unit (enforced at P0; no role is exempt)
    all pass            -> emit the unit as a `token` event carrying its resolved citations
    any fail            -> withhold; record {unit, failed_check}; continue
 -> end of stream: emit system-generated statements (gaps, caveat), then `citations`, then `verification`, then `done`
```

Effects and trade-offs, stated plainly:

- **Latency.** First visible text arrives after the first complete unit plus its check (about 50 to 300 ms on top of generation). It is not token-by-token. `status` events cover the retrieval and evaluation wait.
- **Withheld units and coherence.** Because units are self-contained (§13.0) and the answer unit is checked first, withholding a later unit cannot invalidate the answer. If a withheld unit has `role=caveat`, the gate appends a fixed neutral caveat ("Some details could not be confirmed against the sources") so a conflict or limit is never silently lost. If more than 40% of units are withheld, the response carries a `low_confidence` flag and the UI shows a banner. The summary event reports how many units were omitted. The withheld rate is a tracked quality metric; high rates trigger prompt or threshold review (§19.4).
- **No retraction needed.** Because nothing is emitted before it passes, the system never has to "unsay" text.
- **Thresholds.** The NLI threshold and the grounding rules are calibrated on the golden set to keep the false-withhold rate (valid units dropped) below a target (for example 5%) and the false-pass rate (unsupported units emitted) below a stricter target. NLI accuracy on long passages, tables and numbers is a known weak point **[verify on the real corpus]**; C3 and an optional LLM-judge escalation in `strict` mode compensate.
- **Cost.** NLI runs in-process on CPU (ONNX), batched per unit (§20.5). It adds no provider LLM call in `gated` mode.

### 13.3 Strict mode

Buffer the full answer, verify all units, then (a) if all pass, stream it, (b) if some fail, regenerate once with the failed claims listed as constraints, re-verify, and stream; remaining failures are withheld and reported. The user sees `status` events while this runs.

### 13.4 Failure and cancellation behaviour

| Event | Behaviour |
| --- | --- |
| Client disconnects or is too slow | Keep generating and persisting verified units for the 10 s `reconnect_grace`. The client may reconnect up to 3 times per run and resumes after its last event (§15.3). With no reconnect in the grace, or after the third reconnect fails, cancel the LLM stream and gate and persist the partial trace |
| Invalid stream data (malformed unit, schema or invariant failure) before any answer text was shown | Abort the generation and retry, up to 3 retries (`max_generation_retries`), each reserving budget and respecting the deadline; then `fail_gracefully(verification_failed)` |
| Invalid stream data or a provider failure **after** answer text was shown | **No retry.** End with a terminal `incomplete` marker and the verified units already sent; outcome `upstream_failure(provider)` or `fail_gracefully(verification_failed)` |
| LLM host fails before the first token | Retry the same host up to 3 times (before any text is shown), then `upstream_failure(provider)`. There is no secondary provider in release 1 (§17.3) |
| Provider fails after the first unit was emitted | End with an `error` event; keep the partial message marked incomplete; the run outcome is `upstream_failure(provider)`, not `internal_error`, so alerts, user messaging and reliability metrics separate provider outages from our own defects |
| Gate model (NLI) unavailable | `strict` and `gated` switch to buffering plus the LLM-judge fallback, or fail with `verification_failed`; never silently skip verification |
| Every unit withheld | Outcome becomes `fail_gracefully(verification_failed)` with the verified partial findings |
| Unsupported claims in `fast` mode | Flagged after the stream; stored; counted |

### 13.5 Assurance language, risk tiers and limits

- **Terminology.** Throughout this document "verified" means "passed the grounding screen". The screen combines deterministic checks and a local entailment model. It reduces unsupported claims to a **measured bound**; it does not prove that a claim is true. Entity and number matching and an NLI threshold can still pass unsupported claims, especially for tables, negation, claims that combine several sources, and long passages. User-facing text says "checked against sources", never "verified true".
- **Risk tiers (per tenant, with per-request escalation).**

| Tier | Mode and checks | Target bound on unsupported claims that pass (measured on the golden set's unsupported-claim slice, upper confidence bound) |
| --- | --- | --- |
| `standard` | `gated`: C1 to C5 | At most 1% in production and 2% in staging (D7) |
| `high_risk` (**off in release 1**) | `strict`: C1 to C5, plus an LLM judge (a different model from the generator) on every unit, regenerate once, withhold the rest | At most 2% |
| `regulated` (**off in release 1**) | `high_risk` plus a human review queue for sampled and flagged answers, and an "AI-assisted, review sources" label | At most 1%, plus human sampling |

- A tier cannot be enabled for a tenant until its bound is demonstrated on the golden set (§19.8) and re-checked at each release. The `high_risk` and `regulated` tiers need a judge from a different model, which a single-model deployment (Nemotron 3 Super for answering, reranking and evaluation, D5) cannot provide, so they stay off until a second model is approved. **Judge independence is limited:** the generator, reranker and evaluator share one model family and can make correlated mistakes, which is why the code checks (exact quote, ID, number and entity checks) and the separate local NLI model carry the safety bound, not the evaluator's opinion. If Spike S3 shows the local NLI cannot meet the `standard` bound, the default becomes `strict` buffered mode with a second pass by the same model under a different prompt plus the NLI check, reported as lower assurance (§24.4).
- Escalation: queries touching configured high-risk topics (legal, medical, financial figures), or any answer with `low_confidence`, are promoted one tier for that request.

## 14. Citations

### 14.1 Citation contract

Every evidence item carries a stable, resolvable anchor:

```rust
pub enum SourceAnchor {
    Document { document_id, document_version_id, chunk_id, page: Option<u32>,
               char_start: u32, char_end: u32, content_hash: Hash },
    Api      { tool: String, resource_uri: String, request_hash: Hash, response_hash: Hash,
               field_path: Option<String>, retrieved_at: OffsetDateTime },
    Web      { url: String, fetched_at: OffsetDateTime, content_hash: Hash, snapshot_key: String },
    Db       { tool: String, query_template_id: String, params_hash: Hash, row_key: Option<String>,
               as_of: OffsetDateTime },
}
```

- **Documents.** `(document_version_id, char_start, char_end)` points into immutable normalised text, so the highlight is reproducible. `content_hash` detects corruption.
- **Web.** A snapshot of the fetched content is stored in object storage so the citation can be verified later, even if the page changes.
- **API and DB.** The request and response hashes plus the field path allow audit; the sanitised response needed to show the citation is stored with retention limits.
- The model sees evidence numbers `[n]` in the prompt and returns them only as integers in a unit's `cites` array. It never writes metadata. The citation binder maps each integer to its anchor after validation, so a fabricated citation cannot resolve. Markers shown to the user are **generated by the UI from `cites`**; bracketed numbers inside the model's `text` are stripped (P1) and never parsed as citations.

### 14.2 What counts as valid support

A citation is valid for a unit only if the parse (P0) and checks C1 to C5 of the Grounding Gate pass (§13.2). In summary: the unit is well-formed, every cited ID exists, the evidence was approved, the unit's numbers and entities are present in it, an entailment model finds the passages support the text, and the unit has at least one citation.

| Situation | Behaviour |
| --- | --- |
| Cited ID does not exist or is outside the approved set | Unit withheld (C1) |
| IDs valid but fail C2 to C4 | Unit withheld (`gated`, `strict`) or flagged (`fast`) |
| Empty `cites`, unknown role, `meta` role, or schema failure | Failed unit, withheld; counted in `format_violation` |
| Bracketed numbers or URLs inside `text` | Stripped (P1); the unit is judged on its structured `cites` only |
| Unit cites several sources | Entailment is checked against the concatenated cited passages (windowed); the supporting set is stored |
| Source later deleted or changed | The stored anchor resolves to a tombstone ("source removed") or the old immutable version text |

### 14.3 Resolution API and display

`GET /v1/citations/{id}` re-checks the caller's ACL, then returns the passage text from the immutable version (or snapshot) with highlight offsets and the page. The UI shows the numbered marker (rendered from `cites`), the source title and page, and the highlighted span.

### 14.4 Tests

- Unit tests for each check (C1 to C5) with adversarial fixtures: fabricated IDs, citations to irrelevant evidence, correct IDs with wrong numbers, negations, truncated passages.
- Golden-set metrics: citation accuracy, unsupported-claim recall (does the gate catch known-bad units), and false-withhold rate.
- Property tests: for any model output, no emitted unit has an unresolvable citation, an empty `cites`, or a role outside the schema.
- **End-to-end format test (CI).** A recorded provider stream of JSON-line units is replayed through the framer, parser, gate, citation binder, SSE encoder and the web client's renderer; the rendered text and citation markers must equal the units' `text` and `cites` exactly, for answer, support and caveat units, withheld units, and system-generated statements. A change to the format in any one component fails this test.
- Gate-parser fuzzing (Spike S12): malformed JSON, oversize lines, duplicate or decreasing `u`, injected `[n]` text, a model-supplied `meta` role, empty `cites`; none may produce an emitted unit.
- Regression suite built from production failures (P21).

## 15. API

### 15.1 Endpoints

| Endpoint | Purpose |
| --- | --- |
| `POST /v1/chat` | Non-streaming chat; body: `conversation_id`, `message`, optional `document_ids`, `options` (`sources`, `max_attempts` up to 5, `mode`) |
| `POST /v1/chat/stream` | Same, via SSE |
| `POST /v1/runs/{id}/cancel` | Cancel an active run |
| `GET /v1/runs/{id}` | Run summary and trace (authorised users or admins) |
| `GET /v1/runs/{id}/events` | Replay events after `Last-Event-ID` (release 1; used for reconnects) |
| `POST /v1/search` | Retrieval only (hybrid plus rerank), for debugging and non-chat clients |
| `POST /v1/documents`, `GET /v1/documents`, `GET /v1/documents/{id}`, `DELETE /v1/documents/{id}` | Upload (async ingestion), list, status, delete |
| `GET /v1/citations/{id}` | Resolve a citation (§14.3) |
| `POST /v1/conversations`, `GET /v1/conversations/{id}`, `GET /v1/conversations/{id}/messages` | Conversations and paged history with citations and verification flags |
| `POST /v1/feedback` | Rating linked to a run |
| `GET /v1/tools` | Tools enabled for the caller |
| `GET /health`, `GET /ready`, `GET /metrics` | Probes and Prometheus metrics (metrics not publicly exposed) |

Cross-cutting: OIDC authentication (JWT verified with the provider's keys), request IDs, uniform error envelope, size limits (message length, upload size), rate limits per user and tenant, idempotency keys on chat and uploads.

### 15.2 SSE contract

| Event | Payload |
| --- | --- |
| `status` | Step label (for example "searching documents", "checking evidence", "trying another source") |
| `token` | `{seq, unit, role, origin, text, cites}`: one verified claim unit per event in grounded modes (`origin` is `model` or `system`; `done` also carries `coverage`: `complete` or `partial`; `cites` are resolved citation IDs) |
| `citations` | Resolved citation objects (document, version, page, offsets, title) |
| `verification` | `{mode, withheld, flagged: [...]}` |
| `clarification` | `{question, options}` |
| `done` | `{run_id, outcome, reason?, attempts_used, coverage, incomplete}` |
| `error` | `{code, message, run_id}` (safe text only) |

Each event has an increasing `id`. Verified units and terminal events are persisted in the Postgres `run_events` table (retained 24 hours, verified text only) so a client can reconnect with `Last-Event-ID` and receive missed events. A run allows at most 3 reconnects (release 1).

### 15.3 Streaming mechanics (Axum and SSE)

- The run task writes events into a **bounded** `tokio::sync::mpsc` channel. The response is `axum::response::sse::Sse` over the receiver, with `KeepAlive` comments every 15 s.
- **Backpressure.** If the channel is full (slow client), the run waits with a send timeout. On timeout (default 5 s) the client is treated as disconnected and the run enters the reconnect grace below. The channel capacity and timeout are configuration.
- **Disconnect detection.** The response stream holds a `DropGuard` of the run's cancellation token. When the connection closes the run is **not** cancelled at once. It enters `reconnect_grace` (10 s), during which verified units keep being generated and written to `run_events`, bounded by an event cap. A client may reconnect up to 3 times per run and resumes after its `Last-Event-ID`. If no reconnect arrives within the grace, or the client drops again after its third reconnect, the guard cancels the run, tool calls and the LLM stream stop, and the partial trace is persisted.
- **Proxy behaviour.** Disable response buffering at reverse proxies (for example `X-Accel-Buffering: no`), raise idle timeouts above the keep-alive interval, and test HTTP/1.1 and HTTP/2 (spike S5).
- **Client.** The browser uses a **fetch-based SSE client** (not the native `EventSource`), because requests are `POST` and need an `Authorization` header. It handles reconnection with `Last-Event-ID`.
- **Graceful shutdown.** `with_graceful_shutdown` stops accepting new runs, lets in-flight runs finish up to a timeout, then cancels the rest and persists their traces.
- **Middleware contract for streaming routes.** Streaming routes live in their own `Router` with these layers only: request ID, tracing, catch-panic, authentication, rate limiting (checked before the stream starts). They have **no** compression layer, **no** total-request `TimeoutLayer` (the run deadline and an idle-write timeout of three keep-alive intervals apply instead), and no body buffering. Responses carry `Content-Type: text/event-stream`, `Cache-Control: no-cache, no-transform` and `X-Accel-Buffering: no`. Reverse proxy and load balancer: response buffering off, read and idle timeouts above 120 s, no CDN in the path.
- **Deployed-stack tests (CI against staging, gate G4).** (a) The first event arrives within the target, proving there is no buffering. (b) Killing the client mid-stream with no reconnect cancels the run within `reconnect_grace` plus 2 s (12 s), checked through run status and metrics. (c) A slow reader triggers the backpressure timeout and the reconnect grace; a reader that recovers within the grace resumes without a lost or duplicated unit. (d) A silent period of 30 s does not drop the connection because keep-alives flow. (e) The stream is not compressed. (f) HTTP/1.1 and HTTP/2 both pass. (g) Up to 3 reconnects resume correctly from `Last-Event-ID`; a fourth is refused with a clear message.
- **Fallback transport.** If the deployed stack cannot pass (a) to (f) (Spike S5), the client falls back to `GET /v1/runs/{id}/events?after=<id>` long-polling over the same `run_events` log, with no change to the server run logic.

## 16. Security, tenancy and data lifecycle

### 16.1 Threats considered

Cross-tenant data leakage, privilege escalation through the agent, prompt injection through documents, web pages, tool output and memory, SSRF and data exfiltration through tools, SQL abuse, credential theft or misuse, cache poisoning and cross-principal cache hits, log and trace leakage, supply-chain compromise, cost abuse and denial of service.

### 16.2 Authentication and authorisation

- **OIDC.** The token is verified with the provider's published keys (cached with rotation): signature, issuer, audience, expiry. Access-token lifetime is at most 5 minutes. Tenant and user come from the verified token and never from the request body or model output. The API derives one `Principal` per request. The Principal is the only source of identity for downstream code.
- **Membership freshness.** Effective groups and roles come from the OIDC provider through a `MembershipProvider` trait, by one of two routes: (a) the claims of a token whose `iat` is within the last **300 s**, accepted only if the provider is configured to re-evaluate membership each time it issues a token (verified in Spike S16, open item OI-8); (b) otherwise a call to the provider's `userinfo` endpoint. The result is cached per user in Redis with its `observed_at`, for at most 300 s. A membership-change event, if the provider can send one, deletes the entry and bumps the tenant `authz_epoch` (§16.4). **The maximum revocation delay is 5 minutes** and is published to tenants.
- **Provider unavailable or context older than 5 minutes.** Group and role grants are not used. The Principal is **degraded** to the user's own grants (`user:{id}`) and only those tenant-wide documents whose grant does not depend on membership; the response says some documents may be unavailable. There is **no stale allowance**. The age limit is checked before retrieval, when cached IDs become evidence, when evidence is reused, and when citations are resolved. A run that crosses the limit refreshes membership or stops. Degraded and fresh contexts have different `authz_ctx` hashes, so they never share cache entries.
- **Tests.** Forged tenant or group claims, expired tokens, wrong audience and wrong issuer are rejected. A user removed from a group stops matching group-restricted documents within 5 minutes (with and without events). A token whose `groups` claim is older than 300 s gains nothing. Provider outage uses the degrade path at the 5-minute mark in retrieval, cache, evidence reuse and citation resolution. Degraded and fresh contexts never share cache keys.
- Authorisation happens at request time in the API, in every tool, and in the data layer (RLS). A model decision never grants permissions.
- **Existence leaks.** Responses for "denied" and "not found" use the same user-facing wording (`no_accessible_information`) wherever distinguishing them would reveal that a resource exists. Internal audit logs keep the real reason.

### 16.3 Tenant isolation with pooled connections (Postgres RLS)

- Every tenant-scoped table has `ENABLE ROW LEVEL SECURITY` and `FORCE ROW LEVEL SECURITY`. Policies enforce **both the tenant and the principal's effective ACL**, not the tenant alone, and they fail closed when the settings are missing:

```sql
CREATE SCHEMA app;
CREATE FUNCTION app.tenant_id() RETURNS uuid LANGUAGE sql STABLE AS
  $$ SELECT NULLIF(current_setting('app.tenant_id', true), '')::uuid $$;      -- NULL when unset: comparisons are not true
CREATE FUNCTION app.principal_ids() RETURNS text[] LANGUAGE sql STABLE AS
  $$ SELECT COALESCE(string_to_array(NULLIF(current_setting('app.principal_ids', true), ''), ','), ARRAY[]::text[]) $$;

ALTER TABLE acl_entries ENABLE ROW LEVEL SECURITY;  ALTER TABLE acl_entries FORCE ROW LEVEL SECURITY;
ALTER TABLE documents   ENABLE ROW LEVEL SECURITY;  ALTER TABLE documents   FORCE ROW LEVEL SECURITY;
ALTER TABLE chunks      ENABLE ROW LEVEL SECURITY;  ALTER TABLE chunks      FORCE ROW LEVEL SECURITY;

-- A caller sees only the ACL rows whose principal is in its OWN effective set (user, groups, roles, tenant-wide).
-- Policy subqueries run under the caller's RLS, so this also stops
-- the checks below from enumerating other principals, and it needs no SECURITY DEFINER function.
CREATE POLICY acl_self ON acl_entries FOR SELECT TO app_role
  USING (tenant_id = app.tenant_id() AND principal_id = ANY (app.principal_ids()));

-- Documents: same tenant, not deleted, and the caller holds a live grant.
CREATE POLICY documents_read ON documents FOR SELECT TO app_role
  USING (tenant_id = app.tenant_id() AND deleted_at IS NULL
         AND EXISTS (SELECT 1 FROM acl_entries a WHERE a.document_id = documents.id));

-- Chunks: live ACL, deletion and current-version rules are evaluated by the database, not by application code.
CREATE POLICY chunks_read ON chunks FOR SELECT TO app_role
  USING (tenant_id = app.tenant_id() AND visible
         AND EXISTS (SELECT 1 FROM documents d        -- documents_read applies here: tenant, not deleted, live grant
                      WHERE d.id = chunks.document_id
                        AND d.current_version_id = chunks.document_version_id));   -- release 1: current version only

CREATE POLICY chunks_write ON chunks FOR ALL TO ingest_role
  USING (tenant_id = app.tenant_id()) WITH CHECK (tenant_id = app.tenant_id());
```

  `principal_ids` holds `user:{id}`, every `group:{id}`, every `role:{id}`, and `tenant:{id}:all` for tenant-wide documents. The API sets it from the authenticated Principal, never from request input. The live check is in the policies themselves; the denormalised `chunks.allowed_principals` is an index prefilter only (§12.2). Release 1 sets exactly three transaction-local values: `app.tenant_id`, `app.principal_ids` and the authenticated user ID; there is no version-pinning setting. Views used by application roles are `security_invoker`, and `SECURITY DEFINER` functions are forbidden outside the migration role.
- **Threat boundary (stated plainly).** Transaction-local settings plus RLS protect against **application mistakes**: a missing predicate, a repository that forgets a filter, an injected model or tool input that changes a query's `WHERE` clause. They are **not an independent defence if an attacker can run arbitrary SQL as the shared application role**, because that role can call `set_config` itself. Therefore: (1) no tool runs caller-influenced SQL on the application role; the SQL tool (§16.7) uses a separate read-only role and connection pool that has **no grants on the chunk, document or ACL tables**; (2) repository SQL is parameterised and built only with `sqlx` macros or `QueryBuilder`; (3) the injection suite attempts `set_config`, `SET`, `RESET`, `SET ROLE` and statement stacking through every user-influenced input and must fail every attempt; (4) a compromised API process is outside this control's scope (it already holds the credentials that set the Principal) and is addressed by network isolation, least-privilege credentials and the §28.9 review. **If the threat model must include a compromised application role** (decided in Phase 0), switch to per-tenant database roles entered with `SET LOCAL ROLE` after a database-verified signed claim, or to per-tenant databases for the affected tenants; this is the Spike S6 fallback.
- The application connects as a role that is **not** the table owner, has **no** `BYPASSRLS`, and is not a superuser. A separate migration role owns the tables.
- The tenant and principals are set **per transaction**, never per session: `SELECT set_config('app.tenant_id', $1, true), set_config('app.principal_ids', $2, true)` (transaction-local) at the start of each transaction. Because it is transaction-local, the value cannot leak to the next user of a pooled connection, including behind transaction-mode poolers.
- **Rust enforcement.** Repositories do not take a raw pool. They take `&mut TenantTx`, a type that can only be created by `TenantTxFactory::begin(principal)`, which sets `app.tenant_id` (and `app.user_id`) before returning. There is no public API to run tenant queries without it. A clippy-style lint or code review rule forbids using the raw `PgPool` outside the `store::admin` module.
- **Tests (CI-gated, run against a real Postgres with the production policies).** Each is a named test:
  1. Missing or empty `app.tenant_id` returns zero rows (fail closed). A malformed value makes the request fail with an error, never return rows.
  2. Missing or empty `app.principal_ids` returns zero rows from principal-gated tables.
  3. Tenant A cannot read or write tenant B rows through any repository function (property test over every repository method).
  4. **Within one tenant:** user U1 cannot read a document restricted to user U2; a user in group G1 cannot read a G2-only document; a role grant adds access and its removal removes it; tenant-wide documents are visible to all tenant principals.
  5. **Live revocation and deletion, tested at the SQL boundary (not through `ACL_RECHECK`).** (a) With `allowed_principals` deliberately left stale (still listing the revoked user), a plain `SELECT` on `chunks` as the revoked user returns zero rows on the next transaction. (b) A soft-deleted document and a superseded (non-current, non-pinned) version return zero chunk rows. (c) A newly granted user sees the document's chunks at the SQL boundary as soon as the `acl_entries` row commits; through the search path it appears after re-propagation, and that lag is measured. (d) A user can select only their own `acl_entries` rows; other principals' grants are not enumerable. (e) A cached result ID or reused evidence item for a revoked document is dropped by `ACL_RECHECK` (defence in depth for rows that bypass the policy in the current transaction).
  6. A pooled connection used by A then B shows no residue, including through a transaction-mode pooler.
  7. `FORCE ROW LEVEL SECURITY` holds for the table owner; the application role has no `BYPASSRLS`, is not a superuser and does not own tables (checked from `pg_roles` in CI).
  8. Every retrieval path, cache lookup, memory read and citation lookup is covered by tests 1 to 5.
  9. **Group, role and tenant-wide grants as production-like roles.** A member of group G1 sees a G1-granted document; a non-member does not; removing the user from G1 removes access within the §16.2 limit; a role grant and a tenant-wide grant behave the same way; the user cannot read ACL rows of principals outside their own set.
  10. **Ownership.** A user cannot read or write another user's conversations, messages, clarifications, runs, run events, idempotency records, evidence references or citations, in the same tenant or another.
  11. **Ingestion scope.** `rag_ingest` can touch only jobs it has claimed for the assigned tenant; it cannot read user conversations or change ACL rows.
  12. **Append-only and read-only tables.** `audit_events` accepts inserts but no update or delete from the application and ingestion roles; `feature_flags` and `tenant_config` are read-only for them.
  The expected results depend on the exact policy expressions above; the tests are written against those policies, so a policy change fails the suite until the tests are deliberately updated.
- The same transaction runs the vector search, so the filter, RLS and ACL predicates apply together (§12.4).
- **Performance and fallback (Spike S10).** The live check adds an indexed lookup per candidate row. At the §28.11 corpus (200,000 chunks, 3 tenants, ACL mix) and in a 2,000,000-chunk stress run, measure retrieval p95 and confirm with `EXPLAIN` that the `(document_id, principal_id)` index is used and that filtered HNSW recall (S2) still holds. If the overhead breaks the §6.5 retrieval budget after tuning, switch to the **version-match variant**: the policy requires `chunks.acl_version = d.acl_version AND chunks.allowed_principals && app.principal_ids()`, and every ACL change bumps `documents.acl_version` in the same transaction as the `acl_entries` change. Stale chunks then become invisible immediately (safe), at the cost of a temporary availability dip until re-propagation (P26), which must then meet the 60 s target. The live variant is the default because it has no such window. Either way revocation is enforced before text leaves the database.

### 16.3a Persistence and storage inventory with an access test for every item

A CI job compares the migration-generated list of relations, functions and views with this table; any relation not listed blocks the merge. Roles: `rag_app` (user requests), `rag_ingest` (ingestion worker), `rag_authz` (ACL and epoch changes, audited), `rag_migrator` (schema owner, deployment only, never an application credential). There are no application views, no materialised views and no `SECURITY DEFINER` functions in release 1. The only functions are `app.tenant_id()` and `app.principal_ids()` (`SECURITY INVOKER`, `STABLE`). `memory_items` is not created in release 1, and there is no `membership_snapshots` table: membership is cached in Redis (§16.2).

| Item | `rag_app` | `rag_ingest` | `rag_authz` | Policy in one line | Test |
| --- | --- | --- | --- | --- | --- |
| `tenants`, `tenant_config` | read own tenant, non-secret columns | read ingestion settings | write, audited | tenant match | 1, 3, 12 |
| `authz_epochs` | read current epoch | none | write | tenant match | 1, 3, 9 |
| `documents`, `document_versions`, `parents`, `chunks` | read through RLS | write for the assigned job | none | tenant, not deleted, current ready version, live grant, `visible` | 1 to 5, 9 |
| `acl_entries` | read entries for principals in its own set | none | insert, revoke, change | tenant match and principal in caller's set | 4, 5, 9 |
| `conversations` (with lease and fence columns), `messages`, `clarifications` | own rows | none | none | tenant and owner; writes also need the active lease and fence (§9.5) | 1, 3, 10 |
| `runs`, `run_events`, `idempotency_keys` | own rows | none | none | tenant and owner | 1, 3, 10 |
| `evidence_refs`, `citations` | own run; source ACL rechecked on read | none | none | tenant, owner, live grant on the source | 5, 10 |
| `ingestion_jobs`, `ingestion_dead_letters` | status only through the API | claimed jobs of its tenant | operator replay, audited | tenant and claim | 11 |
| `audit_events` | append | append | append | insert-only | 12 |
| `feature_flags` | read | read | none (deployment role writes) | read-only | 12 |
| Functions `app.tenant_id()`, `app.principal_ids()` | execute | execute | execute | missing or empty setting returns NULL or an empty array | 1, 2 |
| Object storage `originals/{tenant}/{document}/{version}/source` and `converted/...` | none (authorised proxy download only) | write, read for its job | none | tenant-scoped keys, no public URLs, encryption on | 3, 13 |
| Object storage `normalized/{tenant}/{document}/{version}/text` | read through the citation resolver after an ACL check | write | none | as above | 3, 5, 13 |
| Object storage `parser-temp/{job}/...` | none | read and write | none | expires within 24 h | 13 |
| Redis key families (rate counters, membership cache, caches of IDs and scores) | read and write | none | invalidate | every key carries tenant and `authz_ctx`; no durable source text; TTL set | 6, 14 |
| Logs, traces and metrics | write | write | write | identifiers and metrics only, no prompts or document text by default | 15 |
| Database backups and PITR archives | none | none | none | managed and encrypted, 35-day retention, restore re-applies the deletion ledger | §16.8 drill |

Added tests: **13** an object key of tenant A cannot be read with tenant B credentials and no object has a public URL; **14** a cache key built under one `authz_ctx` is never served under another, and no cache value contains source text; **15** a log and trace scan over a golden-set run finds no prompt text, document text, token or secret.

### 16.4 Cache isolation

All cache keys include `tenant_id`, an `authz_ctx` hash of the **complete effective authorisation context**, the `corpus_version`, and the relevant model and prompt versions: `t:{tenant}:z:{authz_ctx}:c:{corpus_ver}:...`, where `authz_ctx = H(tenant_id ‖ sorted(principal_ids: users, groups, roles) ‖ sorted(delegated_scopes) ‖ authz_epoch)`. The `authz_epoch` is a per-tenant counter incremented on any ACL, group-membership, role or policy change, so every affected cache entry is invalidated at once (conservative on purpose). Results from tools that use delegated credentials are cached only under a key that includes the `user_id` and a fingerprint of the delegated subject (never the token), with a TTL no longer than the tool's freshness TTL. Caches store IDs and scores, not raw text across principals, and **`ACL_RECHECK` always runs before any cached result is turned into evidence**, so a cache hit can never expose a document the caller may no longer see. Where an authorisation context cannot be represented in the key, that result is not cached. Embedding caches (keyed by content hash) are tenant-scoped too unless the content is public. Any change to a document or ACL bumps the relevant version, so stale entries are never served. Cache entries have TTLs.

### 16.5 Credentials and delegated access

- Each tool has its own credentials in a secret manager, loaded at start-up into memory types that do not implement `Debug` or `Serialize` (the `secrecy` crate pattern) and are zeroised on drop.
- (Later release; none in release 1.) User-delegated access (CRM, APIs on behalf of the user) uses a short-lived delegated token obtained by the API from the identity provider (token exchange) and kept only for the run. The model never sees tokens. Tool arguments cannot contain credentials: the schema forbids fields named like secrets and outputs are scanned (§16.6).
- Service accounts for read-only data sources have the minimum role and row/column restrictions; the SQL tool uses a dedicated read-only role.
- Egress is restricted to an allow-list per tool through a forward proxy.

### 16.6 Prompt-injection defences

| Control | Detail |
| --- | --- |
| Data, not instructions | Evidence, tool output, web content, memory summaries and user-supplied documents are wrapped in delimited blocks; delimiter-like text inside is escaped; the system prompt states they are data and must never be followed as instructions |
| Authority separation | Evidence is never placed in the system role. Only the Orchestrator chooses tools, from the router plan produced **before** untrusted content is seen on the first attempt |
| Retrieved text cannot authorise tools | Later attempts choose tools only from a fixed enum returned by the Evaluator and validated against the registry and the principal's scopes. `suggested_query` is length-limited and sanitised and is used only as a search query |
| Typed extraction for multi-hop | Values passed from one hop to the next (IDs, dates) come from a deterministic extractor with strict format rules (regex or parser), not from free model text; the receiving tool re-authorises the principal |
| Read-only tools | No tool has side effects (§8.1). Write tools require a new design review |
| Output screening | Answers are screened for canary tokens placed in system prompts, secret patterns and unexpected URLs or image tags; hits block the unit and raise an alert |
| Rendering safety | The UI renders model text as sanitised markdown; no raw HTML; no automatic fetch of model-supplied URLs or images |
| Injection test suite | Documents and web fixtures with known attack strings run in CI (P16); attack success rate is a release gate |

### 16.7 Tool-specific safety

Release 1 has no web, SQL or MCP tool; this subsection is the safety contract that any of them must meet before it can be registered in a later release.

- **SQL tool.** Prefer **query templates** (a catalogue of parameterised queries) over free-form generation. If natural-language-to-SQL is enabled: read-only role, allow-listed schemas and tables and columns, a `sqlparser`-based validator (single `SELECT`, no functions outside an allow-list), `EXPLAIN` cost cap, statement timeout, row cap, and RLS where the data is tenant-scoped. Sensitive columns are masked.
- **Web tool.** Search results are untrusted. Page fetching goes through a sandboxed fetcher with SSRF protection (block private, loopback, link-local and metadata addresses, re-checked after redirects and DNS resolution), content-type allow-list, size and time limits, HTML stripping, and a snapshot for citations. Respect robots rules and provider terms.
- **MCP tools.** Each MCP server is allow-listed per tenant, its tool descriptions are treated as untrusted (they can contain injection), and results pass through the same normaliser and sanitiser.

### 16.8 Data lifecycle

| Data | Retention | Deletion limit |
| --- | --- | --- |
| Original files and converted copies | Until the document is deleted or the tenant's retention ends | Hidden at once; object purge within **24 hours** |
| Normalised text, chunks, vectors, search rows | Until the document is deleted | Hidden at once; rows and vectors purged within **60 seconds** |
| Messages and citations | Per tenant; default **90 days** (proposed, configurable) | Conversation deletion removes evidence references and verification rows within 60 seconds; citation tombstones ("source removed") remain for old messages |
| Run events (replay log) | 24 hours | Automatic |
| Run traces and `run_steps` | 30 days, redacted | Automatic |
| Audit events | 365 days | Automatic |
| Redis caches, membership cache, rate counters | At most 10 minutes (membership 5 minutes) | Automatic; no source text is stored |
| Temporary parser and converter objects | 24 hours | Automatic |
| Database backups and PITR archives | **35 days** | Age out; a restore re-applies the **deletion ledger** before the service resumes |
| Model host (provider side) | No durable copy of our content; no training on it; retention at most **30 days**, preferably zero (OI-2, OI-3) | Contractual; verified in the provider review |
| User or tenant offboarding | Export on request | Cascade deletion of all the above within 7 days of the confirmed request; backups age out in 35 days |

**Deletion drill (gate G8).** Delete a document, a conversation and a user in staging, then check within the stated limits: the database (chunks, vectors, messages), Redis, object storage, the model host's settings and audit records; then restore a backup into an isolated environment and confirm the deletion ledger removes the deleted data before any query is served.

Data-subject requests are handled by the lifecycle jobs (P23). Provider agreements should specify no training on customer data and retention limits **[confirm with the chosen LLM provider]**.

### 16.9 PII and logging

Structured logs carry IDs, not content. Prompts and answers are logged only in traces with redaction and short retention. Redaction runs on log output and, optionally, on text sent to external providers. Errors shown to users never include internal messages.

### 16.10 Supply chain and memory safety

`cargo-deny` and `cargo-audit` in CI, pinned `Cargo.lock`, minimal features, signed container images. Application crates use `#![forbid(unsafe_code)]`. FFI dependencies (ONNX runtime, PDF libraries) are isolated in thin wrapper crates with fuzz or property tests on parsers.

## 17. Reliability, latency, cost and observability

### 17.1 Latency plan

See §6.5. The grounded path's first visible text depends on retrieval, evaluation and the first verified unit. Latency reduction levers, applied only when measured: parallel retrieval and rerank, a rule pre-router, a small router model, evidence reuse, caching of embeddings and results, and the optional deterministic Evaluator fast path (§25).

### 17.2 Timeouts

Every network call has a timeout derived from the run deadline (`min(tool_timeout, remaining_budget)`). No unbounded waits exist; lint checks reject futures without timeouts at call sites of external services.

### 17.3 Provider resilience

| Concern | Design |
| --- | --- |
| Retries | Exponential backoff with jitter for idempotent calls; honour `Retry-After`; never retry after the first token has been delivered |
| Circuit breaker | Per provider and per tool; open on error-rate or latency thresholds; half-open probes |
| Failover | **None in release 1.** One model host serves routing, reranking, evaluation and generation (D5). A host outage ends runs as `upstream_failure(provider)` with a clear message; the second host or model in the D5 fallback is a later, golden-set-validated change |
| Concurrency | Semaphores per provider and per tenant, sized to rate limits; queueing with a bounded wait; overload returns `429` with `Retry-After` |
| Degradation | Router fails: conservative fallback plan (§4), which never answers exact or live questions from documents. Evaluator fails: **fail closed** (`evaluation_unavailable`, §5). NLI fails: buffered LLM-judge mode or `verification_failed`. Redis fails: run without caches and checkpoints, alert. Embedding provider fails: keyword-only search flagged as degraded |

### 17.4 Cost controls

- Per-request token and tool-call budgets (§6.1) and a per-tenant daily or monthly spend cap with a soft warning and a hard stop.
- Model routing: small models for routing and verification-lite, larger models only where the golden set shows value.
- Caching (embeddings, retrieval results keyed per §16.4), evidence reuse, and early exits.
- Every run records tokens, cost and calls per component; dashboards show cost per outcome, per tenant and per feature, so the value of each agentic feature versus its cost is visible (§24).
- LLM calls per grounded request are budgeted: router (0 or 1), evaluator (1 to 3), generation (1, or 2 in strict mode), plus local NLI. This count is a tracked metric.

### 17.5 Observability

- `tracing` spans per state and per tool call, including spans on spawned tasks (`.instrument(span)`), exported with OpenTelemetry. `run_steps` rows make runs replayable.
- Metrics: latency per stage, attempts distribution, outcome mix, tool failure rate by tool, evaluator fallback rate, withheld-unit rate, unsupported-claim rate (`fast` mode), estimator token error, queue depth, dead-letter count, cost per run.
- Alerts: rising retry rate, rising `fail_gracefully` rate by reason, tool faults, circuit breakers open, p95 latency, cost anomalies, cross-tenant test failure.
- Dashboards: outcome mix, per-tool health, quality metrics, cost.

## 18. Versioning

| Artefact | Versioning rule |
| --- | --- |
| Prompts | Each prompt has an ID and a version; its hash is stored on every run. Changing a prompt requires a golden-set run (P19) |
| LLM models | Pinned model IDs per component; change goes through evaluation and a canary |
| Embedding model and chunking | `embedding_model_id` on every chunk; one index per model generation; reindex via dual-index migration (P18); chunking parameters versioned |
| Reranker and NLI models | Model ID plus a calibration artefact (floor, threshold) stored together; model change invalidates calibration |
| Database schema | Forward-compatible migrations (expand, then contract) |
| API | `/v1` prefix; additive changes only inside a version |
| Golden set | Versioned; dev and held-out splits; changes recorded |
| Configuration | Typed, validated at start-up, versioned with the release; per-tenant overrides validated against the same types |

## 19. Evaluation and testing

### 19.1 Golden set

- 200 or more questions at launch (more per slice as it grows), built from real corpus documents and real user questions, each with expected sources and key facts. Slices: single-hop, multi-hop, multi-source, **compound requirements** (conditions, comparisons, constraints, output format; a request that drops a requirement must not score as complete), unanswerable, ambiguous, follow-ups, tool-failure injected, injection attempts, permission-boundary cases (including stale-membership and revoked-grant cases).
- Dev and held-out splits. Tuning uses the dev split; release gates use the held-out split. With a sample of this size, report confidence intervals (for example bootstrap) and use paired comparisons against the baseline.

### 19.2 Metrics

Retrieval: recall@K, precision, MRR. Answers: faithfulness (claims supported), answer correctness, citation accuracy, **unsupported-claim recall** of the gate, **false-withhold rate**, correct-refusal rate. Behaviour: attempts distribution, outcome mix, evaluator false-sufficient rate. Performance: latency per stage, cost, LLM calls per request.

### 19.3 Baseline and feature gates

Stage 1 produces the traditional-RAG baseline. Each agentic feature (router, evaluator loop, rewrite, source switching, memory reuse, verification gate) is enabled only if it passes the pre-registered criteria in §19.8 on a slice large enough to detect the stated effect. Features that do not pass, or whose slice is too small to power the test, are deferred (§25) or enabled only behind an opt-in flag with online monitoring.

### 19.4 Calibration tests

- **Evaluator calibration:** agreement with human sufficiency labels (a chance-corrected measure) and the **false-sufficient rate** (the costly error). This decides the evaluator model and prompt.
- **Reranker floor:** sweep to find the floor that meets the target precision.
- **NLI threshold:** sweep for the false-withhold and false-pass targets (§13.2).
- **Router accuracy:** source and sub-question quality against labels.

### 19.5 LLM record and replay

Provider calls go through the `LlmClient` trait. A **cassette** layer records request and response keyed by a hash of (component, prompt version, model, parameters, input). CI runs in replay mode (deterministic, no provider cost). Re-recording is a deliberate, reviewed step. Temperature 0 does not guarantee identical outputs, which is why cassettes, not live calls, gate CI. A smaller nightly job runs live against a sample to detect model drift.

### 19.6 Test layers

Unit tests (pure logic: state transitions, budget, anchors, gate checks), property tests (`proptest`: budget never exceeded, no emitted unit with an unresolvable citation, an empty `cites` or a role outside the schema, TenantTx isolation), integration tests (`testcontainers`: Postgres with pgvector, Redis, MinIO; `wiremock` for providers and tools), deterministic async tests (`#[tokio::test(start_paused = true)]` for timeouts and backoff), snapshot tests (`insta` for prompts and events), load tests (concurrent streams, slow clients, cancellation storms), chaos tests (provider timeouts, tool faults, Redis loss), security tests (tenant isolation, injection suite, SSRF suite).

### 19.7 Release gates

A release requires: lint and format clean, `cargo-deny` and audit clean, unit and integration tests passing, tenant-isolation suite passing, golden-set results at or above the baseline and gates in §19.3, injection suite below its threshold, and latency and cost within budget on a load test.

### 19.8 Statistical power, thresholds and holdout discipline

**Pre-registration.** Before a feature is evaluated, record in the repository: the target slice, the single primary metric, the minimum detectable effect (MDE), the sample size, α, power, and the no-regression margin. Decisions are made against those numbers, not after seeing results.

**Sample size for paired comparisons (candidate vs baseline on the same questions).** For a binary outcome use McNemar's test. With discordant pairs `p10` (candidate right, baseline wrong) and `p01` (baseline right, candidate wrong), α = 0.05 two-sided and power 0.8:

`n ≈ ( 1.96·√(p10+p01) + 0.84·√(p10+p01 − (p10−p01)²) )² / (p10−p01)²`

| Target effect on the primary metric | Assumed discordance (p10 + p01) | Required paired questions in the slice |
| --- | --- | --- |
| +10 points (for example p10 = 0.175, p01 = 0.075) | 0.25 | about 195 |
| +5 points (for example p10 = 0.125, p01 = 0.075) | 0.20 | about 625 |

The discordance rates are assumptions; they are re-estimated from the Stage 1 baseline runs and the table is recomputed. A feature whose slice has fewer questions than required is **not** declared a win or a loss: it stays behind an opt-in flag and is monitored online until the slice grows.

**Gate criteria.** (1) The paired difference on the primary metric has a lower 95% bound at least +2 points and a point estimate at or above the MDE. (2) On every other slice the lower bound of the difference is at least −3 points. (3) With several features tested, control the family-wise error rate (Holm correction) over the pre-registered primary comparisons. (4) For safety slices (unanswerable questions, injection attempts, permission boundaries) the target is zero failures; with zero observed failures in n independent cases the 95% upper bound is about 3/n (the rule of three), so 300 cases bound the failure rate near 1%. The golden set therefore carries at least 300 cases per safety slice before launch.

**Sizes and growth.** Stage 1: at least 400 questions (150 answerable including multi-hop, 100 unanswerable or ambiguous, 60 follow-ups, 50 model-host and retrieval failure cases, 50 general, live-data and out-of-scope referral cases, 40 injection and permission cases). Safety slices (unsupported claims, permission boundary, injection, referral) grow to 300 each before gate G3, and the total to at least 1,000. The +5 point promotion threshold in G1 needs about 625 paired cases per primary slice, so a feature whose slice is smaller stays off. Two annotators label a random 20% sample; agreement (chance-corrected) must reach at least 0.7 or the guidelines are revised.

**Holdout discipline.** Three disjoint sets: **dev** (tuning, unlimited use), **validation** (feature gate decisions; every look is logged, and the set is refreshed after 20 logged looks or each quarter), and a **final locked test** set used once per release candidate (its use count is logged; it is replaced after exposure). A **temporal holdout**, the newest 20% of questions by creation date drawn from production-like traffic, guards against drift. Prompt, threshold and calibration tuning never touches the validation or final sets.

## 20. Rust implementation architecture

### 20.1 Workspace and module boundaries

**Start small.** The layout below is the target. Stage 1 begins with four crates, `rag-api`, `rag-core` (domain, orchestrator, tool and LLM traits, evaluator, gate), `rag-store` (repositories, `TenantTx`, migrations) and `rag-worker`, with the boundaries kept as modules. A module becomes its own crate only when dependency ownership, compile time or a feature gate makes that useful. The `Tool`, `LlmClient`, repository and inference boundaries exist from day one.

```text
rag/
  Cargo.toml                  (workspace)
  crates/
    common/        ids, Outcome, errors, config types, Evidence, SourceAnchor
    api/           axum server, auth, SSE, rate limits           (binary: rag-api)
    orchestrator/  State machine, Budget, run state, triage glue
    router/        router prompt, schema, rule pre-router
    evaluator/     prompts, quote checker, answer rules (A) to (F), fail-closed handling
    context/       Context Builder (full/light/gaps), token estimation
    generation/    prompt assembly, Grounding Gate (P0, P1, C1-C5), unit framer and parser
    llm/           LlmClient trait, provider clients, structured output, retry, cassettes
    tools-api/     Tool trait, ToolSpec, ToolError, ToolOutput, registry
    tools-docs/    HybridSearchTool, DocumentSearchTool
    tools-web/     WebSearchTool, sandboxed fetcher
    tools-sql/     DatabaseQueryTool, validator, query templates
    tools-rest/    RestApiTool, CrmTool
    tools-mcp/     McpClient trait, McpTool adapter
    retrieval/     vector, keyword, RRF, rerank, expansion, selectivity planner
    inference/     embeddings, reranker, NLI (ONNX), batching, blocking pool
    ingest/        parsers, chunker, embedder, indexer, jobs     (binary: rag-worker)
    memory/        summaries, focus slots, pending clarifications
    store/         sqlx repos, TenantTx, migrations, Redis, object storage
    telemetry/     tracing, metrics, run trace writer
    eval/          golden-set runner, calibration tools          (binary: rag-eval)
  web/             Next.js UI
  deploy/          Dockerfiles, Helm, Terraform
  xtask/           dev automation
```

**Dependency rules** (enforced by `cargo-deny` bans or a workspace lint script): `tools-*` depend on `tools-api`, `common`, `store` (through `TenantTx`) and `retrieval` only; no crate depends on `api`; only `orchestrator` depends on `tools-api` registry internals and `llm`; `store::admin` (raw pool) is not usable by any crate except `ingest` and migrations; heavy dependencies (ONNX runtime) are behind a feature in `inference`.

### 20.2 Error handling

- Libraries use `thiserror` with `#[non_exhaustive]` public error enums; binaries use `anyhow` at the top. Error types map at boundaries: provider errors to `ToolStatus` (§8.2), internal errors to an `error_id` plus a safe message.
- Panics: the API binary uses `panic = "unwind"` so a panic in a spawned task becomes a `JoinError` handled per run, and a `tower` catch-panic layer protects handlers. The worker may use `abort` with supervisor restart; job idempotency makes that safe.
- No `unwrap` or `expect` in non-test code (clippy `unwrap_used` denied outside tests).

### 20.3 Async rules

| Rule | Reason |
| --- | --- |
| Tools and LLM streaming must be **cancel-safe**: dropping the future at any `await` leaves no corrupt state and releases resources | `tokio::time::timeout` and `select!` cancel by dropping futures |
| Use `select!` with `CancellationToken::cancelled()` at long awaits; prefer `biased;` when cancellation should win | Prompt cancellation |
| Never hold a mutex (even `tokio::sync::Mutex`) across a long `await`; prefer message passing or short critical sections | Deadlocks and latency |
| Collect task results through one `collect` function that handles `Result<Result<_,_>,Elapsed>`, `JoinError` (panic or cancel) and cancellation | Avoid nested-result bugs (§8.2) |
| Dropping a `JoinSet` aborts its tasks, but aborting is cooperative at `await` points and cannot stop `spawn_blocking` work | Use deadlines and cancellation flags inside blocking code |
| Instrument spawned tasks with the parent span | Trace continuity |
| Transactions: dropping a `sqlx` transaction without commit rolls back; do not rely on timeout-cancellation mid-write for critical persistence. Terminal persistence runs on a non-cancellable task | Data integrity (§9.3) |
| Unbounded channels and unbounded `Vec` growth are forbidden for request data | Memory safety under slow clients |

### 20.4 Streaming and LLM client

There may be no official Rust SDK for the chosen generator **[verify]**, so `llm` implements the provider's HTTP API on `reqwest` with an SSE stream parser, request IDs, timeouts, retries, structured-output handling (JSON schema validation with `jsonschema`) and token usage capture. Every provider sits behind `LlmClient`; the cassette layer (§19.5) wraps it. Schemas generated by `schemars` are tested against provider acceptance (providers support subsets of JSON Schema), and fixtures check that every prompt-visible schema validates.

### 20.5 CPU-bound and blocking work

Embeddings, reranking, NLI, tokenisation, large JSON handling and PDF parsing are CPU-bound. Rules:

- Run them on a dedicated pool (`spawn_blocking` with a bounded semaphore, or a `rayon` pool behind an async facade), never on Tokio worker threads.
- Share ONNX sessions through `Arc`; batch requests (micro-batching with a short wait) to raise throughput.
- Budget CPU: inference and request handling can contend. If measured contention appears, move `inference` to a separate process with a small internal API (a deployment change, not a design change, because the `inference` crate has a trait boundary).
- Bound model memory, set thread counts explicitly, and load models at start-up with a readiness gate (`/ready` fails until loaded).
- **Cancellation does not stop blocking work, so it is bounded.** (1) Every blocking job processes one micro-batch of at most about 100 ms of CPU and checks an `AtomicBool` or the cancellation token between batches, so cancelled work wastes at most one batch. (2) The inference pool is sized `cores − Tokio worker threads` with a **bounded queue** (twice the thread count). When the queue is full, new work is rejected with `Overloaded`, surfaced as `429` or `503` with `Retry-After`; the queue is never unbounded. (3) Per-run CPU limits: a maximum number of units and passages sent to NLI per run. (4) The async side stops awaiting after its deadline; the orphaned job finishes its current batch and its result is discarded. The metric `orphaned_blocking_jobs` and the CPU-seconds wasted are tracked and alerted on. (5) If measured orphan time or contention is too high, inference moves to a separate process (Spike S8, §24.4).
- **Parsing is process-isolated and killable.** Untrusted documents are parsed in a child process (`tokio::process::Command` with `kill_on_drop(true)`, `setrlimit` or cgroup CPU, memory and file-size limits, a wall-clock timeout then `kill`). A cancelled or runaway parse is terminated, and a parser crash fails one job instead of the worker. In-process parsing is allowed only for trusted small Markdown and plain text.

### 20.6 Database layer

- `sqlx` with compile-time checked queries requires either a live database during the build or the offline query cache (`cargo sqlx prepare`, committed `.sqlx` data, verified in CI). Dynamic filters use `QueryBuilder` (carefully parameterised).
- The `pgvector` crate provides the vector type for `sqlx` **[verify feature flags]**.
- Pool sizing accounts for all replicas against the Postgres connection limit; long SSE streams must not hold a connection (acquire per operation, never per stream).
- Migrations are forward-compatible; the migration role differs from the application role (§16.3).

### 20.7 MCP and ecosystem maturity

The official Rust MCP SDK may move quickly **[verify]**. Hide it behind our `McpClient` trait; pin the version; write contract tests against at least one real server; keep a fallback of a minimal JSON-RPC client over stdio and HTTP. The Tool trait is not frozen until Spike S4 passes.

### 20.8 Build, CI and developer experience

Large workspaces compile slowly. Mitigations: `cargo-chef` layering in Docker, `sccache`, a fast linker, `cargo nextest`, feature-gated heavy crates so day-to-day builds skip ONNX, and per-crate tests. **Toolchain pin (D8).** `rust-toolchain.toml` pins `1.99.1` (released 15 October 2026, [release post](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/)), edition 2024; `Cargo.lock` is committed; CI builds with `cargo --locked` in a clean container; a floating `stable` toolchain is forbidden; a toolchain bump is a reviewed change that reruns Spike S9. The MSRV equals the pinned toolchain. A CI job named `contracts` compiles and tests every code sample in this document under `-D warnings`. `cargo-deny`, `cargo-audit`, `clippy` (deny warnings), `rustfmt` and doc tests run in CI.

### 20.9 Memory and performance

Use `Arc<str>` or `bytes::Bytes` for large text shared between evidence, prompts and events instead of cloning. Cap evidence size per item and per run. Stream large responses. Benchmark hot paths (`criterion`): fusion, unit gate, context assembly, token estimation.

## 21. Technology stack

All choices are defaults behind traits. **[verify]** marks versions or capabilities to confirm.

| Layer | Choice | Notes |
| --- | --- | --- |
| Language, runtime | Rust 1.99.1 (pinned), edition 2024, Tokio | MSRV equals the pin (§20.8) |
| HTTP and SSE | Axum, Tower, tower-http | SSE via `axum::response::sse`. Auth, tracing and catch-panic on all routes; compression and total-request timeouts apply to non-streaming routes only (§15.3) |
| Rate limiting | `tower-governor` plus Redis counters | Per user and per tenant |
| Serialisation and schemas | `serde`, `serde_json`, `schemars`, `jsonschema` | `deny_unknown_fields` on tool and LLM payloads |
| Orchestration | Typed enum state machine, `JoinSet`, `tokio-util::CancellationToken` | No framework dependency |
| LLM client | In-house `LlmClient` over `reqwest` plus an SSE parser, speaking an OpenAI-compatible chat-completions API to the Nemotron 3 Super host **[verify the chosen host's API]** | Provider behind the trait; cassette layer for tests. NVIDIA's model page does not describe a JSON-mode parameter, so JSON Lines is enforced by prompt plus our parser, and by guided decoding only if the host offers it (Spike S7) |
| Resilience | `backon` or `tower::retry`, circuit breaker | Per provider and per tool |
| Postgres | `sqlx`, `pgvector` crate | HNSW index, FTS, RLS |
| Redis | `redis` or `deadpool-redis` | Caches, rate counters and the membership cache. Disposable. Not used for locks, run state or event replay (those are in Postgres) |
| Object storage | `aws-sdk-s3` (S3-compatible) | Originals, normalised text, web snapshots |
| Job queue | Postgres-backed (`FOR UPDATE SKIP LOCKED`, or `apalis` Postgres storage **[verify]**) | Ingestion and maintenance |
| Local inference | `ort` (ONNX Runtime) or `fastembed` **[verify]** | nomic-embed-text-v1.5 embeddings (768 dimensions) and the NLI model; CPU first. Reranking and answering use the model host, not local inference |
| Tokenisation | Provider usage plus conservative estimator; `tokenizers` for local models | Safety margin (§7) |
| Parsing | `lopdf`/`pdfium-render` (PDF), `zip` + `quick-xml` (DOCX and PPTX); converter sidecar for legacy `.doc` and `.ppt`; no OCR in release 1 | Behind the `Parser` trait; spikes S1 and S13 |
| Text utilities | `unicode-segmentation`, `regex`, `blake3` | Sentence gate, grounding checks, hashing |
| SQL guard | `sqlparser` | Allow-list validation (later release; no SQL tool in release 1) |
| MCP | Official Rust MCP SDK (`rmcp`) **[verify]** | Behind `McpClient` (later release; none in release 1) |
| Auth | `jsonwebtoken` plus JWKS cache; `userinfo` client | The customer's OIDC provider (§16.2) |
| Secrets | `secrecy`, secret manager | Redacted debug, zeroise on drop |
| Config | `figment` (TOML plus env) | Typed, validated at start-up |
| Errors | `thiserror`, `anyhow` | §20.2 |
| Observability | `tracing`, `tracing-opentelemetry`, `opentelemetry-otlp`, `metrics` plus Prometheus exporter | Langfuse or Tempo for traces; Grafana; Sentry |
| Testing | `cargo nextest`, `proptest`, `insta`, `wiremock`, `testcontainers`, `criterion` | §19.6 |
| Frontend | Next.js, TypeScript, Tailwind, fetch-based SSE client | §15.3 |
| Packaging and IaC | Docker (`cargo-chef`), Kubernetes or ECS, Terraform, GitHub Actions | Eval gate in CI |

**Minimum Stage 1 stack:** Axum, Tokio, `sqlx` with pgvector, Redis, S3 or MinIO, one embedding model, reranking and generation through one `LlmClient`, `tracing` with one trace backend, Next.js UI, Docker Compose.

## 22. Pipelines catalogue

Each pipeline is separately owned and separately testable. "Gate" means a CI or release check.

| ID | Pipeline | Trigger | Steps | Key rules and failure handling |
| --- | --- | --- | --- | --- |
| P1 | Document ingestion | `POST /documents` | Validate → store → queue → parse → clean → chunk → enrich → embed → index (invisible) → quality checks → atomic publish | Idempotent stages, atomic visibility, dead-letter after max attempts, parser limits (§12.1) |
| P2 | Online query (agentic loop) | `POST /chat/stream` | Auth → lock → load memory → route → plan → select tools → reserve → execute → collect → gate → normalise → ACL recheck → evaluate → decide → context → generate → verify → stream → persist | State graph §3.4; budgets §6; outcomes §3.4 |
| P3 | Retrieval | Search tool call | Filters → vector and keyword in parallel → RRF → rerank → floor → expand | Selectivity planner, calibrated floor, tenant RLS in the same transaction (§12.4 to 12.7) |
| P4 | Query understanding | Each turn | Rule pre-router → standalone query → LLM router → `PlanValidator` → plan | One repair; then the conservative fallback that never answers exact or live questions from documents (§4) |
| P5 | Tool execution and triage | After tool selection | Validate args → reserve budget → parallel execute with timeouts → `collect` → circuit-breaker check → gate rules | Two-layer errors; nested-result handling in one function (§8) |
| P6 | Evidence normalisation | After the gate | Map to `Evidence` → assign IDs → sanitise → dedupe → ACL recheck | Strip or neutralise instruction-like text; drop unauthorised items |
| P7 | Evaluation verdict | After normalisation | Deterministic prechecks → shortlist → LLM judge → schema check → quote check → rules (A) to (F) | Fails closed on invalid output (`evaluation_unavailable`); false-sufficient monitored (§5) |
| P8 | Retry and rewrite | Non-terminal verdict | Matrix check (§6.4) → choose next-attempt parameters → check tried pairs and budget → reserve | Never repeat; stop early on same evidence |
| P9 | Context building | Before generation | Select mode → filter → dedupe → merge → rank → token budget → assemble with escaped delimiters | Three modes; estimator margin |
| P10 | Generation and streaming | After context | LLM stream → unit framer and parser → gate P0, P1, C1 to C5 → emit → citations → verification | Cancellation; retry before first text, never after; `strict` and `fast` variants (§13) |
| P11 | Citation and verification | During and after generation | Resolve anchors → NLI → flags → persist → metrics | Property tests; golden-set accuracy (§14) |
| P12 | Clarification and resume | `ask_user`; next turn | Persist pending row → emit event → classify reply → merge → resume with carried state | One chain, TTL, user-bound (§11) |
| P13 | Memory | After each turn | Update focus slots → summarise at threshold → screen → store with scope and expiry | Summaries from verified content only; revalidate on reuse (§10) |
| P14 | Authentication and tenant isolation | Every request | Verify JWT → Principal → `TenantTx` → tool context → audit | RLS per transaction; isolation tests are a gate (§16.3) |
| P15 | Tool onboarding | New tool or MCP server | Implement trait → declare spec → contract tests → security review → feature flag → golden cases → canary | No core changes; read-only |
| P16 | Prompt-injection defence | Every untrusted input | Sanitise → delimit → authority separation → output screening; CI injection suite | Attack success rate is a gate (§16.6) |
| P17 | Caching and cost control | Cache lookups; each run | Tenant, ACL and corpus-versioned keys → budgets → spend caps → cost records | No cross-principal hits (§16.4, §17.4) |
| P18 | Embedding and model migration | Model or chunking change | Build parallel index generation → backfill → evaluate vs old → flip by flag → rollback window → drop | Calibrations re-fit (§18) |
| P19 | Offline evaluation | PR; nightly | Run golden set (cassette replay) → metrics → compare to baseline with intervals → gate | Slices, held-out split, calibration tests (§19) |
| P20 | Observability | Continuous | Spans → metrics → `run_steps` → dashboards → alerts | Span propagation across tasks |
| P21 | Feedback and improvement | `POST /feedback`; non-answer outcomes | Link to run → queue → triage cause → fix (golden case, threshold, prompt, document request) | Production failures become regression tests |
| P22 | Degradation and error handling | Dependency failure | Failover → evaluator fail-closed → conservative router fallback → buffered verification → keyword-only search | Always honest, never silent (§17.3) |
| P23 | Retention, deletion and privacy | Schedules; requests | Purge traces and payloads → document and tenant deletion cascade → tombstones → backup reconciliation | §16.8 |
| P24 | CI/CD and release | PR; merge | fmt, clippy, deny, audit → tests → isolation suite → eval gate → build → staging → smoke → canary → promote or rollback | §19.7 |
| P25 | Ingestion dead-letter handling | Job exhausts retries | Move to dead letter → alert → operator replay or discard → status update | §12.1 |
| P26 | Access change propagation | ACL, group, role or document change | Bump `acl_version` and the tenant `authz_epoch` → enqueue re-propagation → update `allowed_principals` → caches invalidated by the new epoch | `ACL_RECHECK` covers the lag (§12.2, §16.4) |
| P27 | Idempotency and run locking | Chat request | Idempotency insert → lease acquire (database fence) → run → fenced writes → release | Database fence rejects stale owners; key reuse with a different payload is `422` (§9.5) |
| P28 | Plan validation | Router output | Schema → source union → authorisation → class and source compatibility → coverage → marker consistency → repair → conservative fallback | Required sub-questions are never dropped silently (§4) |

## 23. Dependency map

| Component | Depends on | If it fails | Fallback |
| --- | --- | --- | --- |
| API | Postgres, Redis, JWKS | No service | Replicas; readiness gate |
| Router | LLM provider | No plan | Rule pre-router and the conservative fallback plan, or `ask_user` (§4) |
| Hybrid search | Postgres, embedding model, reranker | No retrieval | Keyword-only search (flagged), then `sources_unavailable` |
| Evaluator | LLM provider | No verdict | Alternate model re-run, then fail closed with `evaluation_unavailable` (no answer) |
| Generation | LLM provider | No answer | Secondary provider; else `internal_error` |
| Verification Gate | NLI model | No verification | Buffered LLM-judge verification, else `verification_failed` (never skipped) |
| Tool: web | Search provider, fetcher | No web evidence | Switch source; report unavailability |
| Tool: SQL/REST/CRM | Targets, delegated tokens | No exact data | Report `sources_unavailable`; do not answer from documents |
| Memory | Postgres, Redis | No continuity | Stateless answers; flagged |
| Ingestion | Object storage, parsers, embeddings | Documents not searchable | Retry, dead-letter, alert |
| Observability | Trace backend | No traces | Local logs; buffered export; never blocks runs |

## 24. Build order, spikes and estimates

### 24.1 Stages and exit criteria

| Stage | Scope | Exit criteria |
| --- | --- | --- |
| 0 | Phase 0 boundary record (§24.5); spikes S1 to S16 (below), with S9, S10, S14, S3, S1 and S5 first; golden-set v0 and annotation process; skeleton workspace | The §0 decision register is signed (D1) and the §0.3 open items are tracked; each spike has a recorded result, a decision and a fallback. A failed assumption changes the design before anything depends on it |
| 1 | Baseline: ingestion (P1), hybrid retrieval (P3), simple context, generation with citation ID check, streaming API with reconnect replay, OIDC auth and RLS, observability, golden set v1, eval pipeline (P19) | Baseline measured on the held-out set; isolation suite green; first dashboards. **Hardening exit (no agent feature starts before this passes):** ACL revocation, deletion, version-change and cache-invalidation tests; ingestion retries, idempotency, dead letters and parser limits; slow-client, disconnect-cancel, provider-timeout and graceful-shutdown tests; the §28.11 profile at the agreed concurrency on the baseline path; backup, restore and runbooks exercised. A weak retriever is fixed in the retriever, not by adding the agent loop |
| 2 | Agentic core: router, plan, Tool Health Gate, PlanValidator, Evaluator with rules (A) to (F), retry loop, Context Builder modes, clarification | Features are added **one at a time, behind per-tenant flags, in this order**: (1) router and PlanValidator, (2) tool health and failure outcomes, (3) Evaluator, (4) bounded retry, (5) clarification. Each must improve its pre-registered slice without unacceptable regression, cost or latency (§19.3, §19.8) or stays disabled |
| 3 | Trust and scale: full Verification Gate (C1 to C5), `strict` mode, evidence-reuse revalidation, cost controls | Unsupported-claim recall and false-withhold targets met |
| 4 (after release 1) | Tool expansion: web, SQL, REST; query templates; SSRF and injection suites | Security suites green; each tool passes P15 |
| 5 (after release 1) | Enterprise: CRM, MCP, delegated credentials, multi-tenant hardening, deep-research mode if justified | Penetration test; load test; DR test |

### 24.2 Stage 0 spikes

| ID | Question | Pass condition |
| --- | --- | --- |
| S1 | Parser quality on the real corpus (PDF text and tables, DOCX, PPTX slides and notes) | Acceptable chunk quality; decision: Rust-only or sidecar |
| S2 | pgvector recall and latency under real filters, with exact search as ground truth | Recall target met, or switch plan |
| S3 | NLI model accuracy and latency on corpus-like claims, numbers and tables | False-withhold and false-pass targets reachable; per-unit latency within budget |
| S4 | MCP crate compatibility with at least one real server | Contract tests pass; `McpClient` trait stable |
| S5 | SSE through the real proxy and load balancer, HTTP/1.1 and HTTP/2, disconnect, reconnect and cancel behaviour | No buffering; reconnects resume; no reconnect cancels the run within 12 s |
| S6 | RLS with pooled connections: no residue, no bypass, performance with the HNSW query | Isolation tests green; acceptable overhead |
| S7 | LLM client against the chosen Nemotron 3 Super host: streaming, cancellation, JSON Lines output, token accounting, retry before first text | Cancel stops the stream; usage captured; JSON Lines output meets S12 |
| S8 | CPU contention between inference and request handling | p95 latency holds under load, or inference moves to its own process |
| S9 | Rust contracts compile and behave: every code sample in §6.3, §8.1, §9.5 and §13 on the pinned toolchain and crate versions | All samples compile; the §6.3 tests pass under paused time; no clippy errors |
| S10 | Live-ACL RLS policy cost and plan at the §28.11 corpus (200,000 chunks, 3 tenants, ACL mix), plus a 2,000,000-chunk stress run | Retrieval p95 within the §6.5 budget with the live policy, or the version-match variant meets it |
| S11 | Fencing under failure: Postgres failover drill, forced lease expiry, Redis flush | Fence never moves backwards, no stale-owner write commits, no run loses its lease without noticing |
| S12 | Gate parser fuzzing and provider JSON-line reliability | Zero emitted units from malformed, uncited, `meta` or injected-marker inputs over at least 1,000,000 fuzz cases |
| S13 | Legacy `.doc` and `.ppt` conversion through the isolated converter, and PPTX parsing quality | At least 95% of a 100-file sample converts with usable text; no network access from the converter; resource limits hold |
| S14 | Model acceptance (D5): nomic-embed-text-v1.5 and Nemotron 3 Super on the golden set: quality, speed, cost, English handling, data handling | Embedding Recall@10 at least 0.90; answer quality, speed and cost within D6 and D7; data terms approved |
| S15 | LLM listwise reranking against no rerank and against a local cross-encoder | nDCG@10 gain of at least 0.03 and rerank latency within the §6.5 retrieval budget |
| S16 | Customer OIDC provider: does it re-evaluate group and role membership at every token issuance, and how fast does a removal show up | Removal visible within 300 s on the chosen route (token claims or `userinfo`) |

### 24.3 Estimates (provisional)

These are planning assumptions, not commitments: a team of 3 to 4 experienced Rust engineers plus one ML and evaluation engineer, one annotator part-time. Stage 0: 2 to 3 weeks. Stage 1: 5 to 7 weeks. Stage 2: 5 to 7 weeks. Stage 3: 4 to 6 weeks. Stage 4: 4 to 6 weeks. Stage 5: 6 to 10 weeks. Re-estimate after Stage 0 and after Stage 1, when real parsing, filter and latency numbers exist. Rust adds ramp-up and compile-time costs relative to a Python prototype; the plan accepts that in exchange for performance, safety and a single deployable.

### 24.4 Decision defaults (so nothing blocks implementation)

Each Stage 0 spike has a default that Stage 1 builds on, a fallback, and a measurable switch criterion. Stage 0 confirms or switches the default; it does not leave the design open.

| Spike | Default decision (built in Stage 1) | Fallback decision | Switch criterion (measured at Stage 0) |
| --- | --- | --- | --- |
| S1 Parsing | Rust-native parsers for text PDF, DOCX and PPTX, in killable child processes. A parsing sidecar container (HTTP `Parser` API, isolated, resource-limited) is part of the deployment from Stage 1 for scans, OCR and complex tables | Route all PDFs through the sidecar | Chunk-quality score on a 100-document corpus sample below the agreed threshold for native parsing |
| S2 Vector search | pgvector HNSW plus the filter selectivity planner (exact scan when the filtered set is small, HNSW with raised `ef_search` and iterative scan **[verify]** otherwise); initial exact-scan cutoff 50,000 filtered chunks, tuned by measurement | Dedicated vector engine behind `VectorIndex` | Recall@10 under production-like filters below 0.95 of exact search at the p95 latency target |
| S3 Grounding screen | `gated` mode with deterministic checks C1 to C3 and C5 plus the local NLI check C4 | `strict` mode with an LLM judge (different model) as the default tier | `standard`-tier bound (§13.5) or the false-withhold target (§13.2) not reachable with the local NLI on the corpus |
| S4 MCP | MCP is deferred to Stage 5 behind our `McpClient` trait; Stages 1 to 4 do not depend on it | Minimal in-house JSON-RPC client | Contract tests against a real server fail with the pinned crate |
| S5 SSE | Fetch-based SSE client, keep-alives, §15.3 contract | Long-poll replay endpoint over the same event stream | Any of the deployed-stack tests (a) to (f) fails after proxy tuning |
| S6 RLS | §16.3 policies and `TenantTx`. **Correctness is never traded for speed** | Add per-tenant partitions or indexes; dedicated database for very large tenants | Retrieval p95 overhead above the budget after tuning; RLS stays on |
| S7 LLM client | In-house `reqwest` client with the cassette layer | A maintained community crate behind `LlmClient` | Streaming, cancellation, structured output or usage capture fails the spike tests; JSONL unit streaming unreliable means buffered `strict` mode with non-streaming structured output (§13.0); no second streaming format in release 1 |
| S8 CPU contention | In-process inference on the bounded pool (§20.5) | A separate `rag-inference` process | API p95 latency inflates more than 20% under the load test, or orphaned blocking time exceeds its budget |
| S9 Rust contracts | Samples as written in this document | Adjust the sample and the section together; the document is corrected, not the test | Any sample fails to compile or fails its tests on the pinned versions |
| S10 Live-ACL policy | Live `EXISTS` policy on `chunks` (§16.3) | Version-match variant (§16.3) | Retrieval p95 over budget after index and plan tuning |
| S11 Fencing | Database lease and fence (§9.5) on a synchronous-standby Postgres | Add a monotonic `lease_epoch` sequence table if row contention is measured to be a problem | Fence regression after failover, or a stale-owner write commits |
| S12 Gate parser | JSON-line units (§13.0) | Buffered `strict` mode with non-streaming structured output | Any fuzz case emits a unit that violates the schema, or provider JSON lines are unreliable |
| S13 Legacy formats | Convert `.doc` and `.ppt` in the isolated converter sidecar | Reject legacy formats and ask users to re-save as `.docx` or `.pptx` | Fewer than 95% of the sample convert usably, or the sidecar cannot be isolated |
| S14 Models | nomic-embed-text-v1.5 plus Nemotron 3 Super (D5) | Another open embedding model; a second host or approved model for answering | Any D5 pass limit missed, or the data terms are not approved |
| S15 LLM rerank | Listwise rerank of the top 20 by the answer model | Local cross-encoder behind the `Reranker` trait; or no rerank | nDCG@10 gain under 0.03, or latency over budget |
| S16 OIDC membership | Token claims if the provider re-evaluates membership at issuance, else `userinfo` per request | `userinfo` per request, cached 300 s | Removal not visible within 300 s on the chosen route |

### 24.5 Phase 0 product and security boundary record

This record is §0 (decisions D1 to D10, the decision log and the open items). It is signed before Stage 1 starts.

- Initial users, tenant model, corpus size, file types and languages.
- What the first release must answer, and what it must refuse or route elsewhere.
- Threat model, including whether a compromised application role is in scope (§16.3), and the membership source and maximum revocation delay (§16.2).
- Data residency, provider use, retention, deletion and audit requirements.
- Expected peak concurrency and request mix, which replace the §28.11 starting values if different.
- A named owner for the evaluation dataset and the release criteria.

## 25. Deferred features (build only if measured need appears)

| Feature | Revisit when |
| --- | --- |
| Deep-research mode (attempts 4 and 5) | Users need it and cost controls allow it |
| HyDE-style rewriting; automatic multi-step decomposition beyond the sub-question DAG | The golden set shows plain rewrite does not recover failures |
| Deterministic Evaluator fast path | Evaluator latency or cost is a proven bottleneck and it matches human labels |
| BM25 engine (extension or Tantivy) | Keyword recall limits quality (§12.5) |
| Dedicated vector database | Spike S2 or scale shows pgvector limits |
| LLM-as-reranker | Cross-encoder quality is insufficient |
| Semantic response cache | Repeated-question hit rate is high and permission-safe keys exist |
| Graph or knowledge-graph retrieval | Multi-document relational questions dominate and fail otherwise |
| Multimodal ingestion | A real need for images or audio |
| Write-capable tools | A separate security design is approved |
| Microservices split | Measured scaling or ownership need |

## 26. Open decisions

Superseded by §0.3, which lists every open item with its risk, the person needed and the default that applies until it is closed. The earlier list of ten open decisions was answered in the owner's checklist reply of 26 October 2026 and is recorded in §0.1 and §0.2.

## 27. Traceability: findings to resolution

### 27.1 Findings from the v4 validation review

| ID | Finding | Resolved in |
| --- | --- | --- |
| F1 | Document incomplete (ended at §8) | Whole document; Document control |
| F2 | Answer decision too permissive | §5 (rule A to D, quote check), §6.3 |
| F3 | Citation validation underspecified | §13.2, §14 |
| F4 | Streaming and validation in tension | §13 (gated, strict, fast), §15.2, §15.3 |
| F5 | Security asserted, not specified | §16 (all), §12.2, §20.6 |
| F6 | Tool abstraction hides differences; failures all become `ToolStatus` | §8.1 to §8.4, §20.2 |
| F7 | Retrieval and ingestion not concrete | §12 (all), §14.1 |
| F8 | Multi-hop coverage and retry budgeting | §4 (plan DAG), §6 (all) |
| F9 | Memory and clarification lifecycle incomplete | §10, §11 |
| R1 | Rust async tool execution and cancellation | §8.2, §8.5, §9.4, §20.3 |
| R2 | Rust tool errors | §8.2, §20.2 |
| R3 | Database, RLS and pooling | §16.3, §20.6 |
| R4 | Ingestion, parsing and sidecar complexity | §12.1, §12.8, S1 |
| R5 | Dependency and API choices (`axum`, `backon`, `rmcp`) | §20.7, §21, S4 |
| R6 | SSE backpressure, disconnect, cancellation | §15.3, §9.4, S5 |

### 27.2 Additional findings from this validation (Appendix A)

| ID | Finding | Resolved in |
| --- | --- | --- |
| A1 | `ROUTE` could not reach `ASK_USER` in the v4 graph although the rules require it | §3.4 |
| A2 | `Empty` always triggered a rewrite, wrong for authoritative lookups where "not found" is the answer | §8.3, §6.4 |
| A3 | `Denied` handling could leak resource existence | §16.2, §3.4 |
| A4 | Evidence reuse test was undefined and ignored ACL, version and age | §10.4 |
| A5 | Evaluator and generator used the same model, risking correlated errors; calibration was unspecified | §5, §19.4 |
| A6 | No concurrency control (two runs per conversation, duplicate requests) | §9.3 |
| A7 | Terminal persistence could be cancelled by client disconnect | §9.3, §20.3 |
| A8 | `FAIL` path had no verification and no partial-findings behaviour | §3.4, §6.3, §13.1 |
| A9 | No ACL staleness handling between ACL change and index update | §12.2, P26 |
| A10 | Web fetch SSRF and NL-to-SQL risks not addressed | §16.7 |
| A11 | MCP tool descriptions are an injection vector | §16.7 |
| A12 | Golden set size and statistics unspecified; no held-out split | §19.1 |
| A13 | LLM cost per request unbounded across router, evaluator, generator, verifier | §17.4 |
| A14 | Cache keys could cross principals | §16.4 |
| A15 | Token counting for the generator not available offline | §7, §21 |
| A16 | Status "Ready for implementation" was premature | Document control, §24 |
| A17 | CPU-bound inference could stall the async runtime | §20.5, S8 |
| A18 | Ingestion publish was not atomic; partial versions could be searched | §12.1, §12.2 |

### 27.3 Findings from the v5 review and v6 additions

| ID | Finding | Resolved in |
| --- | --- | --- |
| N1 | Evaluator invalid-output fallback answered from rerank scores (not safe) | §5 (fail closed), §6.4, §3.4, §22 P7, §23 |
| N2 | RLS compared only the tenant; user, group and role ACLs were not enforced by policy | §16.3 (policies and tests), §12.2 |
| N3 | Cache keys omitted direct user grants, roles, delegated scopes and a policy version | §16.4 (`authz_ctx`, `authz_epoch`, ACL recheck) |
| N4 | Router example contradicted its plan (CRM question without a CRM source) | §4 (corrected example, `PlanValidator`) |
| N5 | Router fallback could answer exact or live questions from documents | §4 (conservative fallback), §5 rule (E) |
| N6 | "Just guess" could bypass the fail-honestly principle | §11.2, §4 |
| N7 | The verification gate was described as proof; withheld sentences could break coherence | §13.0 (units, answer-first), §13.5 (assurance language, tiers) |
| N8 | Evaluation gates were underpowered and open to overfitting the holdout | §19.8 |
| N9 | Cancelled blocking work keeps consuming CPU | §20.5 (bounds, admission control, killable parsing) |
| N10 | Retries were not reserved against the budget | §6.3 (reservation and code) |
| N11 | Provider failure after streaming was labelled `internal_error` | §3.4, §13.4 (`upstream_failure`) |
| N12 | Idempotency and lock semantics unspecified | §9.5 |
| N13 | SSE middleware (compression, timeouts) could break streaming | §15.3, §21 |
| N14 | RLS tests depended on unstated policy expressions | §16.3 (exact policies and named tests) |
| N15 | Unverified assumptions left the design "gated" rather than decided | §24.4 (default, fallback, switch criterion) |
| N16 | Nothing defined what "ready to deploy" means | §28 (go-live gates) |

### 27.4 Findings from the v6 review (R) and my own validation of v6 (O)

| ID | Finding | Resolved in |
| --- | --- | --- |
| R1 | Chunk RLS used a lagging denormalised array; live ACL only in a comment, so a revoked user could receive text before `ACL_RECHECK` | §12.2, §16.3 (live policies, tests 5a to 5e), S10 |
| R2 | Redis `INCR` fence is not durable or monotonic across failover; the database fence advanced only on writes | §9.5 (database lease and fence), S11 |
| R3 | Retry sample reserved the first call twice; `min_useful_time` and `MAX_RETRIES` undefined | §6.3, §8.1, S9 |
| R4 | Generation emitted JSON-line units but the gate split sentences and read inline markers | §13.0, §13.2, §14, §15.2, §22 P10 |
| R5 | Model-supplied `meta` role could skip citation checks | §13.0 (no model `meta`, system templates only), §13.2 |
| R6 | Backoff sleep ignored cancellation and could reserve after cancel | §6.3 (ordered steps, reserve last) |
| R7 | "Load test at target concurrency" had no number | §28.11, G5, §26 item 5 |
| O1 | Policy subqueries run under the caller's RLS, so the v6 policy would have needed `SECURITY DEFINER` or returned nothing from `acl_entries` | §16.3 (`acl_self`, `documents_read`; no `SECURITY DEFINER`) |
| O2 | Lease and fence safety depends on synchronous replication; clocks must be database time | §9.5, §28.2 |
| O3 | Code samples were never compiled | S9, honesty note |
| O4 | Model-written `[n]` text could be mistaken for a citation | §13.2 (P1), §14.1 |
| O5 | The `gaps` answer needed uncited statements once `meta` was removed | §13.0 (system templates) |
| O6 | Starting replica sizes were unvalidated against any concurrency number | §28.3, §28.11 |
| O7 | Status text claimed more than a design can | Document control |

### 27.5 Risks from the v7 strategic review (V)

| ID | Risk | Resolved in |
| --- | --- | --- |
| V1 | Group and role claims could be stale; provider outage behaviour undefined | §16.2 (membership lookup, maximum age, stale and degraded paths) |
| V2 | RLS settings are not an independent defence against arbitrary SQL as the app role | §16.3 (threat boundary, separate SQL-tool role, injection tests, S6 fallback) |
| V3 | Plan coverage checked only entities and dates | §4 (requirement coverage), §5 rule (F), §19.1 |
| V4 | Parser did not enforce first-unit and ordering invariants; two formats | §13.0, §13.2, §24.4 (JSON Lines only in release 1) |
| V5 | Lease expiry used transaction-start time | §9.5 (`clock_timestamp()`, bounded transactions, takeover semantics, tests) |
| V6 | Reservation ownership and drop behaviour implicit | §6.3 |
| V7 | Format consistency not tested end to end | §14.4 |
| V8 | Build order let agent features start before the baseline was hardened | §24.1, §24.5, §20.1 |

### 27.6 Findings from the v10 reviews (W) and the owner's answers (Q)

| ID | Finding | Resolved in |
| --- | --- | --- |
| W1 | v10 was not self-contained: the load test and gate list pointed to v8, and "G0 to G10" was undefined | §28.10, §28.11 (full numbers and named gates) |
| W2 | Spike IDs and decision IDs collided (S1 to S10 meant two things) | Decisions are D1 to D10; spikes are S1 to S16 |
| W3 | The +2 point promotion threshold could not be met with a 300-case set | §28.10 G1 (point estimate at least the pre-registered MDE, default +5 points, lower bound above zero; feature stays off if the data cannot show it) |
| W4 | Group ACL visibility was ambiguous next to "own grants only" | §16.3 (`acl_self` over the caller's effective principal set), tests 9 to 12 |
| W5 | Routing conflict: general answers allowed in one section, refused in another | D3, §3.4, §4, §7, §13 (one rule) |
| W6 | Malformed-output retry could repeat or conflict with text already shown | D6, §13.4 (retry only before any text; terminal `incomplete` after) |
| W7 | Models and versions were not named | D5, §12.4, §12.6, §21 |
| W8 | Rust pin used a mutable "latest" link; stale "v8" text; diagram source missing | D8 and §20.8 (immutable release post), this document's own title, §3.6 and `architecture.mmd` |
| W9 | Cost cap arithmetic unverified | §6.1 (token budget check), OI-9 |
| W10 | Starting with nine crates contradicted "start small" | §20.1 (four crates first) |
| W11 | Redis described as both cache-only and holding locks | §21 (Redis: caches, counters, membership cache only) |
| Q1 | Owner is a non-technical customer | D1 (plain-language approvals, change rule) |
| Q2 | Users, formats, refusals | D2, §12.1, S13 |
| Q3 | No general answers, no live data, refer to the owner | D3, §3.4, §4 |
| Q4 | OIDC for grants | D4, §16.2, §16.3a, S16 |
| Q5 | nomic embeddings and Nemotron 3 Super | D5, OI-2, S14, S15 |
| Q6 | Retry 3 times, none after text is shown, reconnect 3 times | D6, §13.4, §15.3 |
| Q7 | 10 concurrent, 50 random users; 1% production, 2% staging | D7, §28.10, §28.11 |
| Q8 | Rust version pinned; samples compiled | D8, §20.8, S9 |
| Q9 | Retention, region, provider terms, deletion tests | D9, §16.8, OI-3 |
| Q10 | Record approvals, keep open items, no completion with a high-risk item open | §0.2, §0.3 |

## 28. Deployment architecture and go-live readiness

### 28.1 Environments

| Environment | Purpose | Data |
| --- | --- | --- |
| Local | Developer work with Docker Compose (Postgres with pgvector, Redis, MinIO, provider stubs) | Synthetic |
| CI | Ephemeral containers (`testcontainers`), cassette-replayed LLM calls | Synthetic and fixtures |
| Staging | Production-shaped topology, real providers with spend caps, deployed-stack tests (G4), load and chaos tests | Synthetic and anonymised samples; no production data |
| Production | Live service | Customer data under the policies in §16 |

### 28.2 Production topology (reference; Kubernetes, or the equivalent on ECS)

| Component | Configuration |
| --- | --- |
| `rag-api` | At least 2 replicas in 2 availability zones; HPA on concurrent streams and CPU; PodDisruptionBudget (at most 1 unavailable); `terminationGracePeriodSeconds` at least run deadline + 10 s; `preStop` drain; readiness waits for the embedding and NLI models to load (§20.5) |
| `rag-worker` | 1 to 2 replicas; scaling on queue depth and oldest-job age; parser child processes with resource limits |
| Converter and parsing sidecar | Separate Deployment, no internet egress, no access to application databases, read-only filesystem, CPU, memory and time limits, private network only |
| Model host (Nemotron 3 Super) | **Not yet chosen (OI-2).** NVIDIA's model page lists a minimum of 8 H100-80GB GPUs for self-hosting; a managed host is the alternative. The hosted NVIDIA API catalogue is a trial service and is not used for customer data. Whatever is chosen must be in the approved region, under the approved terms, with a spend and concurrency limit |
| PostgreSQL | Managed, highly available (primary plus **synchronous** standby in another zone; synchronous replication is required for fence durability, §9.5), point-in-time recovery, pgvector installed, separate migration, application, ingestion and authorisation roles (§16.3a) |
| Redis | Managed, highly available. Disposable: caches, rate counters and the membership cache only. Not a source of truth, not used for locks or event replay |
| Object storage | Versioning on, lifecycle rules for temporary objects, server-side encryption, tenant-scoped keys |
| Ingress and load balancer | Streaming settings per §15.3 (buffering off, idle timeout above 120 s), TLS 1.2 or higher, WAF, per-IP rate limits |
| Egress | Forward proxy allowing only the model host and the OIDC provider; private networking to Postgres and Redis |
| Secrets | Cloud secret manager, short-lived credentials, rotation schedule |
| Telemetry | OpenTelemetry collector, metrics, log pipeline with PII redaction; only identifiers and metrics by default |

### 28.3 Sizing method and starting assumptions

Sizing is **measured**, not assumed. The figures below are provisional until the §28.11 reference profile has been run; they are not claimed to sustain it. **Corpus assumption (OI-4):** 1 to 3 tenants, at most 2,000 documents, at most 200,000 chunks. Vector memory check: 200,000 chunks × 768 dimensions × 4 bytes ≈ 0.61 GB of raw vectors, about 0.9 GB with an HNSW graph, so the working set fits easily in memory. Starting assumptions: 2 API replicas of 2 vCPU and 4 GiB, 1 worker of 4 vCPU and 8 GiB, Postgres with 4 vCPU and 16 GiB. Model-host capacity cannot be sized on paper: the load test records tokens per second and time to first token at 10 concurrent streams, and the host is sized from those numbers (OI-2). Connection pools are sized as `replicas × pool size ≤ 70%` of the Postgres connection limit; streams never hold a database connection (§20.6).

### 28.4 Configuration and feature flags

Typed configuration validated at start-up. Per-tenant flags: verification tier and mode, enabled tools, `max_attempts` (up to 5), spend caps, retention windows, and which agentic features are on (router, evaluator loop, reuse). Model, prompt and calibration versions are configuration, so a rollback is a flag change plus a redeploy of config, not a code rebuild.

### 28.5 Release and rollback

- Database migrations are expand-then-contract and backward compatible with the previous release.
- Release path: staging soak and gates (§19.7, §28.10), then canary at 5%, 25% and 100% of traffic, with each step held for a fixed observation window.
- **Automatic rollback triggers:** error rate (`internal_error` or `upstream_failure`) above threshold, p95 time to first token above its budget, a shift in the outcome mix (for example `fail_gracefully` up by more than a set margin), cost per run above its budget, a failing synthetic isolation probe, or a rise in the withheld-unit rate.
- Model, prompt or threshold changes follow the same canary path and the golden-set gate.

### 28.6 SLOs and alerts

| Signal | Objective or alert |
| --- | --- |
| Stream start success | 99.9% monthly |
| Time to first token (grounded, 1 attempt) | p95 under 12 s (§28.11) |
| `internal_error` rate | Alert above 0.5% of runs |
| `upstream_failure` rate by provider | Alert above 2% over 10 minutes; page on-call (no failover in release 1) |
| `evaluation_unavailable` rate | Alert above 1% |
| Ingestion | Dead-letter count above 0 for 1 hour; oldest queued job older than 15 minutes |
| Isolation | **Synthetic probes** run continuously in production with a canary tenant pair and users with different ACLs; any cross-boundary read pages on-call immediately |
| Withheld-unit rate and `low_confidence` rate | Alert on a sustained rise over baseline |
| Cost | Cost per run and per tenant against budget; spend cap warnings |
| Orphaned blocking jobs | Alert on a sustained rise |

### 28.7 Backup and disaster recovery

PostgreSQL: continuous archiving with point-in-time recovery, **RPO at most 5 minutes, RTO at most 60 minutes**. Object storage: versioning, with optional cross-region replication. Redis: no durable truth; a loss costs caches, checkpoints and event replay only. After any restore, deletion tombstones are re-applied (§16.8). Restore drills run quarterly and a drill must meet the RPO and RTO to count.

### 28.8 Runbooks (each exercised before go-live)

Model host outage (no failover in release 1: the message and the recovery steps); tool or source outage; ingestion backlog and dead-letter replay; evaluator drift or rising `evaluation_unavailable`; verification tier regression; cost spike and spend-cap response; ACL or isolation alert; tenant data deletion and export request; Redis loss; Postgres failover; certificate and secret rotation; rollback.

### 28.9 Security go-live review

Threat-model review against §16.1; penetration test with no open high or critical findings; isolation and injection suites green; SSRF suite green; secrets and dependency audit (`cargo-deny`, `cargo-audit`, SBOM, signed images); data-processing agreements with providers (no training on customer data, retention limits, **[confirm with each provider]**); privacy impact review.

### 28.10 Go-live gates

Every gate is measurable, has an owner role and is complete in this section. Environment limits come from D7: **unauthorised access is 0 in every environment**; unsupported-claim pass-through and failed-run rate are at most **2% in staging and 1% in production**; in development they are tracked but do not block. A gate must pass in staging at the staging limit before promotion, and in production at the production limit before full rollout.

| Gate | Name | Requirement | Pass limit | Owner |
| --- | --- | --- | --- | --- |
| G0 | Decisions and spikes | Decision log (§0.2) complete; no high-risk open item (§0.3); spikes S1 to S16 recorded, each default confirmed or switched | All 10 decisions logged; OI-1, OI-2, OI-3 closed; every spike has a decision | Business Owner, Technical Lead |
| G1 | Baseline and feature evaluation | Conventional RAG baseline measured on the locked held-out set; every enabled agentic feature compared with it on pre-registered slices | A feature is enabled only if the lower 95% bound of its paired improvement is above 0 and the point estimate is at least the pre-registered minimum detectable effect (default **+5 points**, which needs about 625 paired cases; Holm correction across primary comparisons); no non-primary slice worsens by more than 3 points; otherwise it stays off | AI Evaluation Owner |
| G2 | Isolation and access | §16.3 tests 1 to 15 and the §16.3a inventory check pass as production-like roles; membership freshness (S16); production synthetic isolation probes live | 0 unauthorised rows, objects or cache hits; 100% of inventory items have a passing test; revoked membership blocked within 300 s | Security Owner |
| G3 | Grounding bound | The grounding screen measured on at least 300 unsupported-claim test cases (about 150 suffice for the 2% bound), judged by the locked human labels | Unsupported-claim pass-through upper 95% bound at most 2% in staging and 1% in production (with zero observed failures, 300 cases give a 1% bound by the rule of three; any observed failure needs more cases); false-withhold rate at most 10% | AI Evaluation Owner |
| G4 | Streaming through the real ingress | Stream tests (a) to (g) of §15.3 on the deployed stack; parser fuzzing (S12) | All pass; fuzzing emits 0 invalid units in at least 1,000,000 cases; no retry after text is shown | Platform and SRE Owner |
| G5 | Load profile R1 | The workload in §28.11 | Every pass criterion in §28.11 met in two comparable runs | Platform and SRE Owner |
| G6 | Chaos | Model-host outage, Postgres failover, Redis loss, evaluator failure (fails closed), slow-client storm, blocking-pool saturation | No data loss, no two writers, no unbounded queue, load shed with `429` instead of collapse, fixed templates shown | Platform and SRE Owner |
| G7 | Security review | Threat model review; independent penetration test; prompt-injection, SQL-injection and `set_config`/`SET ROLE` attempt suites; dependency audit (`cargo-deny`, `cargo-audit`, signed images) | No open high or critical finding; injection attack success rate 0 on the suite | Security Owner |
| G8 | Restore and deletion | Backup restore drill; deletion drill of §16.8 including a restored backup | Restore meets RPO at most 5 minutes and RTO at most 60 minutes; deletion limits of §16.8 met; deletion ledger re-applied before serving | Platform and SRE Owner, Data and Records Owner |
| G9 | Cost | Cost per run measured at the chosen host | p95 cost per run within the cap set after OI-2 (OI-9); the 60,000-token hard stop holds | Business Owner |
| G10 | Data handling and sign-off | Region, provider terms (no training, retention at most 30 days), retention schedule, named owners, rollback and runbooks exercised | Signed by the Privacy and Legal Owner and the Business Owner; every runbook exercised once | Privacy and Legal Owner, Business Owner |

### 28.11 Reference load profile (R1) and pass criteria

The profile is a versioned artefact: script hash, tool version, random seed, corpus snapshot and hash, query IDs, model-latency recording version and ingress configuration are stored with each result. The generator is k6 with arrival-rate executors, which start requests on a schedule independent of response time, or an equivalent Rust generator **[choose in Stage 1]**; it opens real SSE streams through the production ingress path. The numbers below are the business-confirmed starting values (D7); if the real peak differs, scale them and rerun.

| Dimension | Specification |
| --- | --- |
| Users | A pool of **50 distinct users**. Each request picks a user uniformly at random and a query from the seeded query set |
| Concurrency | **10 concurrent active streams** in steady state, 15 in the burst, 5 in the soak. Expected from Little's law: arrival rate × mean stream time ≈ concurrency; with a measured mean stream time of about 12 s the starting arrival rates are 0.85 requests per second (steady), 1.3 (burst) and 0.4 (soak). The rates are adjusted after the first baseline capture so that the **achieved** average active streams is within ±10% of the target (9 to 11, burst 13.5 to 16.5); a run that misses its target is **invalid**, not a pass |
| Phases | Ramp 10 minutes to the steady rate, hold 30 minutes, burst 5 minutes, return to steady for 10 minutes; then a 2-hour soak at the soak rate before the first production release |
| Corpus | 200,000 chunks (about 2,000 documents of roughly 100 chunks): 60% PDF, 25% Word, 15% PowerPoint (a share of them legacy formats); English only; 768-dimension vectors, HNSW built, ingestion idle |
| Tenants and access | 3 tenants split 60/30/10 by chunks; the 50 users spread in proportion; access mix of documents 60% tenant-wide, 30% group-restricted, 10% user-restricted |
| Request mix | 60% single-attempt document questions; 20% with 2 to 3 attempts; 8% multi-part; 7% general, live or out-of-scope (expected outcome `refer_to_owner`); 3% greetings; 2% clarification and resume |
| Model host and stream shape | Recorded latency and token-cadence stubs for 95% of calls, the real model host (under a spend cap) for 5% to confirm the stubs. Stubbed streams replay the measured stream-length distribution (p50, p95, maximum), never instant completions |
| Injected faults | 1% model-host timeouts, 0.5% overload responses, one Postgres failover and one Redis restart during the hold, one 60-second model-host outage during the soak |
| Client behaviour | 5% of clients drop mid-stream and reconnect within the 10 s grace (must resume); 2% drop and never return (must be cancelled); 2% are slow readers; the rest read at full speed. At most 3 reconnects per run; reconnects are reported separately from server-side retries and use the same run ID |
| Repetition | Each phase that decides a gate runs at least twice; results carry the achieved rate, dropped arrivals and the distribution of concurrent streams |

| Pass criterion | Threshold |
| --- | --- |
| Stream start success (excluding injected faults) | At least 99.9% |
| Time to first token, grounded, 1 attempt | p50 under 6 s and p95 under 12 s (starting targets; they can be tightened from the baseline, never loosened without the Business Owner's approval) |
| Full grounded answer, up to 3 attempts | p95 under 30 s |
| Retrieval plus rerank, one attempt, live ACL policy on | p95 under 2 s |
| Failed-run rate (`internal_error`, `upstream_failure` or `verification_failed`, excluding runs with an injected fault) | At most 2% in staging, 1% in production |
| Referral correctness | 100% of the general, live and out-of-scope requests end in `refer_to_owner`; 0 get a factual answer |
| Disconnects | 100% of dropped-and-reconnected runs resume with no lost or duplicated unit; 100% of dropped-and-gone runs are cancelled within 12 s |
| Queues | No growth trend in the inference or blocking-pool queue over the last 15 minutes of the hold; load shed (`429` with `Retry-After`) under 1% at 10 streams |
| Burst (15 streams) | No collapse: excess load is shed, errors stay confined to shed requests, and latencies return to hold values within 5 minutes |
| Memory | API resident memory growth under 5% across the hold after warm-up; no growth across the soak |
| Database | Connections at most 70% of the limit; Postgres CPU under 70% during the hold |
| Fencing and idempotency | During the failover and the Redis restart, no conversation has two writers and no repeated idempotency key starts a duplicate run |
| Isolation under load | Synthetic isolation probes report 0 cross-boundary reads |
| Cost | Cost per run at p95 within the G9 cap |

## Appendix A: Validation summary of the v4 design

**Direction is right.** Baseline-first, bounded orchestration, provenance, tenant isolation and explicit tool health are the correct foundations for an agentic RAG system, and the diagram's blocks and nine steps are all present.

**What blocked it.** (1) The document was truncated, so retrieval, ingestion, citations, security, API, evaluation and the build plan were referenced but absent. (2) The two most important quality controls, sufficiency and citation support, were defined in terms that a model could satisfy without the answer being supported. (3) The streaming design either delayed the first token or left validation undefined. (4) Security was stated as intent, not as enforceable mechanism. (5) The Rust-specific failure modes (nested async results, cancellation, pooled-connection RLS, blocking inference) were not specified.

**How v5 addresses it.** Each finding has a mechanism, a section and a test (§27). The principal changes in design philosophy are: sufficiency is checked by code against verbatim quotes, not by model confidence; nothing is emitted before a sentence passes grounding checks; isolation is enforced by types and RLS in every transaction; and every uncertain technical claim is tied to a spike.

**Residual risks that only measurement can close** (Stage 0 and 1): parsing quality in Rust, pgvector recall under filters, NLI accuracy and latency on the real corpus, CPU contention between inference and serving, and provider behaviour on cancellation and billing. The design has fallbacks for each (§23), but the numbers in §2 and §6.5 stay targets until measured.

## Appendix B: Rust development challenges register

| # | Challenge | Why it bites | Mitigation | Verified by |
| --- | --- | --- | --- | --- |
| B1 | Nested async results (`timeout` → `Result` → `JoinError` → provider error) | Easy to lose a failure class or mislabel a bug as an outage | One `collect` function and `CallOutcome` enum; exhaustive tests (§8.2) | Unit and property tests |
| B2 | Cancellation correctness | Dropped futures can leave partial state; `spawn_blocking` cannot be aborted | Cancel-safe tools, `CancellationToken`, deadlines in blocking code, non-cancellable terminal persistence (§20.3, §9.3) | Chaos tests, S5, S7 |
| B3 | Errors: expected failure vs bug | A uniform "tool failed" hides defects | Two-layer `ToolError`; alerts on `Internal` (§8.2) | Review rule, alert tests |
| B4 | Pooled-connection RLS | A session-level tenant setting leaks across requests | Transaction-local `set_config`; `TenantTx` type; non-owner role without `BYPASSRLS` (§16.3) | Isolation suite, S6 |
| B5 | `sqlx` compile-time queries need a database or offline cache | Broken CI and slow builds | Committed `.sqlx` data, CI check, `QueryBuilder` for dynamic filters (§20.6) | CI |
| B6 | CPU-bound inference on the async runtime | Request latency spikes | Dedicated blocking pool, batching, separate process option (§20.5) | S8, load test |
| B7 | Parsing ecosystem gaps (scans, tables) | Poor chunks degrade everything downstream | `Parser` trait, sidecar option, parser limits and sandboxing (§12.1, §12.8) | S1 |
| B8 | No official offline tokenizer or SDK for the chosen generator **[verify]** | Budget errors, hand-written client bugs | Conservative estimator with usage feedback; thin tested `LlmClient` (§7, §20.4) | S7 |
| B9 | Crate maturity (`rmcp`, ONNX wrappers, `apalis`, pgvector features) | API churn, missing features | Wrap behind own traits, pin versions, contract tests, fallbacks (§20.7) | S4 and CI |
| B10 | SSE behaviour (buffering proxies, HTTP/2, disconnect detection) | Silent buffering, leaked runs | Bounded channel, `DropGuard`, keep-alive, proxy settings, fetch client (§15.3) | S5, load test |
| B11 | Long compile times in a large workspace | Slow iteration | `cargo-chef`, `sccache`, fast linker, feature-gated heavy crates (§20.8) | CI metrics |
| B12 | Shared large strings and memory growth | Clone overhead and memory pressure | `Arc<str>`, `Bytes`, bounded buffers and evidence size caps (§20.9) | Benchmarks, load test |
| B13 | Panics in spawned tasks | Lost runs or poisoned state | `panic = "unwind"` in the API, `JoinError` handling, catch-panic layer (§20.2) | Fault-injection tests |
| B14 | Trace context lost across `spawn` | Untraceable runs | `.instrument(span)` on every spawned task (§17.5) | Trace tests |
| B15 | Schema generation drift (`schemars` vs provider JSON Schema support) | Provider rejects or ignores constraints | Schema acceptance fixtures; `deny_unknown_fields`; validate on both sides (§20.4) | CI fixtures |
| B16 | Async trait objects and dynamic dispatch | Boxed futures cost and `dyn` limits | `async_trait` for `dyn Tool`; measure; keep hot paths concrete | Benchmarks |
| B17 | Backpressure with slow clients | Memory growth, stuck runs | Bounded channels with send timeout, cancel on timeout (§15.3) | Load test with slow readers |
| B18 | Migration role vs application role mix-ups | RLS bypass | Separate roles; app role cannot own tables; CI check on role attributes (§16.3) | Isolation suite |
| B19 | Blocking work outlives cancellation | CPU wasted after disconnects; contention | Micro-batches with cancellation checks, bounded queue, admission control, killable child-process parsing (§20.5) | S8, chaos test G6 |
| B20 | Retry loops overspend budgets or ignore cancellation | Cost and latency blow-ups; cancelled runs sleeping | The wave reservation is passed in; retries reserve last, after the deadline check and a cancellable sleep (§6.3) | Property test on `Budget`, paused-time tests, S9 |
| B21 | Stale lock owners and non-monotonic fences | Two runs writing one conversation | Database-issued lease and fence matched on every write; Redis outside the correctness path; synchronous standby (§9.5) | S11, failover drill |
| B22 | Middleware breaks streaming (compression, total timeouts, buffering) | Silent buffering, dropped streams | Separate streaming router and layer list; deployed-stack tests (§15.3) | S5, gate G4 |
| B23 | RLS helper functions and settings misconfigured | Fail-open access | `NULLIF` and `COALESCE` helpers, named tests for missing and malformed settings (§16.3) | Isolation suite |
| B24 | Self-contained unit streaming depends on provider JSON-lines reliability | Malformed units, dropped answers | One format per release (JSON Lines); buffered structured-output fallback; malformed line equals failed unit (§13.0) | S7, S12 |
| B25 | RLS policy subqueries run under the caller's RLS and can be slow or empty | Fail-open workarounds (`SECURITY DEFINER`) or slow retrieval | `acl_self` own-grants policy, indexed lookups, version-match fallback (§16.3) | Isolation suite, S10 |
| B26 | Samples that were never compiled | Contracts that do not type-check or hide logic errors | Compile and test every sample before merge; fix the document with the code (§24.2) | S9 |
| B27 | Trusting model-supplied labels in a safety gate | A model can label a claim `meta` and skip checks | No model `meta`; all model units factual; system text from templates (§13.0) | S12 fuzzing |

## Appendix C: Validation summary of the v5 design

**What was right.** The v5 architecture, the Rust module boundaries, the typed state machine, the two-layer tool error model, `TenantTx`, bounded channels and the baseline-first approach were sound and are unchanged.

**What was not safe.** Five items could produce a wrong or unauthorised answer: (1) the evaluator fallback could approve an answer from rerank scores alone; (2) RLS enforced the tenant but not the principal's ACL; (3) cache keys did not capture the full authorisation context; (4) the router fallback and example could drop or misroute exact and live questions; (5) "just guess" could substitute an assumption for evidence. All five now fail closed (§5, §16.3, §16.4, §4, §11.2).

**What was overstated.** The verification gate was described as proof, and the evaluation gates were not powered for the slices they judged. v6 describes the gate as a screen with measured bounds per tier (§13.5) and sizes each comparison before it is run (§19.8).

**What was unfinished.** Rust items (orphaned blocking work, retry reservation, provider-failure labelling, idempotency and fencing, SSE middleware, RLS test precision) and the unverified assumptions are now specified or given defaults with switch criteria (§20.5, §6.3, §3.4, §9.5, §15.3, §16.3, §24.4).

**Residual risk that only measurement can close.** Parsing quality on your corpus, pgvector recall under your filters, NLI accuracy on your claims, CPU contention, and provider cancellation and billing behaviour. These now have defaults, fallbacks and gates (§24.4, §28.10); the figures in this document remain targets until the built system meets them.

## Appendix D: Validation summary of the v6 design

**What was right.** The staged build, the typed state machine, fail-closed evaluator, conservative router fallback, `TenantTx`, bounded channels, the grounding screen language and the decision defaults held up and are unchanged.

**What was wrong.** Two items broke a headline guarantee. (1) The chunk RLS policy read a denormalised array that the document admitted could lag, so a revoked user could receive restricted text before `ACL_RECHECK`; v7 evaluates the live ACL, deletion and current-version rules inside the policy (§16.3). (2) The Redis `INCR` fence could repeat or move backwards after a failover and advanced in the database only on writes; v7 issues the fence and lease from Postgres and requires every write to match them (§9.5).

**What was inconsistent.** The generator emitted JSON-line units while the gate split sentences and read inline markers, and a model-chosen `meta` role could skip citation checks. v7 has one format, parsed and schema-validated, with no model-controlled exemption (§13.0, §13.2). The retry sample double-reserved, used undefined names and slept without observing cancellation; v7 corrects it and requires it to compile (§6.3, S9).

**What was unmeasurable.** The load gate had no number. v7 defines a reference profile with pass criteria (§28.11).

**What I could not verify here.** The Rust and SQL samples were not compiled or executed because the authoring environment cannot reach the crate registry or a Postgres instance. They are specified as contracts and gated by Spikes S9 to S12. Every latency, capacity and accuracy figure remains a target until the built system meets it.

**Residual risk that only measurement can close.** Parsing quality on your corpus, pgvector recall under live-ACL filters, NLI accuracy on your claims, CPU contention, provider cancellation and billing behaviour, and the real concurrency your users generate.

## Appendix E: Validation summary of the v7 design

**What held.** The live-ACL policy, the database lease, the single claim-unit format, the load profile and the spike plan were sound and are unchanged in structure.

**What was open.** Seven contracts were still loose: where group and role membership comes from and how stale it may be; what RLS settings do and do not protect against; whether plan coverage reaches conditions, comparisons and output requests; parser invariants for the first unit and ordering; the clock used for lease expiry; who owns a call reservation; and whether one format is used end to end. Each now has a rule, a section and a test (§27.5).

**What changed in the plan.** The build order now requires the baseline to be hardened and measured before any agent feature starts, adds agent features one at a time behind flags, and records the product and security boundary before coding (§24).

**What only measurement can close.** The same list as Appendix D, plus the real membership-service latency and availability, and the real request mix.

## Appendix F: Validation summary of the v10 design and the merge into v11

**What v10 got right.** The per-decision register (rule, owner, test, fallback), the five-minute fail-closed membership bound, the per-table role matrix, removal of pinned versions, the open-arrival load model and the plain statement that no tests have been run. All are kept.

**What v10 could not do alone.** It was not self-contained (its load test and gate list pointed to v8 and "G0 to G10" was never defined), its IDs collided with the spike IDs, its +2 point promotion threshold could not be reached with 300 cases, its routing text disagreed with itself, its retry rule could repeat text already shown, and it named no models. v11 merges v10's register into v8's detailed contracts and fixes each of these (§27.6).

**What the owner's answers changed.** A narrower product: documents only, everything else referred to the Business Owner, English only, no live-data or web tools, no model-knowledge answers. A smaller workload: 10 concurrent streams and 50 users. Tighter limits: 1% in production and 2% in staging. Two named models.

**What the model choices introduced.** (1) nomic-embed-text-v1.5 is English-only and produces 768 dimensions, which sets the language scope and the vector size. (2) Nemotron 3 Super is a 120-billion-parameter model that NVIDIA lists as needing about eight H100 GPUs to self-host, and its NVIDIA-hosted API is a trial service. Hosting, region and data terms are therefore open items OI-2 and OI-3, and they are high-risk. (3) One model family does answering, reranking and evaluation, so the evaluator is not an independent judge; the exact-quote, number and entity checks and the separate local NLI model carry the safety bound, and the `high_risk` and `regulated` tiers stay off.

**Why the design is not marked complete.** Under D10 a high-risk open item blocks completion. OI-1 (named owners and deputies), OI-2 (model hosting) and OI-3 (region and provider terms) are open.

**What I could not verify here.** The Rust and SQL samples were not compiled or run (no access to the crate registry or a database in the authoring environment); S9 to S12 gate them. The behaviour of Nemotron 3 Super for JSON Lines output, cancellation and listwise reranking is untested (S7, S14, S15). The customer's OIDC provider behaviour is unknown (S16).

**What only measurement can close.** Parsing and conversion quality on the real files, filtered recall, NLI accuracy, model quality, speed and cost at the chosen host, and the real request pattern of the 50 users.
