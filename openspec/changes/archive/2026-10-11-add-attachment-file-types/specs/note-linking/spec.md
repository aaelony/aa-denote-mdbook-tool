# Spec Delta

## MODIFIED Requirements

### Requirement: Generate a link
The `link <TARGET-ID>` command SHALL print a Markdown link to the target whose text is the target title and whose path is relative to the note given by `--from <ID>` (default: relative to `src/notes/`). When the target is a renderable attachment (`png`, `jpg`, `svg`), it SHALL use image syntax `![Title](path)`; otherwise it SHALL use link syntax `[Title](path)`. With `--append`, it SHALL append the link to the `--from` note instead of printing it.

#### Scenario: Print a link
- **WHEN** the user runs `aa-denote-mdbook-tool link 20261010T080000 --from 20261010T090105`, where the target is "Weekly Sync" in `src/notes/` and the source is in `src/notes/zoom-summaries/`
- **THEN** the command prints `[Weekly Sync](../20261010T080000--weekly-sync__acme_meeting.md)`

#### Scenario: Link to a renderable attachment
- **WHEN** the user runs `aa-denote-mdbook-tool link 20261010T081000` and that identifier belongs to `src/svg/20261010T081000--data-flow.svg`
- **THEN** the command prints `![data-flow](../svg/20261010T081000--data-flow.svg)`

#### Scenario: Link to a non-renderable attachment
- **WHEN** the user runs `aa-denote-mdbook-tool link 20261010T081500` and that identifier belongs to `src/pdf/20261010T081500--vendor-contract.pdf`
- **THEN** the command prints `[vendor-contract](../pdf/20261010T081500--vendor-contract.pdf)`

### Requirement: Accept Denote-style links as input
The `links fix` command SHALL rewrite any link whose target is `denote:<ID>` into a relative path to the note or attachment with that identifier, so users can type identifiers while writing. The link text and the choice of image or link syntax SHALL be left as written.

#### Scenario: Convert a denote link
- **WHEN** a note contains `[sync](denote:20261010T080000)` and the user runs `aa-denote-mdbook-tool links fix`
- **THEN** that link becomes `[sync](20261010T080000--weekly-sync__acme_meeting.md)` relative to the note

#### Scenario: Convert a denote link to an attachment
- **WHEN** a note in `src/notes/` contains `[query](denote:20261010T082000)` and that identifier belongs to `src/sql/20261010T082000--revenue.sql`
- **THEN** `links fix` rewrites it to `[query](../sql/20261010T082000--revenue.sql)`

### Requirement: Check links
The `links check` command SHALL scan every Markdown file under `src/` for relative links to `.md` files, to files with a supported attachment extension (`png`, `jpg`, `svg`, `pdf`, `sql`, `json`), and `denote:` links, report each one whose target does not exist with file and line number, and exit non-zero when any are found.

#### Scenario: Broken link reported
- **WHEN** a note links to `../20261010T080000--weekly-sync.md` but that file was renamed
- **THEN** `links check` prints the note path, line number, and broken target, marks it fixable because identifier `20261010T080000` still exists, and exits non-zero

#### Scenario: Broken attachment link reported
- **WHEN** a note links to `../pdf/20261010T081500--contract.pdf` but the file is now `../pdf/20261010T081500--vendor-contract.pdf`
- **THEN** `links check` reports it as fixable, and `links fix` rewrites it to the current path

#### Scenario: Other extensions ignored
- **WHEN** a note links to `chapter.html` or `notes.docx`
- **THEN** `links check` does not report those links

#### Scenario: All links valid
- **WHEN** every link resolves
- **THEN** `links check` prints nothing and exits zero
