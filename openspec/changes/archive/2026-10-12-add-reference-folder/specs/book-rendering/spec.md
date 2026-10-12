## MODIFIED Requirements

### Requirement: Generate SUMMARY.md
The `summary` command SHALL rewrite the region of `src/SUMMARY.md` between the lines `<!-- aa-denote-mdbook-tool:begin -->` and `<!-- aa-denote-mdbook-tool:end -->` with an entry for every note under `src/notes/` and every reference note under `src/reference/`, and SHALL leave all content outside that region unchanged. If the markers are missing, it SHALL append them with the generated region.

#### Scenario: Hand edits preserved
- **WHEN** the user added `[About](about.md)` above the begin marker and runs `aa-denote-mdbook-tool summary`
- **THEN** the `[About](about.md)` line is still present and the region between the markers is regenerated

## ADDED Requirements

### Requirement: Reference section
When reference notes exist, the generated region SHALL begin with the item `- [Reference](reference.md)`, before any day item, with every reference note nested beneath it, ordered by title without regard to case and then by identifier. Each entry's text SHALL be the note's front matter title, falling back to its identifier. Reference notes SHALL NOT appear under any day item or on any day page. Without reference notes, the region SHALL have no Reference item.

#### Scenario: Reference notes listed before the days
- **WHEN** reference notes "Product: Widget" and "glossary" exist alongside meetings on 2026-10-10, and the user runs `aa-denote-mdbook-tool summary`
- **THEN** the region starts with `- [Reference](reference.md)`, followed by nested entries for `glossary` and then `Product: Widget`, linking to `reference/<file name>`
- **AND** the `- [2026-10-10](days/2026-10-10.md)` item follows, and neither reference note appears under it or on `src/days/2026-10-10.md`

#### Scenario: No reference notes
- **WHEN** `src/reference/` is empty or missing and the user runs `aa-denote-mdbook-tool summary`
- **THEN** the generated region is the same as it would be without the reference kind

### Requirement: Reference page
When reference notes exist, the `summary` command SHALL write `src/reference.md` with a `# Reference` heading followed by links to every reference note, in the same order as the Reference section, relative to the page. When no reference notes exist, it SHALL remove `src/reference.md` if present.

#### Scenario: Reference page content
- **WHEN** a reference note `src/reference/20261012T090000--glossary__acme.md` titled "Glossary" exists and the user runs `aa-denote-mdbook-tool summary`
- **THEN** `src/reference.md` starts with `# Reference` and contains `- [Glossary](reference/20261012T090000--glossary__acme.md)`

#### Scenario: Last reference note removed
- **WHEN** the only reference note is deleted and the user runs `aa-denote-mdbook-tool summary`
- **THEN** `src/reference.md` is removed and `SUMMARY.md` has no Reference item

#### Scenario: Book builds with reference notes
- **WHEN** the notebook has reference notes and the user runs `aa-denote-mdbook-tool summary` and then `mdbook build`
- **THEN** the build succeeds and the rendered book shows the Reference page with its notes nested beneath it in the sidebar
