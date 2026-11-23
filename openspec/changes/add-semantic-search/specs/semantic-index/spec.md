# Spec Delta

## Purpose

Keeps a local Qdrant collection of note chunks in step with the notebook so that `search --semantic` and the `semantic_search` MCP tool can rank notes by meaning and by exact terms.

## ADDED Requirements

### Requirement: Index command
With the `semantic` feature, `index` SHALL chunk every note in `src/`, embed each chunk on the local machine, and store it in the collection `denote_<notebook dir name>`. A repeated run SHALL process only notes whose content hash changed and SHALL remove points for notes that no longer exist. `--rebuild` SHALL drop and recreate the collection.

#### Scenario: Incremental run
- **WHEN** one note changed since the last `index`
- **THEN** only that note's chunks are deleted and stored again

#### Scenario: Deleted note
- **WHEN** a note file was removed since the last `index`
- **THEN** its points are removed from the collection

#### Scenario: Different notebook copy
- **WHEN** the user runs `index` in a copy of the notebook in another directory
- **THEN** the copy uses its own collection and the original collection is unchanged

### Requirement: Local processing
Embedding SHALL run on the local machine. No note content SHALL be sent to any service other than the configured Qdrant URL.

#### Scenario: Offline after first run
- **WHEN** the model is cached and the network is off
- **THEN** `index` and `search --semantic` still work against a local Qdrant

### Requirement: Write commands keep the index current
When `.semantic-index.json` exists, `new`, `import`, `attach`, `rename`, and `links fix` SHALL update the index for the notes they change. An indexing failure SHALL print a warning on standard error and SHALL NOT change the command's exit status.

#### Scenario: Qdrant down during import
- **WHEN** Qdrant is not running and the user runs `import --kind summary file.md`
- **THEN** the note is created, a warning is printed, and the command exits 0
