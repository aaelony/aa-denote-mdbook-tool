## MODIFIED Requirements

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
