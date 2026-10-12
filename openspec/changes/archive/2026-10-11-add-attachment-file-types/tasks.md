# Tasks

## 1. Supported extensions and naming

- [x] 1.1 Add `ATTACHMENT_EXTENSIONS` (`png`, `jpg`, `svg`, `pdf`, `sql`, `json`), `is_attachment_ext`, and `is_renderable_ext` (`png`, `jpg`, `svg`) to `naming.rs`; verify `test_`-prefixed unit tests for each extension, case-insensitive matching, and rejection of `jpeg`, `docx`, `html`
- [x] 1.2 Make `DenoteName::parse` accept `md` or any lowercase attachment extension; verify unit tests that `...--vendor-contract__acme.pdf` round-trips, `...--draft.docx` and `...--x.PDF` are rejected, and existing `.md`/`.png` tests still pass

## 2. Notebook folders

- [x] 2.1 Replace `Notebook::png_dir` with `attachment_dir(ext)` returning `src/<ext>/` (created if missing) and switch `init` to `attachment_dir("png")`; verify the existing `init` layout test and the deleted-png-folder test still pass, plus a new integration test that the first `.json` attachment creates `src/json/`
- [x] 2.2 Change the `find_note` refusal in `notebook.rs` from "is an image, not a note" to "is an attachment, not a note" and update "note or image" doc comments and errors to "note or attachment"; verify the rename-rejection test asserts the new message and an integration test that a `.docx` with a Denote-shaped name does not block identifier allocation

## 3. Attach command

- [x] 3.1 Add a shared `markdown_reference(text, path, ext)` helper that returns `![..](..)` for renderable extensions and `[..](..)` otherwise; verify unit tests for `svg` and `pdf`
- [x] 3.2 Replace `capture::add_image` with `add_attachment`: accept any supported extension case-insensitively, write the lowercase extension into `attachment_dir(ext)`, print the reference via 3.1, and on an unsupported extension fail with an error listing `png, jpg, svg, pdf, sql, json` before copying; verify integration tests for the svg image scenario, the pdf link scenario, `Photo.JPG` becoming `--photo.jpg` in `src/jpg/`, `.docx` rejection leaving `src/` unchanged, and a missing file
- [x] 3.3 In `src/cli.rs`, rename the `Image` subcommand to `Attach` with `visible_alias = "image"` and help text "Copy a file into src/<ext>/ and print a Markdown reference"; verify the existing `image` tests pass unchanged through the alias and `--help` lists `attach [alias: image]`

## 4. Linking

- [x] 4.1 Make `links::is_local_target` treat `.md` plus every supported attachment extension (case-insensitive, after stripping `#fragment`) as local; verify unit tests that `../pdf/x.pdf`, `../sql/q.sql`, `Report.PDF` are extracted and `chapter.html`, `notes.docx` are not
- [x] 4.2 Make `make_link` use `markdown_reference` so renderable targets print `![..](..)`; verify integration tests for the svg and pdf `link` scenarios in `specs/note-linking/spec.md`
- [x] 4.3 Verify `links check`/`fix` and `denote:` conversion on attachments with integration tests: a renamed `.pdf` link reported fixable then repaired, `[query](denote:<ID>)` to a `.sql` rewritten to `../sql/...`, and `links backlinks` listing a note that links to a `.json` attachment

## 5. Documentation

- [x] 5.1 Update README.md: layout tree shows per-extension folders, commands table (2 columns) lists `attach FILE [--title] [--for ID]` with the image alias and the renderable rule, and the workflow example attaches a png and a pdf; verify the documented commands run against a scratch notebook

## 6. Integration checks

- [x] 6.1 Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `just crap` (every function at or below 30), and `openspec validate add-attachment-file-types --strict`; verify all pass and `rg ' as ' --type rust` finds no new `as` casts
- [x] 6.2 Build a scratch notebook with one attachment of each type linked from a note, run `mdbook build`, and verify the png/jpg/svg render as `<img>` and the pdf/sql/json files are present in `book/` with working links

## Workflow follow-up

- Archive `add-denote-mdbook-cli` before archiving this change, so the MODIFIED requirements here have base specs to apply to.
- Then run `/opsx:archive add-attachment-file-types`.
