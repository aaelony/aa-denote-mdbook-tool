# Spec Delta

## MODIFIED Requirements

### Requirement: Denote file name format
Note file names SHALL follow `IDENTIFIER[==SIGNATURE][--TITLE][__KEYWORDS].md`, where each optional part is omitted when empty. Attachments use the same format with their lowercase extension, which SHALL be one of `png`, `jpg`, `svg`, `pdf`, `sql`, or `json`. Files with any other extension SHALL NOT be treated as Denote notes or attachments.

#### Scenario: Full name
- **WHEN** a note is created at 2026-10-10 08:00:00 local time with title "Weekly Sync", keywords `meeting` and `acme`, and no signature
- **THEN** the file name is `20261010T080000--weekly-sync__acme_meeting.md`

#### Scenario: Signature included
- **WHEN** the same note is created with signature `1a`
- **THEN** the file name is `20261010T080000==1a--weekly-sync__acme_meeting.md`

#### Scenario: Identifier only
- **WHEN** a note is created with no title, no keywords, and no signature
- **THEN** the file name is `20261010T080000.md`

#### Scenario: Attachment name
- **WHEN** a PDF is attached at 2026-10-10 08:15:00 local time with title "Vendor Contract" and keyword `acme`
- **THEN** the file name is `20261010T081500--vendor-contract__acme.pdf`

#### Scenario: Unsupported extension ignored
- **WHEN** `src/notes/` contains `20261010T090000--draft.docx`
- **THEN** that file is not listed by identifier lookup and its identifier is not considered taken
