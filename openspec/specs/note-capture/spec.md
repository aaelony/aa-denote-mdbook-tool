# note-capture Specification

## Purpose

Lets the user capture notes quickly during meetings: create manual notes, bring in Zoom AI summaries and transcripts attached to the meeting they belong to, and add `png` images, all named and placed automatically.

## Requirements

### Requirement: Create a new note
The `new` command SHALL create a note of kind `manual` by default, accepting `--title`, `--keywords` (comma separated), `--signature`, `--kind <manual|summary|transcript>`, `--parent <ID>`, and `--date <WHEN>`, and SHALL print the path of the created file relative to the notebook root.

#### Scenario: Quick manual note
- **WHEN** the user runs `aa-denote-mdbook-tool new --title "Weekly Sync" --keywords meeting,acme` at 2026-10-10 08:00:00
- **THEN** `src/notes/20261010T080000--weekly-sync__acme_meeting.md` is created with front matter and a `# Weekly Sync` heading
- **AND** the command prints `src/notes/20261010T080000--weekly-sync__acme_meeting.md`

#### Scenario: Note with no title
- **WHEN** the user runs `aa-denote-mdbook-tool new` with no options
- **THEN** a manual note named only by its identifier is created and its heading is the identifier

### Requirement: Note timestamp selection
`new`, `import`, and `attach` SHALL timestamp a note by when its content happened, using the first available of: `--date`; for `new` and `import`, the `--parent` note's timestamp; for `import` of a file, a Zoom `GMTYYYYMMDD-HHMMSS` timestamp in the file name (UTC, converted to local time); otherwise the current local time.

#### Scenario: Late import attached to a meeting
- **WHEN** at 12:00 the user runs `aa-denote-mdbook-tool import summary.md --kind summary --parent 20261010T080000`
- **THEN** the summary's identifier is `20261010T080001`, not a 12:00 identifier

#### Scenario: Explicit date wins over parent
- **WHEN** the user runs `aa-denote-mdbook-tool import t.vtt --kind transcript --parent 20261010T080000 --date "2026-10-10 08:30"`
- **THEN** the transcript's identifier is `20261010T083000`

#### Scenario: Zoom file name timestamp
- **WHEN** local time is UTC-7 and the user imports `GMT20261010-150012_Recording.transcript.vtt` without `--date` or `--parent`
- **THEN** the transcript's identifier is `20261010T080012`

#### Scenario: Parent wins over Zoom file name
- **WHEN** the user imports `GMT20261010-150012_Recording.transcript.vtt` with `--parent 20261010T080000`
- **THEN** the identifier is derived from `20261010T080000`, not from the file name

#### Scenario: Import the next day
- **WHEN** on 2026-10-11 the user imports a transcript with `--parent 20261010T100000`
- **THEN** the transcript's identifier is on 2026-10-10 and it is grouped under that date in the book

#### Scenario: Standard input falls back to the clock
- **WHEN** the user pipes text into `import - --kind summary` with no `--date` and no `--parent` at 12:00:00
- **THEN** the note's identifier is today's date at `T120000`

### Requirement: Date option format
`--date` SHALL accept local times as `YYYY-MM-DD HH:MM[:SS]`, `YYYY-MM-DDTHH:MM[:SS]`, `YYYY-MM-DD` (midnight), `HH:MM[:SS]` (today), or a Denote identifier `YYYYMMDDTHHMMSS`. Any other value SHALL make the command exit non-zero without creating a note.

#### Scenario: Time only means today
- **WHEN** on 2026-10-10 the user runs `aa-denote-mdbook-tool new --title "Standup" --date 09:00`
- **THEN** the note's identifier is `20261010T090000`

#### Scenario: Invalid date
- **WHEN** the user passes `--date "next tuesday"`
- **THEN** the command exits non-zero with an error listing the accepted formats and no note is created

### Requirement: Open in editor
When `--edit` is given, `new` and `import` SHALL open the created file in the program named by `$VISUAL`, else `$EDITOR`, and SHALL fail with a clear error if neither is set; the file SHALL still exist on failure.

#### Scenario: Edit after create
- **WHEN** `EDITOR=emacsclient` is set and the user runs `aa-denote-mdbook-tool new --title "1:1" --edit`
- **THEN** the note is created and `emacsclient <path>` is launched

### Requirement: Import a Zoom summary or transcript
The `import <FILE> --kind <summary|transcript>` command SHALL copy the contents of `FILE` (or standard input when `FILE` is `-`) into a new note of that kind with Denote naming and front matter, accepting the same `--title`, `--keywords`, `--signature`, `--parent`, and `--date` options as `new`. The source file SHALL NOT be modified or removed.

