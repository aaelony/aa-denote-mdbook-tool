# Spec Delta

## Purpose

Defines the on-disk structure of a aa-denote-mdbook-tool notebook so that it is a valid mdBook project, keeps each kind of note in a predictable folder, and can be located from anywhere inside it.

## ADDED Requirements

### Requirement: Notebook initialization
The `init [DIR]` command SHALL create an mdBook-compatible notebook in `DIR` (default: current directory) containing `book.toml`, `src/SUMMARY.md`, `src/README.md`, `src/notes/`, `src/notes/zoom-summaries/`, `src/notes/zoom-transcripts/`, `src/png/`, and a `.ignore` file that excludes the rendered `book/` output.

#### Scenario: Initialize an empty directory
- **WHEN** the user runs `aa-denote-mdbook-tool init ~/work-notes` and `~/work-notes` does not exist or is empty
- **THEN** the directory is created with `book.toml`, `.ignore`, `src/SUMMARY.md`, `src/README.md`, `src/notes/`, `src/notes/zoom-summaries/`, `src/notes/zoom-transcripts/`, and `src/png/`
- **AND** `mdbook build` succeeds in that directory without further edits

#### Scenario: Refuse to overwrite an existing notebook
- **WHEN** the user runs `aa-denote-mdbook-tool init` in a directory that already contains `book.toml`
- **THEN** the command exits with a non-zero status and an error naming the existing file
- **AND** no existing file is modified

### Requirement: Book configuration registers the preprocessor
The generated `book.toml` SHALL set the book title (from `--title`, default the directory name), set `src = "src"`, and register `aa-denote-mdbook-tool` as an mdBook preprocessor so front matter is handled at render time.

#### Scenario: Generated book.toml
- **WHEN** the user runs `aa-denote-mdbook-tool init --title "Work Notes"`
- **THEN** `book.toml` contains `title = "Work Notes"` under `[book]`
- **AND** contains a `[preprocessor.aa-denote-mdbook-tool]` table whose command invokes `aa-denote-mdbook-tool preprocess`

### Requirement: Folder per note kind
Each note kind SHALL map to exactly one folder: `manual` to `src/notes/`, `summary` to `src/notes/zoom-summaries/`, and `transcript` to `src/notes/zoom-transcripts/`. Images SHALL live in `src/png/`.

#### Scenario: Manual note location
- **WHEN** a note of kind `manual` is created
- **THEN** its file is written directly inside `src/notes/`

#### Scenario: Zoom transcript location
- **WHEN** a note of kind `transcript` is created or imported
- **THEN** its file is written inside `src/notes/zoom-transcripts/`

### Requirement: Notebook root discovery
Every command other than `init` SHALL locate the notebook root using, in order: the `--book <DIR>` option, the `AA_DENOTE_MDBOOK_TOOL_DIR` environment variable, then the nearest ancestor of the current directory that contains `book.toml`.

#### Scenario: Run from a subdirectory
- **WHEN** the user runs `aa-denote-mdbook-tool new --title "Standup"` from `~/work-notes/src/notes/zoom-summaries`
- **THEN** the notebook root is resolved to `~/work-notes`

#### Scenario: Explicit option wins
- **WHEN** `AA_DENOTE_MDBOOK_TOOL_DIR=~/a` is set and the user passes `--book ~/b`
- **THEN** the notebook root is `~/b`

#### Scenario: No notebook found
- **WHEN** no option, no environment variable, and no ancestor `book.toml` exist
- **THEN** the command exits non-zero with an error suggesting `aa-denote-mdbook-tool init` or `--book`

### Requirement: Missing folders are recreated
When a command needs a kind folder or `src/png/` that does not exist in an existing notebook, the tool SHALL create it rather than fail.

#### Scenario: Deleted png folder
- **WHEN** `src/png/` was deleted and the user adds an image
- **THEN** `src/png/` is recreated and the image is written into it
