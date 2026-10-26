# Production-Grade Agentic RAG System in Rust & Astro

A production-grade, native Agentic Retrieval-Augmented Generation (Agentic RAG) system built from scratch in **Rust** (Axum, Tokio, Tantivy, SQLite, Rig) integrated with an **Astro** conversational UI.

## Architecture Highlights

- **Native Rust Runtime**: 100% native execution (no Docker or Docker Compose required).
- **Explicit Agentic Control Loop**: Bounded, observable retrieval loop that evaluates evidence sufficiency, performs adaptive query rewriting, verifies citations, and terminates on calibrated confidence thresholds.
- **Hybrid Retrieval**: Native Tantivy BM25 full-text indexing combined with structured schema scopes.
- **SQLite Persistence**: Embedded durable relational store for conversations, messages, document metadata, and schema migration tracking.
- **Server-Sent Events (SSE)**: Real-time stream protocol exposing stage transitions (`planning`, `retrieving`, `evaluating`, `refining`, `building_context`, `generating`, `completed`).
- **Astro Frontend**: Modern conversational interface with live execution timeline, multi-session tabs, document catalog manager, and citation cards.

## Repository Layout

```text
├── backend/
│   ├── Cargo.toml
│   ├── .cargo/config.toml
│   ├── src/
│   │   ├── main.rs            # Axum server entry point
│   │   ├── lib.rs             # Core library exports
│   │   ├── config/            # Typed configuration
│   │   ├── models/            # Domain models (Document, Chunk, Evidence, Citation)
│   │   ├── db/                # SQLite persistence and migration engine
│   │   ├── ingestion/         # PDF, TXT, MD parser and sliding-window chunker
│   │   ├── retrieval/         # Tantivy BM25 hybrid search engine
│   │   ├── evaluator/         # Deterministic evidence sufficiency guardrails
│   │   ├── orchestrator/      # Adaptive retry & retrieval orchestrator
│   │   └── api/               # REST + SSE endpoints
│   └── tests/
│       └── integration_tests.rs
├── frontend/
│   ├── astro.config.mjs       # Dev server proxy configuration
│   ├── package.json
│   └── src/
│       ├── pages/index.astro  # Conversational AI workspace UI
│       └── lib/rag/           # Typed client and SSE parser
└── README.md
```

## Quickstart

### Prerequisites
- Rust 1.80+ (MSVC toolchain on Windows)
- Node.js 20+

### 1. Backend Setup
```bash
cd backend
cargo run --bin backend
```
The Axum API starts at `http://127.0.0.1:8080`.

### 2. Frontend Setup
```bash
cd frontend
npm install
npm run dev
```
Open [http://localhost:4321](http://localhost:4321) in your browser.

## Testing
Run the complete integration and regression suite:
```bash
cd backend
cargo test
```