#### Scenario: Import a Markdown summary
- **WHEN** the user runs `aa-denote-mdbook-tool import ~/Downloads/summary.md --kind summary --title "Weekly Sync Summary"`
- **THEN** a note is created in `src/notes/zoom-summaries/` whose body after the heading is the content of `summary.md`
- **AND** `~/Downloads/summary.md` is unchanged

#### Scenario: Import a plain text or VTT transcript
- **WHEN** the imported file has a `.txt` or `.vtt` extension
- **THEN** its content is placed in the body inside a fenced code block tagged `text`, so it renders verbatim

#### Scenario: Import from standard input
- **WHEN** the user pipes text into `aa-denote-mdbook-tool import - --kind summary`
- **THEN** the piped text becomes the body of a new summary note

#### Scenario: Non-UTF-8 input
- **WHEN** the input is not valid UTF-8
- **THEN** the command exits non-zero and no note is created

### Requirement: Attach to a parent note
When `--parent <ID>` is given, the new or imported note SHALL record the parent identifier in its front matter, and the tool SHALL append a relative link to the new note in the parent note under a `## Attachments` heading, creating that heading if absent.

#### Scenario: Attach transcript to meeting note
- **WHEN** the user runs `aa-denote-mdbook-tool import zoom.vtt --kind transcript --parent 20261010T080000`
- **THEN** the transcript's front matter contains `parent = "20261010T080000"`
- **AND** its identifier is the first unused second after `20261010T080000`
- **AND** the parent note `src/notes/20261010T080000--weekly-sync__acme_meeting.md` gains a line under `## Attachments` linking to `zoom-transcripts/<new file name>`

#### Scenario: Unknown parent
- **WHEN** `--parent` names an identifier that no note has
- **THEN** the command exits non-zero and no note is created

### Requirement: Defaults inherited from the parent
When `--parent` is given and `--title` or `--keywords` is omitted, the title SHALL default to the parent title followed by ` AI Summary` or ` Transcript` per kind, and keywords SHALL default to the parent keywords plus `zoomsummary` or `zoomtranscript`.

#### Scenario: Defaults for an attached summary
- **WHEN** the parent is titled "Weekly Sync" with keywords `acme`, `meeting`, and the user imports a summary with only `--parent`
- **THEN** the new note's title is `Weekly Sync AI Summary` and its keywords are `acme`, `meeting`, `zoomsummary`

### Requirement: Attach a file
The `attach <FILE>` command SHALL copy a supported file into its extension folder with a Denote name built from `--title` (default: the source file stem), `--keywords`, and `--date`, and SHALL print a reference relative to `src/notes/`, or to the note given by `--for <ID>`. `image` SHALL be accepted as an alias of `attach`.

#### Scenario: Image for a summary note
- **WHEN** the user runs `aa-denote-mdbook-tool attach ~/shot.png --title "Arch Diagram" --for 20261010T090105` and that note is in `src/notes/zoom-summaries/`
- **THEN** `src/png/<ID>--arch-diagram.png` is created
- **AND** the command prints `![Arch Diagram](../../png/<ID>--arch-diagram.png)`

#### Scenario: Image alias
- **WHEN** the user runs `aa-denote-mdbook-tool image ~/shot.png --title "Arch Diagram"`
- **THEN** the result is the same as running `attach` with the same arguments

#### Scenario: Extension stored lowercase
- **WHEN** the user attaches `~/Photo.JPG` with no `--title`
- **THEN** `src/jpg/<ID>--photo.jpg` is created

### Requirement: Reference syntax follows renderability
The reference printed by `attach` SHALL use Markdown image syntax `![Title](path)` when the extension is renderable (`png`, `jpg`, `svg`) and Markdown link syntax `[Title](path)` otherwise (`pdf`, `sql`, `json`).

#### Scenario: Renderable svg
- **WHEN** the user attaches `flow.svg` with `--title "Data Flow"`
- **THEN** the command prints `![Data Flow](../svg/<ID>--data-flow.svg)`

#### Scenario: Non-renderable pdf
- **WHEN** the user attaches `contract.pdf` with `--title "Vendor Contract"`
- **THEN** the command prints `[Vendor Contract](../pdf/<ID>--vendor-contract.pdf)`

### Requirement: Unsupported attachment rejected
`attach` SHALL exit non-zero without copying anything when the file's extension is not one of `png`, `jpg`, `svg`, `pdf`, `sql`, or `json` (compared case-insensitively), and the error SHALL list the supported extensions.

#### Scenario: Unsupported extension
- **WHEN** the user runs `aa-denote-mdbook-tool attach notes.docx`
- **THEN** the command exits non-zero, the error names `png, jpg, svg, pdf, sql, json`, and no file is created under `src/`

#### Scenario: Missing file
- **WHEN** the user attaches `missing.pdf` and that file does not exist
- **THEN** the command exits non-zero and no file is created under `src/`
