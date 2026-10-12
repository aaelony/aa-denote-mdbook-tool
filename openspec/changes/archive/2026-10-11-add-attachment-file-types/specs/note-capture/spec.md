# Spec Delta

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Add a png image
**Reason**: Replaced by "Attach a file", which accepts every supported attachment extension instead of only `png`.
**Migration**: `image <FILE.png>` keeps working unchanged as an alias of `attach`; library callers switch from the png-only function to the attachment function.

## ADDED Requirements

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
