# Proposal

## Why

The `search` command finds exact text only. A question such as "what did Mark say about rollout risk" misses a note that says "progressively roll it out". The notebook now holds hundreds of meeting notes, summaries, and transcripts, so search by meaning would save time. It must keep exact matches for names, versions, and identifiers such as `Emirates`, `4.45.1.0`, and `lrs_pygen`, which embedding models handle poorly.

Qdrant is already installed on the author's machine. The notes contain customer names, so all processing stays local: no cloud embedding API.

## What Changes

- New `index` subcommand: `aa-denote-mdbook-tool [--book DIR] index [--rebuild]` reads every note in `src/`, splits it into chunks, embeds each chunk on the local machine, and stores it in a local Qdrant collection. Repeated runs process only notes whose content changed and remove notes that were deleted.
- New `--semantic` flag on `search`: `search --semantic "<question>"` runs a hybrid query in Qdrant (dense vectors for meaning, sparse vectors for exact terms, fused with Reciprocal Rank Fusion) and prints the best chunks with note title, date, heading, path, and a short snippet. The existing `--keyword` and `--kind` filters apply. New filters: `--since` and `--until` (dates), `--limit` (default 10).
- New MCP tool `semantic_search` with the same inputs, returning structured results.
- The write commands (`new`, `import`, `attach`, `rename`, `links fix`) update the index for the notes they change, when indexing is set up. Indexing failures print a warning and never fail the write.
- If Qdrant is not reachable, `search --semantic` exits non-zero with a message that names the Qdrant URL and suggests plain `search`.
- The feature is behind a Cargo feature `semantic` so the default binary stays small. The release build for the author enables it.
- Out of scope:
  - running or installing Qdrant (the README documents a launchd service);
  - cloud embedding providers;
  - indexing attachments (PDF, images);
  - re-ranking models.

Assumptions made without asking (change them with `/opsx:update` if they are wrong):
- Transcripts are indexed, but `search --semantic` leaves them out unless `--kind transcript` or `--all-kinds` is given. They are most of the text and would crowd out summaries and notes.
- Qdrant URL defaults to `http://127.0.0.1:6333`, overridable with `AA_DENOTE_MDBOOK_TOOL_QDRANT_URL`. One collection per notebook, named from the notebook directory.

## Capabilities

### New Capabilities
- `semantic-index`: the `index` command, chunking rules, what each indexed chunk stores, incremental updates (including updates from write commands), and the Qdrant collection layout.

### Modified Capabilities
- `note-search`: `search --semantic` with hybrid ranking and the new filters.
- `mcp-server`: the `semantic_search` tool.

## Impact

- `denote-mdbook-core/src/semantic/` (new, behind feature `semantic`): chunker, sparse term vectors, embedder wrapper, Qdrant REST client on the existing `ureq` dependency, and index state.
- New dependency: `fastembed` (local ONNX embeddings). No Qdrant client crate: the REST API through `ureq` and `serde_json` is enough and avoids gRPC dependencies.
- `src/cli.rs`: `Index` subcommand and `search --semantic` flags. `src/mcp.rs`: one new tool.
- First run downloads the embedding model (about 130 MB) into the user cache directory.
- New integration tests run against a Qdrant started by the test harness and are skipped when the `qdrant` binary is not on `PATH`.
- README: setup section (start Qdrant, run `index`, register MCP).
