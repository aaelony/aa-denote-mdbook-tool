# note-search Specification

## Purpose

Makes the notebook easy to search with `ripgrep`, both directly and through a `search` command that filters by keyword and note kind, so it also works well as a plain-text knowledge base for LLM tools.

## Requirements

### Requirement: Plain-text, greppable notes
All notes SHALL be UTF-8 Markdown with one front matter field per line, and keywords SHALL appear both in the file name and in the `tags` field, so `rg` can match on content, file names, or metadata without extra tooling.

#### Scenario: Find notes by tag with plain ripgrep
- **WHEN** the user runs `rg -l 'tags = .*"acme"' src/notes` in a notebook
- **THEN** every note with keyword `acme` is listed

#### Scenario: Find notes by file name
- **WHEN** the user runs `rg --files src | rg '__.*meeting'`
- **THEN** every note whose keywords include `meeting` is listed

### Requirement: Rendered output excluded from search
The notebook's `.ignore` file SHALL exclude the `book/` directory so that `rg` run from the notebook root does not return duplicate matches from rendered HTML.

#### Scenario: No duplicate matches
- **WHEN** the book has been built and the user runs `rg "roadmap"` from the notebook root
- **THEN** no match from `book/` is reported

### Requirement: Search command
The `search <PATTERN>` command SHALL run `rg` over `src/` with `PATTERN`, restricted by `--keyword <K>` (repeatable; all must match the note's keywords) and `--kind <manual|summary|transcript|reference>`. Extra arguments after `--` SHALL pass to `rg`, and the command SHALL exit with `rg`'s status.

#### Scenario: Search only transcripts tagged acme
- **WHEN** the user runs `aa-denote-mdbook-tool search "budget" --keyword acme --kind transcript`
- **THEN** only matches in `src/notes/zoom-transcripts/` files whose keywords include `acme` are shown

#### Scenario: Search only reference notes
- **WHEN** the user runs `aa-denote-mdbook-tool search "SLA" --kind reference`
- **THEN** only matches in `src/reference/` notes are shown, and matches in meeting notes are not

#### Scenario: Pass-through options
- **WHEN** the user runs `aa-denote-mdbook-tool search "budget" -- -i -C 2`
- **THEN** `rg` is invoked with `-i -C 2` in addition to the pattern

#### Scenario: No matching files
- **WHEN** no note satisfies the keyword and kind filters
- **THEN** the command prints nothing and exits with status 1, as `rg` does for no matches

#### Scenario: ripgrep not installed
- **WHEN** `rg` is not on `PATH`
- **THEN** the command exits non-zero with an error explaining that ripgrep is required
