# Spec Delta

## ADDED Requirements

### Requirement: Semantic search tool
With the `semantic` feature, the server SHALL expose `semantic_search` with inputs `query` (required), `keywords`, `kind`, `since`, `until`, `limit`, and `all_kinds`. It SHALL return a list of results, each with identifier, title, date, kind, heading, path, snippet, and score. If Qdrant is not reachable, the tool SHALL return a tool error and the server SHALL keep running.

#### Scenario: Assistant asks a question
- **WHEN** a client calls `semantic_search` with `query: "what did Mark say about rollout"`
- **THEN** the result lists matching chunks with their note paths

#### Scenario: Feature not built
- **WHEN** the binary was built without the `semantic` feature
- **THEN** `tools/list` does not include `semantic_search`
