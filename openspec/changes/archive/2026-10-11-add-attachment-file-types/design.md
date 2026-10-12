# Design

## Context

See proposal.md for motivation. `png` is the only non-Markdown type, and it is hardcoded in four places in the core crate:

- `naming.rs`: `DenoteName::parse` accepts only `md` and `png`, so the scan, identifier lookup, identifier allocation, `link`, `backlinks`, and `links fix` never see other files.
- `links.rs`: `is_local_target` only considers destinations ending in `.md` or `.png`.
- `capture.rs`: `add_image` rejects non-`png` input, copies into `src/png/`, and always prints `![..](..)`.
- `notebook.rs`: `png_dir` creates `src/png/`; `init` calls it.

`make_link` always prints `[..](..)`, even for a png. mdBook copies every non-`.md` file under `src/` into the built book, so attachments render or download without preprocessor changes.

## Goals / Non-Goals

**Goals:**
- One table of supported extensions that every part of the tool consults, so adding a type later is a one-line change.
- Keep the `image` command and existing png notebooks working unchanged.

**Non-Goals:**
- Renaming attachments with `rename` (still notes only).
- Including attachments in keyword- or kind-filtered `search`. Unfiltered `search` already greps all of `src/`, which covers `.sql` and `.json` text; ripgrep skips binary PDFs.
- Listing attachments in `SUMMARY.md`. They stay out of the table of contents, as png files do today.
- Raw HTML `<img>` tags. Image syntax `![..](..)` renders as `<img>` in mdBook, and raw HTML would be invisible to `links check`/`fix` because pulldown-cmark reports it as HTML, not as a link.
- Creating every attachment folder in `init`. `init` keeps creating only `src/png/`; other folders appear on first use.

## Decisions

### 1. A single extension table in `naming.rs`

Add `pub const ATTACHMENT_EXTENSIONS: [&str; 6] = ["png", "jpg", "svg", "pdf", "sql", "json"]` plus `is_attachment_ext(ext)` and `is_renderable_ext(ext)` (`png`, `jpg`, `svg`). `DenoteName::parse` accepts `md` or any attachment extension. Alternative: an enum `AttachmentKind`. Rejected for now because the only behavior that varies is renderability, and a slice keeps parsing and error messages trivial. Alternative: accept any extension. Rejected because links such as `chapter.html` are valid in the rendered book but absent from `src/`, so `links check` would report false positives.

### 2. Case handling

Source files are matched case-insensitively (`Photo.JPG` is accepted) and the copy is written with the lowercase extension, matching Denote's lowercase names. `parse` stays strict: only lowercase extensions are Denote names, so `20261010T090000--x.PDF` written by hand is ignored, consistent with how uppercase titles are already rejected. `is_local_target` compares case-insensitively so a hand-written `Report.PDF` link is still checked.

### 3. Folder per extension: `Notebook::attachment_dir(ext)`

Replace `png_dir()` with `attachment_dir(ext) -> Result<PathBuf>` returning `src/<ext>/`, created if missing. `init` calls `attachment_dir("png")`. Alternative: one `src/attachments/` folder. Rejected by the user in favor of mirroring the existing `png/` convention.

### 4. `attach` command with `image` alias

In `src/cli.rs`, rename the `Image` variant to `Attach` with `#[command(visible_alias = "image")]`, keeping identical options (`--title`, `--keywords`, `--date`, `--for`). In core, `add_image` becomes `add_attachment` with the same signature; the reference is built by a shared `markdown_reference(text, path, ext)` helper that picks `![..]` or `[..]` via `is_renderable_ext`. `make_link` uses the same helper so `attach` and `link` cannot drift. Alternative: keep `image` as the only name. Rejected because `image contract.pdf` reads wrongly.

### 5. Link text for attachments

Unchanged from today: `attach` uses `--title` (default: source file stem) as the text; `link` uses the slug title from the file name, or the identifier when the name has no title. Attachments have no front matter to read a human title from.

## Risks / Trade-offs

- [Library API break: `add_image` and `png_dir` disappear] → Crate is 0.1.0 with no external users; the proposal marks it BREAKING (library only).
- [`.jpeg`, `.gif`, `.webp`, `.csv` are rejected though they are common] → The user chose exactly this list; the error message names the supported extensions, and Decision 1 makes adding one a one-line change.
- [Strict lowercase `parse` ignores hand-copied `X.PDF` Denote files] → Same rule already applies to titles; `attach` always writes lowercase, so only manual copies are affected.
- [CRAP threshold on `cli.rs` and `links.rs`] → New logic lives in small helpers with unit tests; run `just crap` before finishing.

## Migration Plan

No data migration: existing `src/png/` files and `![..](../png/..)` links keep their names and paths. Order of archiving: `add-denote-mdbook-cli` first, then this change, because this change's MODIFIED requirements apply on top of that change's specs.
