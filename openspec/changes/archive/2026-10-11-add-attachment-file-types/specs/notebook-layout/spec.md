# Spec Delta

## MODIFIED Requirements

### Requirement: Folder per note kind
Each note kind SHALL map to exactly one folder: `manual` to `src/notes/`, `summary` to `src/notes/zoom-summaries/`, and `transcript` to `src/notes/zoom-transcripts/`. Each supported attachment extension (`png`, `jpg`, `svg`, `pdf`, `sql`, `json`) SHALL map to a folder under `src/` named after the extension, for example `src/png/` and `src/pdf/`.

#### Scenario: Manual note location
- **WHEN** a note of kind `manual` is created
- **THEN** its file is written directly inside `src/notes/`

#### Scenario: Zoom transcript location
- **WHEN** a note of kind `transcript` is created or imported
- **THEN** its file is written inside `src/notes/zoom-transcripts/`

#### Scenario: Attachment location follows extension
- **WHEN** the user attaches `contract.pdf` and `query.sql`
- **THEN** the PDF is written inside `src/pdf/` and the SQL file inside `src/sql/`

### Requirement: Missing folders are recreated
When a command needs a kind folder or an attachment folder that does not exist in an existing notebook, the tool SHALL create it rather than fail.

#### Scenario: Deleted png folder
- **WHEN** `src/png/` was deleted and the user adds an image
- **THEN** `src/png/` is recreated and the image is written into it

#### Scenario: First attachment of a type
- **WHEN** the notebook has no `src/json/` folder and the user attaches `payload.json`
- **THEN** `src/json/` is created and the file is written into it
