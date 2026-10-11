# Spec Delta

## Purpose

Turns the notebook into a readable mdBook: generates `SUMMARY.md` from the notes on disk, nests AI summaries and transcripts under the meeting they belong to, hides front matter in the rendered pages, and runs `mdbook` to build or serve the book.

## ADDED Requirements

### Requirement: Generate SUMMARY.md
The `summary` command SHALL rewrite the region of `src/SUMMARY.md` between the lines `<!-- aa-denote-mdbook-tool:begin -->` and `<!-- aa-denote-mdbook-tool:end -->` with an entry for every note under `src/notes/`, and SHALL leave all content outside that region unchanged. If the markers are missing, it SHALL append them with the generated region.

#### Scenario: Hand edits preserved
- **WHEN** the user added `[About](about.md)` above the begin marker and runs `aa-denote-mdbook-tool summary`
- **THEN** the `[About](about.md)` line is still present and the region between the markers is regenerated

### Requirement: Chapters grouped by date
Generated entries SHALL be grouped under a part title `# YYYY-MM-DD` per identifier date, newest date first, and within a date by identifier in ascending order. Each entry's text SHALL be the note's front matter title, falling back to its identifier.

#### Scenario: Two days of notes
- **WHEN** notes exist on 2026-10-09 and 2026-10-10
- **THEN** the `# 2026-10-10` part title appears before `# 2026-10-09`
- **AND** a note at 08:00 is listed before a note at 09:00 on the same day

### Requirement: Attachments are subchapters
A note whose front matter `parent` names an existing note SHALL be listed as a nested subchapter under that parent, with AI summaries before transcripts. A note without a valid parent SHALL be listed as a top-level chapter under its own date.

#### Scenario: Meeting with summary and transcript
- **WHEN** meeting note `20261010T080000` has an attached summary and an attached transcript
- **THEN** SUMMARY.md lists the meeting note as a chapter with the summary and then the transcript indented beneath it

#### Scenario: Orphan transcript
- **WHEN** a transcript has no `parent` field, or its parent identifier no longer exists
- **THEN** it is listed as a top-level chapter under its own date

### Requirement: Front matter hidden in rendered output
When mdBook runs the `aa-denote-mdbook-tool` preprocessor, the tool SHALL remove the leading `+++` TOML front matter block from each chapter before rendering, leaving the source files unchanged.

#### Scenario: Rendered chapter
- **WHEN** the book is built
- **THEN** the HTML page for a note begins with its `# <title>` heading and shows no `title =` or `identifier =` lines
- **AND** the source Markdown file still contains its front matter

#### Scenario: Preprocessor supports renderer check
- **WHEN** mdBook invokes `aa-denote-mdbook-tool preprocess supports html`
- **THEN** the command exits zero

### Requirement: Build and serve
The `build` command SHALL regenerate SUMMARY.md and then run `mdbook build` in the notebook root. The `serve` command SHALL do the same and then run `mdbook serve`, passing through `--open` and `--port`. Both SHALL exit with mdBook's exit status.

#### Scenario: Build the book
- **WHEN** the user runs `aa-denote-mdbook-tool build`
- **THEN** SUMMARY.md is regenerated and the rendered book is written to `book/`

#### Scenario: mdbook not installed
- **WHEN** `mdbook` is not on `PATH`
- **THEN** `build` exits non-zero with an error explaining how to install mdBook, after SUMMARY.md has been regenerated
