# Proposal

## Why

Back-to-back meetings produce three kinds of notes at once: manual notes, AI summaries from Zoom, and full AI transcripts from Zoom. Denote gives fast, timestamped, greppable filenames but only inside Emacs and with a flat layout. mdBook gives a browsable rendered book but requires a specific directory layout and a hand-maintained `SUMMARY.md`. Nothing ties these together. A small Rust CLI that names files the Denote way and places them the mdBook way lets notes be captured in seconds, rendered into a book with one command, searched with `ripgrep`, and reused as a knowledge base for LLMs and other tools.

## What Changes

- New Rust CLI binary `aa-denote-mdbook-tool` (replacing the placeholder `src/main.rs`).
- `init` command that scaffolds an mdBook-compatible notebook: `book.toml`, `src/SUMMARY.md`, and the folders for manual notes, Zoom AI summaries, Zoom AI transcripts, and `png` images.
- `new` command that creates a note with a Denote-style filename (`IDENTIFIER--title__keywords.md`, identifier `YYYYMMDDTHHMMSS` in local time) and a front matter block, placed in the correct folder for its kind.
- `import` command that copies an existing Zoom AI summary or transcript file into the notebook with a Denote name, optionally attached to a parent meeting note so it appears as a subchapter.
- Timestamps reflect when the content happened, not when it was captured: `--date`, else the parent meeting's time, else a Zoom file-name timestamp, else the clock. Imports done hours or a day later still sort and group with their meeting, and an `imported` field records the actual import time.
- `image` command that copies a `png` into the `png` folder with a Denote name and prints a relative Markdown image reference.
- Linking: notes reference each other with ordinary relative Markdown links, and the tool resolves notes by their stable Denote identifier, so `link` produces a correct relative path and `links check` / `links fix` detect and repair links after a file is renamed or moved.
- `summary` command that regenerates `src/SUMMARY.md` from the files on disk (chronological meeting chapters, with summaries and transcripts as nested subchapters), and `build` / `serve` commands that run `mdbook`.
- `rename` command that changes a note's title or keywords while keeping its identifier, then fixes inbound links.
- `search` command as a thin wrapper over `ripgrep` scoped to the notebook, with filters by keyword and kind.
- Every command (except the hidden mdBook preprocessor) prints `aa-denote-mdbook-tool <version>` to standard error at startup; standard output stays reserved for paths, links, and other output meant for scripts and mdBook.

## Capabilities

### New Capabilities
- `notebook-layout`: The on-disk structure of a notebook: mdBook root files, folder per note kind (manual, zoom summary, zoom transcript), the `png` folder, and how the tool locates a notebook root.
- `note-naming`: The Denote file-naming scheme (identifier, signature, title slug, keywords, extension), front matter contents, identifier uniqueness, and renaming rules.
- `note-capture`: Creating new notes, importing Zoom AI summaries and transcripts, attaching them to a parent note, choosing each note's timestamp, and adding `png` images.
- `note-linking`: Creating relative links between notes and images by identifier, checking links, and repairing links after renames or moves.
- `book-rendering`: Generating `SUMMARY.md` from the notebook contents and invoking `mdbook` to build or serve the rendered book.
- `note-search`: Making notes easy to find with `ripgrep`, including the `search` wrapper and its filters.
- `cli-output`: What the CLI writes to standard output versus standard error, starting with the startup banner.

### Modified Capabilities
- None (greenfield project, no existing specs).

## Impact

- Code: `src/main.rs` replaced; new modules under `src/` (CLI, naming, layout, links, summary, search).
- Dependencies: `Cargo.toml` gains crates for argument parsing, time handling, file walking, and error handling (chosen in design).
- External tools: `mdbook` is required for `build` / `serve`; `rg` (ripgrep) is required for `search`. Both are invoked as subprocesses and are optional for the other commands.
- Out of scope: direct integrations with Zoom, GitHub, JIRA, or Aha! APIs; Emacs integration; editing Denote files in Org or plain-text formats. Notes remain plain Markdown with front matter so those tools (and LLMs such as Claude) can consume them later.
