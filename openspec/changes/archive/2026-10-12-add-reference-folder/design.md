# Design

## Context

See proposal.md for why. The note kind lives in the `Kind` enum (`frontmatter.rs`). Each kind maps to a folder (`Kind::folder`), and a note's kind comes from the folder it sits in (`Notebook::kind_of`), which treats anything that isn't a Zoom folder as `manual`. `summary.rs` builds the generated `SUMMARY.md` region only from notes under `src/notes/` and groups them by day. The CLI (`KindArg`, `ImportKind`) and MCP server (`KindParam`, `ImportKindParam`) each have their own kind enums that convert into `Kind`. Zoom import picks parent candidates with `kind_of(e) == Kind::Manual`.

## Goals / Non-Goals

**Goals:**
- Reference notes reuse everything that works on any Denote note: naming, front matter, `rename`, `link`, `links check/fix`, `links backlinks`, `attach --for`, `search`.
- Notebooks without reference notes keep exactly the same `SUMMARY.md` and day pages.

**Non-Goals:**
- Plain, non-Denote Markdown files in `src/reference/` are not listed in the Reference section. Users can still link them by hand above the begin marker, as today. `import --kind reference` is the way to bring an existing file in.
- No nesting or grouping inside `src/reference/` (no subfolders, no keyword sections). The flat, title-sorted list can grow into that later.
- No command to move an existing `src/notes/` note into `src/reference/`. Moving the file by hand works, since the kind comes from the folder; `links fix` repairs links afterward.

## Decisions

**A fourth `Kind` rather than a flag or a keyword.** `Kind::Reference` with folder `reference` and front matter `kind = "reference"`. The kind already decides the folder, the front matter and the `--kind` search filter, so adding a variant gets all three. Alternatives: a `reference` keyword on manual notes (no separate folder, and it mixes into day lists unless the summary code special-cases a keyword), or a `--reference` flag on `new` (a second way to say what `--kind` already says).

**Folder `src/reference/`, a sibling of `src/notes/`, not under it.** Keeping it out of `src/notes/` means `collect_items`, which walks only `src/notes/`, keeps reference notes out of the day lists with no extra filter. `kind_of` gains one branch for the reference folder before the `manual` fallback.

**Reference notes neither have nor are parents.** A parent link drives day-list nesting and the `## Attachments` section, both meeting concepts. A transcript under a glossary would render under a day with its parent missing. The checks go in `capture::create_note`, the one place `new`, `import` and the MCP tools all pass through, so the rule holds everywhere. `attach --for` is unaffected: it only decides the relative path of a file reference, not a parent.

**Generated `src/reference.md` page plus a Reference item at the top of the region.** This mirrors the day pages: a clickable parent item with its notes nested beneath. Placing it first keeps it in a stable spot, since day items are newest first and keep moving down. The page sits at `src/reference.md`, not `src/reference/README.md` or `index.md`, so the tool never writes a non-Denote file into a folder the user owns. Alternative: an mdBook part title (`# Reference`) inside the region. Rejected because a part title applies to every item after it, so the day items would also fall under "Reference" unless the tool emitted a second title, which changes the layout of books with no reference notes.

**Sorting by title, ignoring case, then by identifier.** Reference material is looked up by name, not by date. The identifier tiebreak keeps the order stable when two notes share a title.

**Kind enums at the edges just gain a variant.** `KindArg`, `ImportKind`, `KindParam` and `ImportKindParam` each add `Reference`. `import --kind reference` reuses `import_body`, so `.txt` files are fenced and Markdown gets bare URLs linked, exactly as for summaries. The parent-default logic in `create_note` matches only `Summary | Transcript`, so reference notes get no inherited title or keywords.

## Risks / Trade-offs

- [A user already has a hand-written `src/reference.md`] → `summary` would overwrite it, or delete it when there are no reference notes. This is unlikely in a notebook the tool created. The README will say the file is generated, like `src/days/`.
- [A user drops non-Denote files into `src/reference/` and expects them listed] → The README says only notes created by `new`/`import --kind reference` are listed, and shows the import command.
- [The `mcp-server` delta targets a capability whose main spec doesn't exist yet] → It contains only ADDED requirements, so archiving in either order works. Archiving after `add-mcp-server` is still cleaner.

## Migration Plan

None needed. Existing notebooks get `src/reference/` the first time a reference note is created, through the existing missing-folder rule (`kind_dir` creates it). Rolling back leaves `src/reference/` notes that an older binary treats as `manual` notes outside `src/notes/`: they are still searchable and linkable but are not listed in `SUMMARY.md`.
