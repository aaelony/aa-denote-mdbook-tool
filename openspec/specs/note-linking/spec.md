# note-linking Specification

## Purpose

Keeps references between notes and images working the way Denote does, by resolving targets through their stable identifiers, while storing every link as an ordinary relative Markdown path that works in mdBook, on GitHub, and in editors.

## Requirements

### Requirement: Links are relative Markdown paths
Links written by the tool SHALL be standard Markdown links whose target is a path relative to the linking file. The tool SHALL NOT write absolute paths or custom URL schemes into notes.

#### Scenario: Link from a summary to its meeting note
- **WHEN** a link is written from `src/notes/zoom-summaries/A.md` to `src/notes/B.md`
- **THEN** the link target is `../B.md`

### Requirement: Resolve notes by identifier
Any command argument that names a note SHALL accept its identifier, and the tool SHALL find the file with that identifier anywhere under `src/`, regardless of its current title, keywords, or folder.

#### Scenario: Resolve after rename
- **WHEN** note `20261010T080000` was renamed from `...--weekly-sync.md` to `...--acme-weekly.md`
- **THEN** commands given `20261010T080000` act on `...--acme-weekly.md`

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

### Requirement: Repair links by identifier
The `links fix` command SHALL rewrite each broken relative link whose file name contains an identifier that exists in the notebook so it points to that file's current relative path. Links that cannot be resolved SHALL be left unchanged and reported.

#### Scenario: Repair after move
- **WHEN** a link points to an old path of note `20261010T080000` and the note now lives at a different path
- **THEN** `links fix` rewrites the link to the current relative path and leaves the link text unchanged

#### Scenario: Unresolvable link left alone
- **WHEN** a broken link's file name has no identifier, or its identifier matches no file
- **THEN** the link is unchanged and is listed in the output

### Requirement: Backlinks
The `links backlinks <ID>` command SHALL list every note that links to the given note, showing each linking note's relative path and title.

#### Scenario: List backlinks
- **WHEN** two notes link to `20261010T080000`
- **THEN** `links backlinks 20261010T080000` prints both notes, one per line

### Requirement: Bare URLs become links
`links fix` and `import` of Markdown SHALL rewrite each bare `http://` or `https://` URL in note prose as `[url](url)`, excluding trailing sentence punctuation and an unbalanced closing parenthesis. URLs in front matter, code spans, code blocks (including verbatim `.txt`/`.vtt` imports), existing links, autolinks, and raw HTML SHALL be left unchanged. `links fix` SHALL print each linked URL with its file and line.

#### Scenario: URL in a note
- **WHEN** a note contains `Deck: https://example.com/deck.` and the user runs `aa-denote-mdbook-tool links fix`
- **THEN** the line becomes `Deck: [https://example.com/deck](https://example.com/deck).`
- **AND** a second `links fix` changes nothing

#### Scenario: URL in code left alone
- **WHEN** a URL appears inside a fenced code block or an imported `.vtt` transcript
- **THEN** it is not rewritten
