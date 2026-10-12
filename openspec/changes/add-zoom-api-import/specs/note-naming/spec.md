# Spec Delta

## MODIFIED Requirements

### Requirement: Note front matter
Every note created by the tool SHALL begin with a TOML front matter block delimited by `+++` lines (Denote's `markdown-toml` format) holding `title`, `date` (the identifier's time as an RFC 3339 datetime with offset), `tags`, `identifier`, and `kind`, then a `# <title>` heading. With a parent, it SHALL also hold `parent` (the parent's identifier). Notes made by `import` or `zoom import` SHALL also hold `imported`, the actual import time. Notes made by `zoom import` SHALL also hold `zoom_meeting`, the Zoom meeting UUID.

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

#### Scenario: Notes imported from Zoom
- **WHEN** a summary is imported by `zoom import` from meeting UUID `aDYlohsHRtCd4ii1uC2+hA==`
- **THEN** its front matter contains `zoom_meeting = "aDYlohsHRtCd4ii1uC2+hA=="`
- **AND** notes made by `new` or `import` have no `zoom_meeting` field
