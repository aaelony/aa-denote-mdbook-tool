# Spec Delta

## Purpose

Defines how notes and images are named and described using the Denote file-naming convention, so files sort chronologically, carry their metadata in the name, and keep a stable identifier across renames.

## ADDED Requirements

### Requirement: Denote file name format
Note file names SHALL follow `IDENTIFIER[==SIGNATURE][--TITLE][__KEYWORDS].md`, where each optional part is omitted when empty. Images use the same format with the `.png` extension.

#### Scenario: Full name
- **WHEN** a note is created at 2026-10-10 08:00:00 local time with title "Weekly Sync", keywords `meeting` and `acme`, and no signature
- **THEN** the file name is `20261010T080000--weekly-sync__acme_meeting.md`

#### Scenario: Signature included
- **WHEN** the same note is created with signature `1a`
- **THEN** the file name is `20261010T080000==1a--weekly-sync__acme_meeting.md`

#### Scenario: Identifier only
- **WHEN** a note is created with no title, no keywords, and no signature
- **THEN** the file name is `20261010T080000.md`

### Requirement: Identifier format
The identifier SHALL be the note's timestamp (when its content happened, as selected by the note-capture timestamp rules) in local time formatted as `YYYYMMDDTHHMMSS`, and SHALL be the only part of the name used to identify a note.

#### Scenario: Identifier from clock
- **WHEN** a note is created at 2026-10-12 14:05:09 local time with no other timestamp source
- **THEN** its identifier is `20261012T140509`

#### Scenario: Identifier from a selected timestamp
- **WHEN** a note is imported at 12:00 but its selected timestamp is 2026-10-10 08:00:00
- **THEN** its identifier starts from `20261010T080000`, before uniqueness is applied

### Requirement: Identifier uniqueness
An identifier SHALL be unique across all notes and images in the notebook. If the computed identifier is already used, the tool SHALL add one second until it is unused.

#### Scenario: Two notes in the same second
- **WHEN** a note with identifier `20261010T080000` exists and another note is created in the same second
- **THEN** the new note receives identifier `20261010T080001`

#### Scenario: Attachments sort right after their meeting
- **WHEN** meeting `20261010T080000` exists and its summary and then its transcript are imported with `--parent 20261010T080000`
- **THEN** the summary receives `20261010T080001` and the transcript `20261010T080002`

### Requirement: Title slug rules
The title part SHALL be the title lowercased, with every run of characters other than ASCII letters and digits replaced by a single hyphen, and leading or trailing hyphens removed.

#### Scenario: Punctuation and spaces
- **WHEN** the title is `Q4 Roadmap: Acme / Beta!`
- **THEN** the title part is `q4-roadmap-acme-beta`

### Requirement: Keyword rules
Each keyword SHALL be lowercased with all characters other than ASCII letters and digits removed. Empty and duplicate keywords SHALL be dropped, and keywords SHALL be sorted alphabetically and joined with a single underscore.

#### Scenario: Normalized keywords
- **WHEN** keywords `Zoom`, `ACME-corp`, `zoom`, and `` are given
- **THEN** the keyword part is `__acmecorp_zoom`

### Requirement: Signature rules
The signature SHALL be lowercased with all characters other than ASCII letters, digits, and `=` removed, and SHALL be omitted when empty.

#### Scenario: Normalized signature
- **WHEN** the signature `1-A` is given
- **THEN** the signature part is `==1a`

### Requirement: Note front matter
Every note created by the tool SHALL begin with a TOML front matter block delimited by `+++` lines (Denote's `markdown-toml` format) holding `title`, `date` (the identifier's time as an RFC 3339 datetime with offset), `tags`, `identifier`, and `kind`, then a `# <title>` heading. With a parent, it SHALL also hold `parent` (the parent's identifier). Notes made by `import` SHALL also hold `imported`, the actual import time.

#### Scenario: Front matter of a new manual note
- **WHEN** the note `20261010T080000--weekly-sync__acme_meeting.md` is created
- **THEN** the file starts with a `+++` line, then `title = "Weekly Sync"`, `date = 2026-10-10T08:00:00` with the local offset, `tags = ["acme", "meeting"]`, `identifier = "20261010T080000"`, `kind = "manual"`, then a closing `+++` line
- **AND** the block parses as valid TOML
- **AND** the first line after the front matter block is `# Weekly Sync`

#### Scenario: Front matter of an attached transcript
- **WHEN** a transcript is attached to parent `20261010T080000`
- **THEN** its front matter contains `kind = "transcript"` and `parent = "20261010T080000"`

#### Scenario: Front matter of a late import
- **WHEN** a transcript is imported at 2026-10-10 12:00:00 local time and receives identifier `20261010T080002`
- **THEN** its front matter contains `date = 2026-10-10T08:00:02` and `imported = 2026-10-10T12:00:00`, both with the local offset

#### Scenario: Manual notes have no imported field
- **WHEN** a note is created with `new`
- **THEN** its front matter has no `imported` field

### Requirement: Rename keeps identifier
The `rename <ID>` command SHALL change a note's title, keywords, or signature by renaming the file and updating its front matter and first heading, while keeping its identifier and folder unchanged, then repair inbound links.

#### Scenario: Change title
- **WHEN** the user runs `aa-denote-mdbook-tool rename 20261010T080000 --title "Acme Weekly"`
- **THEN** the file becomes `20261010T080000--acme-weekly__acme_meeting.md`
- **AND** its front matter `title` and first heading read `Acme Weekly`
- **AND** every other note that linked to the old path now links to the new path

#### Scenario: Unknown identifier
- **WHEN** the user runs `rename` with an identifier that no note has
- **THEN** the command exits non-zero and no file changes
