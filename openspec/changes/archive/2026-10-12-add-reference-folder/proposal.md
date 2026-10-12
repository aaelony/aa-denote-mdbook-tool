# Proposal

## Why

Some notes are not about a meeting: glossaries, definitions of terms, product notes. Today every note lands in `src/notes/` and shows up in the per-day lists of `SUMMARY.md`, which buries long-lived reference material among dated meetings. A dedicated reference folder keeps that material in one place, out of the day lists, while still giving it identifiers so meeting notes can link to it and `backlinks` can show which meetings cite it.

## What Changes

- New note kind `reference`, stored in `src/reference/` with the usual Denote file names and front matter (`kind = "reference"`).
- `init` creates `src/reference/`; any command that needs it recreates it when missing.
- `new --kind reference` creates a reference note, and `import --kind reference FILE` imports an existing Markdown file (for example a glossary) as one.
- Reference notes stand alone: `--parent` is rejected with `--kind reference`, and a reference note cannot be named as another note's `--parent`.
- The generated region of `SUMMARY.md` starts with a `Reference` entry linking to a generated `src/reference.md` page, with every reference note nested beneath it in title order. Reference notes never appear in the day lists or day pages.
- `search --kind reference` restricts a search to reference notes.
- The MCP tools `new_note`, `import_note`, `search_notes` and `list_notes` accept the `reference` kind.
- Zoom import never picks a reference note as a meeting's parent (it already only considers `manual` notes; this keeps reference notes out of that set).
- README: document the folder, the kind, and the `Reference` section.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `notebook-layout`: `init` creates `src/reference/`; the `reference` kind maps to `src/reference/`.
- `note-capture`: `new` and `import` accept `--kind reference`; reference notes cannot have or be a parent.
- `book-rendering`: the generated `SUMMARY.md` region gains a `Reference` section and a generated `src/reference.md` page.
- `note-search`: `--kind` accepts `reference`.
- `mcp-server`: the tools' `kind` parameters accept `reference`. This capability is introduced by the pending `add-mcp-server` change; this change only adds a requirement to it.

## Impact

- Code: `frontmatter.rs` (`Kind::Reference`), `notebook.rs` (`kind_of`, `init`), `capture.rs` (parent rules), `summary.rs` (Reference section and page), `src/cli.rs` (`KindArg`, `ImportKind`), `src/mcp.rs` (`KindParam`, `ImportKindParam`).
- Existing notebooks: no migration. Notes already in `src/notes/` stay where they are; `src/reference/` is created on first use.
- `SUMMARY.md`: the generated region changes only when reference notes exist, so books without them render exactly as before.
- No new dependencies.
