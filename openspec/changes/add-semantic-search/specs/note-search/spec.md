# Spec Delta

## ADDED Requirements

### Requirement: Semantic search
With the `semantic` feature, `search --semantic <QUESTION>` SHALL run a hybrid query against the notebook's Qdrant collection, combining dense (meaning) and sparse (exact term) results with Reciprocal Rank Fusion. It SHALL print, for each result, the note date, title, heading, path, and a snippet. It SHALL accept `--keyword` (repeatable, all must match), `--kind`, `--since`, `--until`, `--limit` (default 10), `--all-kinds`, and `--json`. A note SHALL appear at most twice in the results.

#### Scenario: Paraphrase match
- **WHEN** a note says "we should progressively roll it out" and the user runs `search --semantic "rollout risk"`
- **THEN** that note is among the results

#### Scenario: Exact term match
- **WHEN** the user runs `search --semantic "4.45.1.0"`
- **THEN** notes containing `4.45.1.0` rank above notes that do not

#### Scenario: Transcripts left out by default
- **WHEN** the user runs `search --semantic "budget"` without `--kind transcript` or `--all-kinds`
- **THEN** no result comes from `src/notes/zoom-transcripts/`

#### Scenario: Date filter
- **WHEN** the user runs `search --semantic "roadmap" --since 2026-10-01`
- **THEN** every result has a note date on or after 2026-10-01

#### Scenario: Qdrant not reachable
- **WHEN** Qdrant does not answer at the configured URL
- **THEN** the command exits non-zero with a message naming the URL and suggesting plain `search`
