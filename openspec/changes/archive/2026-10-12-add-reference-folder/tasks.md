# Tasks

All new tests start with `test_`, and any new test helper starts with `test_helper_`.

## 1. Reference kind and folder

- [x] 1.1 Add `Kind::Reference` (`as_str` "reference", `folder` "reference", an `attachment_rank` value) and a `kind_of` branch for `src/reference/` before the `manual` fallback; verify with unit tests that a note in `src/reference/` has kind `reference` and notes in `src/notes/` are unchanged
- [x] 1.2 Make `init` create `src/reference/`; verify `tests/layout.rs` checks the folder exists after `init` and that `mdbook build` still succeeds
- [x] 1.3 Make Zoom import's parent candidates exclude reference notes (confirm `manual_note_ids` only returns `manual` kind once `kind_of` knows the folder); verify with a `zoom.rs` unit or `tests/zoom.rs` test that a reference note at the meeting time is not chosen as parent

## 2. Creating and importing reference notes

- [x] 2.1 Add `Reference` to the CLI `KindArg` and `ImportKind`; verify in `tests/capture.rs` that `new --kind reference --title "Glossary" --keywords acme` creates `src/reference/<id>--glossary__acme.md` with `kind = "reference"` and prints that path, and that `import FILE --kind reference` copies the body and leaves the source unchanged
- [x] 2.2 In `capture::create_note`, reject `--parent` with kind `reference` and reject a parent that is a reference note, before anything is written; verify in `tests/capture.rs` that both cases exit non-zero with the spec's messages, create no note and leave the other note unchanged
- [x] 2.3 Verify in `tests/linking.rs` that `link <reference id> --from <meeting id> --append` works and `links backlinks <reference id>` lists the meeting note
- [x] 2.4 Verify that a reference note created in a notebook without `src/reference/` recreates the folder (`tests/capture.rs`)

## 3. SUMMARY.md Reference section and page

- [x] 3.1 In `summary.rs`, collect reference notes from `src/reference/`, sort them by lowercase title then identifier, and render `- [Reference](reference.md)` with nested entries at the start of the region only when any exist; verify with unit tests for order, title fallback to identifier, and an unchanged region when there are none
- [x] 3.2 Write `src/reference.md` (`# Reference` plus links relative to `src/`) when reference notes exist and remove it when none do; verify in `tests/rendering.rs` the page content, its removal after the last reference note is deleted, and that reference notes never appear on a day page
- [x] 3.3 Verify in `tests/rendering.rs` that `new --kind reference` regenerates `SUMMARY.md` with the Reference item and that `mdbook build` succeeds (requires `mdbook`, like the existing build test)

## 4. Search and MCP

- [x] 4.1 Verify in `tests/search.rs` that `search PATTERN --kind reference` matches only `src/reference/` notes
- [x] 4.2 Add `Reference` to the MCP `KindParam` and `ImportKindParam`; verify in `tests/mcp.rs` that `new_note` with kind `reference` creates the note and updates `SUMMARY.md`, `list_notes` with kind `reference` returns only reference notes, and `new_note` with kind `reference` and a `parent` returns a tool error

## 5. Documentation and checks

- [x] 5.1 Update the README: `reference/` and the generated `reference.md` in the layout tree, `reference` in the `--kind` lists, and a short "Reference notes" section (glossaries and product notes, `new`/`import --kind reference`, no parents, the Reference item in `SUMMARY.md`, only Denote-named files are listed); verify the commands shown run as written
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `just crap` and `openspec validate add-reference-folder`; verify all pass and no function scores above 30

## Workflow follow-up

- Archive `add-mcp-server` before this change if possible, since this change adds a requirement to the `mcp-server` capability that `add-mcp-server` introduces.
- Archive with `/opsx:archive` after review.
