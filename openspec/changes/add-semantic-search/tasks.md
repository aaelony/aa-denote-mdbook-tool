# Tasks

## 1. Feature flag and dependencies

- [x] 1.1 Add Cargo feature `semantic` to both crates (binary feature enables the core one) and add `fastembed` and `uuid` (v5) as optional core dependencies. Verify `cargo build` and `cargo build --features semantic` both succeed.

## 2. Chunking and sparse vectors (no Qdrant needed)

- [x] 2.1 Add `semantic/chunk.rs`: heading-based chunks for manual, summary, and reference notes; speaker-turn chunks for transcripts; the title, date, and section prefix. Verify with `test_chunk_splits_at_headings`, `test_chunk_long_section_overlaps_one_paragraph`, and `test_chunk_transcript_groups_turns`.
- [x] 2.2 Add `semantic/sparse.rs`: tokenizer that keeps `4.45.1.0` and `lrs_pygen` whole, stable token hash, BM25 term weights. Verify with `test_tokenize_keeps_versions_and_identifiers` and `test_sparse_vector_is_stable_across_runs`.

## 3. Embedder

- [x] 3.1 Add `semantic/embed.rs` wrapping fastembed `BGESmallENV15` with the cache under `dirs::cache_dir()`. Verify with an ignored-by-default test `test_embed_returns_384_dims` (needs the model download).

## 4. Qdrant REST client

- [x] 4.1 Add `semantic/qdrant.rs` on `ureq`: ensure collection (dense 384 cosine, sparse `terms` with idf, payload indexes), upsert, delete by identifier, hybrid query with filters. Verify with `test_qdrant_roundtrip` in `tests/semantic.rs`, which starts `qdrant` on a free port in a temp dir and is skipped when `qdrant` is not on `PATH`.
- [x] 4.2 Read `AA_DENOTE_MDBOOK_TOOL_QDRANT_URL` with the default `http://127.0.0.1:6333`. Unreachable server gives a clear error naming the URL. Verify with `test_semantic_search_without_qdrant_explains`.

## 5. Index command

- [x] 5.1 Add `semantic/state.rs` (`.semantic-index.json`, content hashes, chunk counts) and add the file to the notebook `.ignore`. Verify with `test_index_state_roundtrip`.
- [x] 5.2 Add `index [--rebuild]` to `src/cli.rs`: full and incremental runs, deletes for removed notes, model-name check. Verify with `test_index_skips_unchanged_notes`, `test_index_removes_deleted_note`, and `test_index_rebuild_recreates_collection`.

## 6. Search

- [x] 6.1 Add `search --semantic` with `--kind`, `--keyword`, `--since`, `--until`, `--limit`, `--all-kinds`, `--json`. Transcripts are left out unless `--kind transcript` or `--all-kinds`. Verify with `test_semantic_search_finds_paraphrase`, `test_semantic_search_finds_exact_version_string`, `test_semantic_search_excludes_transcripts_by_default`, and `test_semantic_search_date_filter`.

## 7. Keep the index current

- [x] 7.1 Call the per-note index update after `new`, `import`, `attach`, `rename`, and `links fix` when the state file exists. Failures warn on standard error only. Verify with `test_import_updates_index` and `test_write_succeeds_when_qdrant_down`.

## 8. MCP

- [x] 8.1 Add the `semantic_search` tool to `src/mcp.rs` (built only with the feature). Verify with `test_mcp_semantic_search_returns_structured_results` and update `test_mcp_tools_list_is_exact` for both feature settings.

## 9. Documentation and checks

- [x] 9.1 README: start Qdrant as a launchd service bound to `127.0.0.1`, run `index`, example queries, MCP tool. Verify every command shown matches `--help`.
- [ ] 9.2 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, and `just crap`, and verify all pass.
- [ ] 9.3 Index the real notebook with `--book ~/Documents/meetings-book` and check 10 real questions. Record which ones fail and tune chunk size or prefetch limits.
