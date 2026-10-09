#!/usr/bin/env bash
# Self-test RAG connection probes (pgvector local + mock HTTP engines).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/src-tauri"
export MONOLITH_TEST_PG="${MONOLITH_TEST_PG:-postgresql://muqiang@127.0.0.1:5432/postgres}"
exec cargo test rag_probe_tests -- --nocapture --test-threads=1
