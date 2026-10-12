## MODIFIED Requirements

### Requirement: Notebook initialization
The `init [DIR]` command SHALL create an mdBook-compatible notebook in `DIR` (default: current directory) containing `book.toml`, `src/SUMMARY.md`, `src/README.md`, `src/notes/`, `src/notes/zoom-summaries/`, `src/notes/zoom-transcripts/`, `src/reference/`, `src/png/`, and a `.ignore` file that excludes the rendered `book/` output.

#### Scenario: Initialize an empty directory
- **WHEN** the user runs `aa-denote-mdbook-tool init ~/work-notes` and `~/work-notes` does not exist or is empty
- **THEN** the directory is created with `book.toml`, `.ignore`, `src/SUMMARY.md`, `src/README.md`, `src/notes/`, `src/notes/zoom-summaries/`, `src/notes/zoom-transcripts/`, `src/reference/`, and `src/png/`
- **AND** `mdbook build` succeeds in that directory without further edits

#### Scenario: Refuse to overwrite an existing notebook
- **WHEN** the user runs `aa-denote-mdbook-tool init` in a directory that already contains `book.toml`
- **THEN** the command exits with a non-zero status and an error naming the existing file
- **AND** no existing file is modified

### Requirement: Folder per note kind
Each note kind SHALL map to exactly one folder: `manual` to `src/notes/`, `summary` to `src/notes/zoom-summaries/`, `transcript` to `src/notes/zoom-transcripts/`, and `reference` to `src/reference/`. Each supported attachment extension (`png`, `jpg`, `svg`, `pdf`, `sql`, `json`) SHALL map to a folder under `src/` named after the extension, for example `src/png/` and `src/pdf/`.

#### Scenario: Manual note location
- **WHEN** a note of kind `manual` is created
- **THEN** its file is written directly inside `src/notes/`

#### Scenario: Zoom transcript location
- **WHEN** a note of kind `transcript` is created or imported
- **THEN** its file is written inside `src/notes/zoom-transcripts/`

#### Scenario: Reference note location
- **WHEN** a note of kind `reference` is created or imported
- **THEN** its file is written directly inside `src/reference/`

#### Scenario: Kind follows the folder
- **WHEN** a Denote-named note sits in `src/reference/`
- **THEN** the tool treats it as kind `reference`, for example in `search --kind reference`

#### Scenario: Attachment location follows extension
- **WHEN** the user attaches `contract.pdf` and `query.sql`
- **THEN** the PDF is written inside `src/pdf/` and the SQL file inside `src/sql/`

