## ADDED Requirements

### Requirement: Reference kind in tools
The `kind` parameter of `new_note`, `import_note`, `search_notes` and `list_notes` SHALL accept `reference`, with the same folder, `SUMMARY.md` and parent rules as the CLI. `list_notes` SHALL report reference notes with kind `reference`.

#### Scenario: Create a reference note through MCP
- **WHEN** the client calls `new_note` with title "Glossary" and kind `reference`
- **THEN** the note is created in `src/reference/`, the result holds its path, and `SUMMARY.md` lists it under the Reference item

#### Scenario: List only reference notes
- **WHEN** the client calls `list_notes` with kind `reference`
- **THEN** only notes in `src/reference/` are returned, each with kind `reference`

#### Scenario: Parent rejected for a reference note
- **WHEN** the client calls `new_note` with kind `reference` and a `parent`
- **THEN** the call returns a tool error saying reference notes cannot have a parent, and no note is created
