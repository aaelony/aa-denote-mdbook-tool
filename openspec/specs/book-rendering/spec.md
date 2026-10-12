# book-rendering Specification

## Purpose

Turns the notebook into a readable mdBook: generates `SUMMARY.md` from the notes on disk, nests AI summaries and transcripts under the meeting they belong to, hides front matter in the rendered pages, and runs `mdbook` to build or serve the book.

## Requirements

### Requirement: Generate SUMMARY.md
The `summary` command SHALL rewrite the region of `src/SUMMARY.md` between the lines `<!-- aa-denote-mdbook-tool:begin -->` and `<!-- aa-denote-mdbook-tool:end -->` with an entry for every note under `src/notes/`, and SHALL leave all content outside that region unchanged. If the markers are missing, it SHALL append them with the generated region.

#### Scenario: Hand edits preserved
- **WHEN** the user added `[About](about.md)` above the begin marker and runs `aa-denote-mdbook-tool summary`
- **THEN** the `[About](about.md)` line is still present and the region between the markers is regenerated

### Requirement: Chapters grouped by date
Generated entries SHALL be grouped under a list item per identifier date, written as a link `- [YYYY-MM-DD](days/YYYY-MM-DD.md)`, newest date first, with that date's notes nested beneath it in ascending identifier order. Each note entry's text SHALL be the note's front matter title, falling back to its identifier.

#### Scenario: Two days of notes
- **WHEN** notes exist on 2026-10-09 and 2026-10-10
- **THEN** the `- [2026-10-10](days/2026-10-10.md)` item appears before `- [2026-10-09](days/2026-10-09.md)`
- **AND** a note at 08:00 is listed before a note at 09:00 beneath the same day

### Requirement: Day pages
The `summary` command SHALL write `src/days/YYYY-MM-DD.md` for every date that has notes, with a `# YYYY-MM-DD` heading followed by the same nested links to that date's notes, relative to the page. It SHALL delete `src/days/YYYY-MM-DD.md` pages for dates that no longer have notes and SHALL leave other files in `src/days/` unchanged.

#### Scenario: Day page content
- **WHEN** a meeting on 2026-10-10 has an attached summary and the user runs `aa-denote-mdbook-tool summary`
- **THEN** `src/days/2026-10-10.md` starts with `# 2026-10-10` and links to the meeting, with the summary nested beneath it, using `../notes/...` paths

#### Scenario: Stale day page removed
- **WHEN** the only note dated 2026-10-09 is deleted and the user runs `aa-denote-mdbook-tool summary`
- **THEN** `src/days/2026-10-09.md` is removed and any non-date file in `src/days/` remains

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
