# Tasks

## 1. Project setup

- [x] 1.1 Add `clap` (derive), `chrono`, `serde`, `toml`, `serde_json`, `pulldown-cmark`, `walkdir`, `pathdiff`, `anyhow` to `Cargo.toml`, plus dev-dependencies `assert_cmd`, `tempfile`; verify `cargo build` succeeds
- [x] 1.2 Replace `src/main.rs` with a clap skeleton declaring every subcommand (`init`, `new`, `import`, `image`, `link`, `links check|fix|backlinks`, `rename`, `summary`, `build`, `serve`, `search`, hidden `preprocess`) and the global `--book` option, plus empty modules from design.md; verify `cargo run -- --help` lists all subcommands

## 2. Naming and front matter

- [x] 2.1 Implement title slug, keyword, and signature normalization in `naming.rs`; verify unit tests cover every example in `specs/note-naming/spec.md` (slug rules, keyword sort and dedupe, signature)
- [x] 2.2 Implement building and parsing Denote file names (`ID[==SIG][--TITLE][__KW].md|.png`); verify round-trip unit tests for full name, signature, and identifier-only cases, and that non-Denote names are rejected
- [x] 2.3 Implement timestamp selection in `naming.rs`: `--date` parsing (all formats in the note-capture spec, including time-only and Denote identifier), parent identifier to timestamp, Zoom `GMTYYYYMMDD-HHMMSS` file-name extraction (UTC to local), clock fallback with the hidden `AA_DENOTE_MDBOOK_TOOL_NOW` override, and `YYYYMMDDTHHMMSS` formatting; verify unit tests for each source, the precedence order (date > parent > file name > clock), and invalid `--date` rejection
- [x] 2.4 Implement `+++` TOML front matter writing (fixed field order, optional `parent` and `imported`, `date` equal to the identifier time) and order-independent parsing in `frontmatter.rs`; verify unit tests that written output parses back with the `toml` crate and matches the spec scenario text

## 3. Notebook layout and discovery

- [x] 3.1 Implement `init [DIR] [--title]` creating `book.toml` (with `[preprocessor.aa-denote-mdbook-tool]`), `.ignore` with `book/`, `src/SUMMARY.md` with markers, `src/README.md`, and the four folders; refuse if `book.toml` exists; verify integration tests for both scenarios and that `mdbook build` succeeds on a fresh notebook
- [x] 3.2 Implement root discovery (`--book`, `AA_DENOTE_MDBOOK_TOOL_DIR`, ancestor `book.toml`) and kind-to-folder mapping with folder recreation; verify integration tests for subdirectory, precedence, not-found, and deleted-folder scenarios
- [x] 3.3 Implement the notebook scan returning notes and images keyed by identifier (ignoring non-Denote files); add identifier uniqueness by incrementing seconds; verify the "two notes in the same second" scenario with `AA_DENOTE_MDBOOK_TOOL_NOW`

## 4. Note capture

- [x] 4.1 Implement `new` with `--title`, `--keywords`, `--signature`, `--kind`, `--date`, printing the relative path; verify integration tests for the quick-note, no-title, and time-only `--date` scenarios with exact file names and contents, and that no `imported` field is written
- [x] 4.2 Implement `import <FILE|-> --kind summary|transcript` with `--date` (Markdown as-is, `.txt`/`.vtt` in a ```` ```text ```` fence, UTF-8 check, source untouched, Zoom file-name timestamp, `imported` field); verify integration tests for Markdown, VTT, stdin clock fallback, non-UTF-8, Zoom file name with a fixed `TZ`, and the late-import front matter (`date` vs `imported`) scenarios
- [x] 4.3 Implement `--parent <ID>` for `new` and `import`: record `parent`, take the parent's timestamp so attachments get `+1s`, `+2s`, inherit default title and keywords, append a relative link under `## Attachments` in the parent with a safe temp-file write; verify integration tests for attach, defaults, unknown parent, the noon import of summary then transcript (`T080001`, `T080002`), `--date` overriding parent, parent overriding a Zoom file name, and a next-day import grouped under the meeting's date
- [x] 4.4 Implement `--edit` launching `$VISUAL` then `$EDITOR`; verify integration test using a stub editor script that records its argument, and the error when neither is set
- [x] 4.5 Implement `image <FILE.png> [--title] [--keywords] [--date] [--for ID]`, printing a relative image reference; verify integration tests for the summary-note path (`../../png/...`) and non-png rejection

## 5. Linking

- [x] 5.1 Implement link extraction with `pulldown-cmark` byte ranges for `.md`, `.png`, and `denote:` targets, ignoring code blocks and external URLs; verify unit tests including a link inside a fenced block that must be skipped
- [x] 5.2 Implement `link <TARGET> [--from ID] [--append]`; verify the spec scenario output exactly
- [x] 5.3 Implement `links check` with file, line, target, and fixable marker, and non-zero exit on any broken link; verify integration tests for broken and all-valid scenarios
- [x] 5.4 Implement `links fix` (repair by identifier, convert `denote:` links, leave unresolvable links and report them); verify integration tests for move repair, `denote:` conversion, and unresolvable cases
- [x] 5.5 Implement `links backlinks <ID>`; verify integration test with two linking notes
- [x] 5.6 Implement `rename <ID> [--title] [--keywords] [--signature]` updating file name, front matter, and first heading, then fixing inbound links; verify integration tests for title change with inbound link and unknown identifier

## 6. Book rendering

- [x] 6.1 Implement `summary`: date-grouped parts newest first, identifier order within a day, parent nesting (summaries before transcripts), orphan handling, marker region rewrite preserving outside content; verify integration tests comparing generated SUMMARY.md to expected text for each spec scenario
- [x] 6.2 Implement `preprocess` (`supports <renderer>` exits 0; strip leading `+++` block from every chapter via `serde_json::Value`); verify a unit test on a sample mdBook JSON payload with nested sub_items
- [x] 6.3 Implement `build` and `serve` (`--open`, `--port`) regenerating SUMMARY then running `mdbook`, passing through exit status, with a clear error when `mdbook` is missing; verify an integration test that builds a notebook and checks the rendered HTML has the title heading and no `identifier =` text, plus a test with `PATH` cleared for the missing-mdbook error

## 7. Search

- [x] 7.1 Implement `search <PATTERN> [--keyword K]... [--kind K] [-- RG_ARGS]` filtering files from the scan, batching long path lists, and passing through `rg`'s exit status; verify integration tests for keyword plus kind filtering, pass-through args, no-matching-files status 1, and missing `rg`
- [x] 7.2 Verify the plain-ripgrep scenarios from `specs/note-search/spec.md` (`rg -l 'tags = .*"acme"'`, file-name match, no hits from `book/` after a build) with an integration test

## 8. Documentation and end-to-end check

- [x] 8.1 Update `README.md` with install (`cargo install --path .`), a command reference table (at most 3 columns), the folder layout, and the Monday-morning meeting workflow; verify every command shown runs as written in a temp notebook
- [x] 8.2 Run the full Monday-morning workflow end to end (init, new meeting note, import summary and transcript with `--parent`, add image, rename, `links check`, `build`, `search`); verify `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` all pass
- [x] 8.3 Print the startup banner (`<CARGO_PKG_NAME> <CARGO_PKG_VERSION>`) to stderr from `cli::main`, skipped for `preprocess`; verify the `banner_on_stderr_only` integration test (stderr banner, exact stdout, silent preprocessor) and that `--version` prints the same text on stdout only

## Workflow follow-up

- Archive the change with `/opsx:archive` after review.
