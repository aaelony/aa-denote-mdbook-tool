# Proposal

## Why

Meeting notes reference more than screenshots: contracts arrive as PDFs, queries as `.sql`, configs and API payloads as `.json`, diagrams as `.svg`, and phone photos as `.jpg`. Today only `.png` files can be added, named, linked, checked, and repaired, so every other file is either left outside the notebook or linked by hand with no protection against renames. The original design already listed this as an open question ("Should image formats beyond `png` be accepted later?").

## What Changes

- Supported attachment extensions become exactly `png`, `jpg`, `svg`, `pdf`, `sql`, and `json`. Of these, `png`, `jpg`, and `svg` are renderable images; `pdf`, `sql`, and `json` are plain files.
- Each extension lives in its own folder under `src/` named after the extension (`src/png/`, `src/jpg/`, `src/svg/`, `src/pdf/`, `src/sql/`, `src/json/`), created on demand.
- New `attach <FILE>` command copies a supported file into its folder with a Denote name and prints a reference: image syntax `![Title](path)` for renderable files, link syntax `[Title](path)` for the others. `image` remains as an alias of `attach`, so existing usage keeps working.
- `link <ID>` uses the same rule: image syntax when the target is a renderable attachment, link syntax otherwise.
- Denote names, identifier lookup, `links check`, `links fix`, `denote:` conversion, and `links backlinks` recognize every supported extension, not only `.md` and `.png`.
- A file with an unsupported extension (for example `.jpeg`, `.docx`) is rejected with an error listing the supported extensions. Extensions are matched case-insensitively and stored lowercase (`Photo.JPG` becomes `...--photo.jpg`).
- **BREAKING (library only)**: the core crate's `add_image` and `png_dir` are replaced by attachment-generic functions. The CLI stays compatible through the `image` alias.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `notebook-layout`: the folder rule changes from a single `src/png/` to one folder per supported extension, and on-demand recreation covers every attachment folder.
- `note-naming`: Denote file names may carry any supported attachment extension, not only `.md` and `.png`.
- `note-capture`: "Add a png image" is replaced by attaching any supported file, with image or link syntax chosen by renderability; timestamp selection names `attach`.
- `note-linking`: `link` chooses image syntax for renderable targets; `links check`/`fix` and `denote:` conversion cover every supported extension.

Note: these capabilities are currently defined by the unarchived change `add-denote-mdbook-cli`. That change must be archived (creating `openspec/specs/`) before this one, so the MODIFIED requirements here have a base to apply to.

## Impact

- Core crate: `naming.rs` (supported extensions, renderable check, parse), `notebook.rs` (per-extension folder, scan), `capture.rs` (attach replaces add_image), `links.rs` (local-target detection, link syntax).
- CLI: `src/cli.rs` gains `attach` with `image` as alias; help text updated.
- Tests: new integration tests for each extension class, case folding, and rejection; existing image tests keep passing through the alias.
- Docs: README layout tree, commands table, and workflow example.
- No new dependencies. Rendering needs no preprocessor change because mdBook already copies non-Markdown files from `src/` into the built book.
