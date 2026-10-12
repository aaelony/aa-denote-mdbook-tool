# Spec Delta

## ADDED Requirements

### Requirement: SUMMARY.md regenerated after every write
Every command that writes to the notebook (`new`, `import`, `attach`, `link --append`, `links fix`, and `rename`) SHALL regenerate SUMMARY.md and the day pages, as `summary` does, after its own change succeeds. With `--edit`, regeneration SHALL happen after the editor exits so an edited title is picked up. A command that fails SHALL leave SUMMARY.md unchanged.

#### Scenario: New note appears without running summary
- **WHEN** the user runs `aa-denote-mdbook-tool new --title "Acme sync"`
- **THEN** SUMMARY.md lists "Acme sync" under today's day entry
- **AND** `src/days/<today>.md` lists it too

#### Scenario: Rename updates the entry
- **WHEN** the user runs `aa-denote-mdbook-tool rename 20261011T090000 --title "Acme kickoff"`
- **THEN** SUMMARY.md links the renamed file with the title "Acme kickoff" and no longer links the old path

#### Scenario: Failed command
- **WHEN** `attach` fails because the file type is unsupported
- **THEN** SUMMARY.md is not rewritten
