# Design

## Context

- Greenfield: `src/main.rs` is a "Hello, world!" placeholder and `Cargo.toml` has no dependencies. Edition 2024; local toolchain is rustc 1.99, mdbook 0.4.40, and ripgrep are installed.
- Motivation is in proposal.md (Why). Behavior is defined by the six specs under `specs/`; this document covers how to build it.
- mdBook renders only files listed in `SUMMARY.md` and does not understand front matter, so a `+++` TOML block would otherwise render as a horizontal rule plus text. This drives the preprocessor decision below.
- Denote's own `markdown-toml` front matter uses `title`, `date`, `tags`, and `identifier`. Our extra `kind`, `parent`, and `imported` keys are ignored by Denote, so notes stay editable from Emacs Denote.

## Goals / Non-Goals

**Goals:**
- One self-contained binary that is both the CLI and the mdBook preprocessor.
- No hidden state: the file system (names plus front matter) is the only source of truth, rescanned on each run.
- Deterministic, testable naming and SUMMARY generation.

**Non-Goals:**
- No persistent index, cache, or database.
- No parsing of VTT cue structure (transcripts are kept verbatim).
- No support for Org or plain-text Denote file types; Markdown only.
- No Windows-specific path handling beyond what `std::path` gives for free.

## Decisions

**1. Crates**

| Need | Crate | Why over alternatives |
|---|---|---|
| CLI parsing | `clap` (derive) | Standard; subcommands and help for free |
| Time | `chrono` | `Local::now()` and RFC 3339 with offset; mature |
| Front matter | `toml` | Parse `+++` blocks as a TOML table (no direct `serde` dependency needed) |
| Preprocessor I/O | `serde_json` | Read and write mdBook's JSON protocol |
| Markdown links | `pulldown-cmark` | Finds real links, skipping code blocks |
| Directory walk | `walkdir` | Simple recursive scan of `src/` |
| Relative paths | `pathdiff` | Correct `../` computation between files |
| Errors | `anyhow` | Context-rich errors for a CLI |
| Tests | `assert_cmd`, `tempfile` | End-to-end CLI tests in temp notebooks |

Alternative considered: `jiff` for time. Rejected only for familiarity; either works.

**2. Workspace and module layout**

A Cargo workspace with two crates, so the logic can be reused as a library:

- `denote-mdbook-core/` (library package `aa-denote-mdbook-tool-core`): all behavior, no clap dependency.
- Root package `aa-denote-mdbook-tool` (binary, installed as the `aa-denote-mdbook-tool` command via `[[bin]]`): a thin clap front end. Keeping the binary at the root means `cargo install --path .` still works; `default-members` covers both crates so `cargo test` runs everything.

Binary (`src/`):

- `main.rs`: declares `cli` and calls `cli::main()`.
- `cli.rs`: clap definitions (including CLI-side kind enums converted to `Kind` with `From`), command dispatch, printing, and the startup banner.

Library (`denote-mdbook-core/src/`):

- `naming.rs`: slug, keyword, signature normalization; build and parse Denote file names; timestamp selection. Pure functions, heavily unit tested.
- `frontmatter.rs`: parse and write the `+++` TOML block. Writing uses a fixed field order (`title`, `date`, `tags`, `identifier`, `kind`, `parent`, `imported`) with one space around `=` so plain `rg 'tags = ...'` patterns match (Denote itself aligns the `=` signs, but both are valid TOML); parsing uses `toml` with unknown keys ignored.
- `notebook.rs`: root discovery (`--book`, `AA_DENOTE_MDBOOK_TOOL_DIR`, ancestor `book.toml`), `init`, kind-to-folder mapping, and a scan that returns every note and image keyed by identifier.
- `capture.rs`: `new`, `import`, `image`, `--parent` attachment.
- `links.rs`: link extraction, check, fix, backlinks, `link` generation, used by `rename` too.
- `summary.rs`: build the grouped tree, rewrite the marker region, and `run_mdbook` for `build`/`serve`.
- `preprocess.rs`: mdBook preprocessor.
- `external.rs`: locating and running `mdbook`, `rg`, and the editor with clear "not installed" errors.

**3. Timestamp selection and identifier allocation**

The identifier records when the content happened, not when the file was written, because notes are often captured hours after a meeting. A pure function `resolve_timestamp` returns the first available of:

1. `--date`: parsed as local time in the accepted formats (see `specs/note-capture/spec.md`). A time-only value uses today's date.
2. `--parent`: the parent's timestamp, parsed from its identifier.
3. A Zoom file-name timestamp: the pattern `GMT<8 digits>-<6 digits>` anywhere in the source file name, read as UTC and converted with `chrono::Local`. Only `import` of a real file (not `-`) checks this.
4. The clock: `Local::now()`.

Allocation then scans existing identifiers (notes and images share one namespace) and adds one second while the candidate is taken. Starting from the parent's own identifier means the first attachment lands at `+1s`, the next at `+2s`, so attachments sort directly after their meeting with no special casing.

`date` in front matter is the allocated identifier's time. `import` also writes `imported`, the actual clock time. A hidden `AA_DENOTE_MDBOOK_TOOL_NOW` environment variable (RFC 3339) overrides the clock, for both fallback 4 and `imported`, so integration tests produce exact file names.

Alternatives considered:
- Increment with a random suffix. Rejected because Denote itself increments, and suffixes break the format.
- File modification time as a fallback. Rejected because downloads and copies reset it, so it is often the import time anyway.
- Parsing timestamps inside VTT or summary content. Rejected because Zoom VTT cues are offsets from the recording start, not wall-clock times, and AI summaries carry no reliable timestamp.
- Putting the file-name timestamp ahead of `--parent`. Rejected because the parent is the user's explicit statement of which meeting this belongs to, while the file name may come from a recording that started early or was renamed.

