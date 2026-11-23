# Design

## Overview

```
notes (src/**/*.md) --> chunker --> dense embedder (fastembed, local)  --+
                                \-> sparse term vector (in-house BM25) --+--> Qdrant collection
query ------------------------------> same two encoders -----------------> hybrid query (RRF) --> results
```

All code lives in `aa-denote-mdbook-tool-core` under `src/semantic/`, compiled only with the `semantic` feature. The binary crate adds the CLI flags and the MCP tool.

## Decisions

### Qdrant over REST, not gRPC
The core crate already depends on `ureq` and `serde_json`. The REST API covers everything needed: create collection, upsert points, delete by filter, scroll, and the Query API with `prefetch` + `fusion: rrf`. The official `qdrant-client` crate pulls in `tonic` and `prost` and grows the binary, which the release profile tries to keep small.

### Embeddings: fastembed, `BAAI/bge-small-en-v1.5`
- Runs in-process on ONNX Runtime. No server to keep running.
- 384 dimensions, good English retrieval quality for its size, fast on Apple silicon.
- Model files are cached under `dirs::cache_dir()/aa-denote-mdbook-tool/models`.
- The model name is stored in the collection metadata. If it changes, `index` refuses to mix vectors and asks for `--rebuild`.

### Sparse vectors: in-house BM25 terms with Qdrant IDF
- Tokenize: lowercase, split on non-alphanumeric characters but keep `.`, `_` and `-` inside a token, so `4.45.1.0` and `lrs_pygen` stay whole.
- Index = a stable 32-bit hash of the token. Value = BM25 term-frequency weight with `k1 = 1.2`, `b = 0.75`, using the chunk length and a stored average length.
- The collection's sparse vector uses `modifier: idf`, so Qdrant applies inverse document frequency at query time. No model download.

### Chunking
- Front matter is parsed (existing `frontmatter.rs`) and not embedded.
- Manual notes, summaries, and reference notes: split at `#`, `##`, and `###` headings with `pulldown-cmark`. A section longer than about 400 tokens is split again at paragraph boundaries, with one paragraph of overlap.
- Transcripts: group consecutive speaker turns until about 400 tokens, with one turn of overlap.
- The embedded text is prefixed with `Title: <title>. Date: <YYYY-MM-DD>. Section: <heading>.` so a chunk makes sense on its own.

### Payload per point
| Field | Type | Indexed | Use |
|---|---|---|---|
| `identifier` | keyword | yes | delete and update by note |
| `title`, `heading` | text | no | display |
| `kind` | keyword | yes | `--kind` filter |
| `keywords` | keyword[] | yes | `--keyword` filter |
| `date` | datetime | yes | `--since` / `--until` |
| `parent` | keyword | yes | group summaries with their meeting |
| `path` | keyword | no | display, links |
| `chunk` | integer | no | order |
| `text` | text | no | snippet |

Point ID: UUID v5 of `identifier + "#" + chunk number`. Stable across runs.

### Incremental updates
- State file `.semantic-index.json` in the notebook root (added to `.ignore`): for each identifier, the SHA-256 of the file content and the chunk count.
- `index` compares against the state. Changed note: delete its points by `identifier` filter, then upsert new chunks. Removed note: delete its points. Unchanged note: skip.
- `--rebuild` drops and recreates the collection.
- Write commands call the same per-note update when the state file exists. A failure prints a warning to standard error and the command still succeeds.

### Collection
- Name: `denote_<sanitized notebook dir name>`, for example `denote_meetings_book`. Two copies of a notebook in different directories never share a collection, which matters after the `meetings-backup` incident.
- Vectors: `dense` (384, cosine), sparse `terms` (modifier idf).

### Query
```json
{
  "prefetch": [
    {"query": [/* dense */], "using": "dense", "limit": 40},
    {"query": {"indices": [], "values": []}, "using": "terms", "limit": 40}
  ],
  "query": {"fusion": "rrf"},
  "filter": {"must": [/* kind, keywords, date range */]},
  "limit": 10,
  "with_payload": true
}
```
Results are grouped so one note appears at most twice.

### Output
CLI, one result per block:
```
2026-11-19  todo-thurs-nov-19 > Git Integration next steps   (score 0.031)
  src/notes/20261119T000000--todo-thurs-nov-19.md
  ...we should progressively roll it out...
```
`--json` prints the same fields as an array, for scripts.

## Risks

- **ONNX Runtime linking.** `fastembed` uses `ort`, which downloads a prebuilt ONNX Runtime at build time. That works on macOS arm64. The feature flag keeps it out of builds that do not need it.
- **Indexing time.** About 355 files and roughly 2,000 chunks. The first run should take under two minutes on Apple silicon, and later runs only a few seconds.
- **Two notebook copies.** The collection name comes from the notebook directory, and every command requires a found notebook (`--book`, env var, or `book.toml`). Indexing the backup copy creates a separate collection rather than mixing results.
