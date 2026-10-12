## MODIFIED Requirements

### Requirement: Create a new note
The `new` command SHALL create a note of kind `manual` by default, accepting `--title`, `--keywords` (comma separated), `--signature`, `--kind <manual|summary|transcript|reference>`, `--parent <ID>`, and `--date <WHEN>`, and SHALL print the path of the created file relative to the notebook root.

#### Scenario: Quick manual note
- **WHEN** the user runs `aa-denote-mdbook-tool new --title "Weekly Sync" --keywords meeting,acme` at 2026-10-10 08:00:00
- **THEN** `src/notes/20261010T080000--weekly-sync__acme_meeting.md` is created with front matter and a `# Weekly Sync` heading
- **AND** the command prints `src/notes/20261010T080000--weekly-sync__acme_meeting.md`

#### Scenario: Note with no title
- **WHEN** the user runs `aa-denote-mdbook-tool new` with no options
- **THEN** a manual note named only by its identifier is created and its heading is the identifier

#### Scenario: Reference note
- **WHEN** the user runs `aa-denote-mdbook-tool new --kind reference --title "Glossary" --keywords acme` at 2026-10-12 09:00:00
- **THEN** `src/reference/20261012T090000--glossary__acme.md` is created with `kind = "reference"` in its front matter
- **AND** the command prints `src/reference/20261012T090000--glossary__acme.md`

### Requirement: Import a Zoom summary or transcript
The `import <FILE> --kind <summary|transcript|reference>` command SHALL copy the contents of `FILE` (or standard input when `FILE` is `-`) into a new note of that kind with Denote naming and front matter, accepting the same `--title`, `--keywords`, `--signature`, `--parent`, and `--date` options as `new`. The source file SHALL NOT be modified or removed.

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

#### Scenario: Import an existing glossary as a reference note
- **WHEN** the user runs `aa-denote-mdbook-tool import ~/docs/glossary.md --kind reference --title "Glossary"`
- **THEN** a note is created in `src/reference/` with `kind = "reference"` whose body after the heading is the content of `glossary.md`
- **AND** `~/docs/glossary.md` is unchanged

#### Scenario: Non-UTF-8 input
- **WHEN** the input is not valid UTF-8
- **THEN** the command exits non-zero and no note is created

## ADDED Requirements

### Requirement: Reference notes stand alone
A reference note SHALL have no parent and SHALL NOT be a parent. `new` and `import` with `--kind reference` SHALL reject `--parent`, and `new` and `import` of any kind SHALL reject a `--parent` that names a reference note. In both cases the command SHALL exit non-zero, name the conflict, and create no note.

#### Scenario: Parent given for a reference note
- **WHEN** the user runs `aa-denote-mdbook-tool new --kind reference --title "Glossary" --parent 20261010T080000`
- **THEN** the command exits non-zero with an error saying reference notes cannot have a parent
- **AND** no note is created and the note `20261010T080000` is unchanged

#### Scenario: Reference note named as a parent
- **WHEN** `20261012T090000` is a reference note and the user runs `aa-denote-mdbook-tool import zoom.vtt --kind transcript --parent 20261012T090000`
- **THEN** the command exits non-zero with an error saying a reference note cannot be a parent
- **AND** no note is created and the reference note is unchanged

#### Scenario: Linking to a reference note is unaffected
- **WHEN** a meeting note links to reference note `20261012T090000` with `aa-denote-mdbook-tool link 20261012T090000 --from 20261010T080000 --append`
- **THEN** the link is added as for any other note, and `links backlinks 20261012T090000` lists the meeting note