**4. mdBook preprocessor via raw JSON, not the `mdbook` crate**

`aa-denote-mdbook-tool preprocess` reads `[context, book]` from stdin as `serde_json::Value`, walks `sections` and nested `Chapter.sub_items`, strips a leading `+++ ... +++` block from each `content`, and writes the book back. `preprocess supports <renderer>` exits 0. Working on `Value` preserves fields we do not know about. Alternative: depend on `mdbook` as a library. Rejected because it is heavy and couples us to its exact version; the JSON protocol is small and stable.

`init` writes:

```toml
[preprocessor.aa-denote-mdbook-tool]
command = "aa-denote-mdbook-tool preprocess"
```

**5. Link rewriting by byte range**

Use `pulldown-cmark`'s offset iterator to find each inline link and image, then locate the destination text inside that source range and replace just those bytes. Edits are applied back to front so offsets stay valid. Alternative: a regex over raw text. Rejected because it would rewrite "links" inside code fences and transcripts. Only targets that end in `.md` or `.png` (after stripping `#fragment`) or start with `denote:` are considered; external URLs are ignored.

Identifier extraction from a broken target uses the file name regex `^\d{8}T\d{6}`. Fixing replaces the path with `pathdiff::diff_paths(target, linking_file_dir)` using forward slashes.

**6. SUMMARY region**

The generated region sits between `<!-- aa-denote-mdbook-tool:begin -->` and `<!-- aa-denote-mdbook-tool:end -->`. `init` writes a SUMMARY with `[Introduction](README.md)` above the markers. Nesting: a note whose `parent` resolves becomes a `sub_item`; children sort summaries before transcripts, then by identifier. Parent cycles or a parent that is itself a child are flattened to one level to keep output valid.

Dates: newest first, as the daily review use case reads today first. This is a single sort and can be made a flag later without changing the specs' structure.

**7. Safe writes**

Every modification of an existing file (parent attachments, link fixes, renames, SUMMARY) writes to a temp file in the same directory and renames over the original, so an interrupted run never leaves a half-written note. A rename that also needs link fixes renames first, then fixes links; a failure during link fixes is reported and can be finished with `links fix`.

**8. Search filtering**

The tool filters files itself (via the notebook scan, matching parsed keywords and kind) and passes the resulting paths to `rg` along with the pattern and pass-through args. With no filters it passes `src/` directly. Alternative: translate filters into `rg --glob` patterns. Rejected because "all keywords must match" is awkward to express as globs.

**9. Rust conventions**

Prefer `From`/`TryFrom` conversions over `as` casts. Library-style modules return `anyhow::Result`; only `cli::main` prints errors and sets the exit code. External tool exit codes are passed through using `std::process::ExitCode::from`.

**10. Startup banner on stderr**

`cli::main` prints `env!("CARGO_PKG_NAME")` and `env!("CARGO_PKG_VERSION")` to standard error after argument parsing, so the version always matches `Cargo.toml`. Standard error, because standard output carries note paths from `new`/`import`, links, search results, and the preprocessor JSON that mdBook parses; a banner there would break scripts and mdBook. `preprocess` is skipped because mdBook runs it twice per build. `--help` and `--version` exit inside clap before the banner, and `--version` already shows the same text. Alternative: stdout with a `--quiet` flag. Rejected because the safe default should not require a flag.

## Risks / Trade-offs

- [Zoom changes its file-name pattern, or the user renames the file] → The pattern is only the third fallback; `--parent` or `--date` always override it, and a non-matching name falls through to the clock.
- [A notebook moved across time zones shows different local times for the same Zoom UTC stamp] → Conversion happens once, at import; the identifier is then fixed, as with any Denote note.
- [`--parent` was forgotten on a late import, so the note gets the clock time] → `rename` cannot change identifiers; the user deletes and re-imports with `--parent` or `--date`. The `imported` field makes such notes easy to spot.

- [Running plain `mdbook build` without `aa-denote-mdbook-tool` on `PATH` fails at the preprocessor step] → `init` prints a note; `build`/`serve` always use the installed binary; README documents `cargo install --path .`.
- [Very large notebooks make the per-command rescan slow] → Scan reads only file names except where front matter is needed (summary, search filters, backlinks). Thousands of notes stay well under a second; revisit with a cache only if measured.
- [Long file lists passed to `rg` could hit OS argument limits] → Batch paths into multiple `rg` runs if the list exceeds a threshold, merging exit codes (0 if any matched).
- [Users hand-edit file names and break the format] → Scan ignores files that do not parse as Denote names and `links check` reports links to them as unresolvable rather than crashing.
- [Front matter edits by Emacs Denote reorder or reformat fields] → Parsing is order-independent; the tool only rewrites front matter on `rename`.
- [Transcripts in fenced blocks render as one long monospace block] → Acceptable for "full reference, not always looked at"; VTT cleanup is listed as a later improvement.

## Migration Plan

Not applicable: no existing users or data. Install with `cargo install --path .`; uninstall with `cargo uninstall aa-denote-mdbook-tool`. Notebooks remain plain Markdown and keep working with plain `mdbook` if the preprocessor table is removed from `book.toml`.

## Open Questions

- Should `.vtt` transcripts later be converted to readable "Speaker: text" paragraphs instead of a fenced block? Deferrable; it only changes import formatting.
- Should rendered chapters show tags and date as a small line under the heading? Deferrable; it is an addition to the preprocessor.
- Should image formats beyond `png` (for example `jpg`, `svg`) be accepted later? Deferrable; the README only asks for `png`.
