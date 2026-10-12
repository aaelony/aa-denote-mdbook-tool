# Tasks

## 1. SUMMARY.md regenerated after every CLI write

- [x] 1.1 Add an `after_write(nb)` helper in `src/cli.rs` that calls `summary::write`, and call it after `new`, `import`, `attach`, `link --append`, `links fix` and `rename` succeed (after the editor exits with `--edit`). Verify with new tests in `tests/rendering.rs`: `test_new_regenerates_summary`, `test_rename_regenerates_summary`, `test_failed_attach_leaves_summary_unchanged`.
- [x] 1.2 Update existing integration tests that assert on SUMMARY.md or `src/days/` contents after a write command without running `summary`, and verify `cargo test` passes.
- [x] 1.3 Update the README commands table and rendered-book paragraph to say write commands keep SUMMARY.md current, and verify that every command named there exists in `--help`.

## 2. Capturable search in core

- [x] 2.1 In `denote-mdbook-core/src/search.rs`, extract the filtered path list and batching into one helper, keep `search` streaming for the CLI, and add a function returning `(status, stdout, stderr)` via `Command::output()`. Verify the existing `tests/search.rs` still passes and a new unit or integration test (`test_search_capture_returns_matching_lines`) checks the captured text and the no-match status 1.

## 3. MCP server scaffolding

- [x] 3.1 Add `rmcp` (pinned 3.5.x, with the server, macros and stdio transport features), `tokio` (`rt`, `macros`, `io-std`), `serde` and `schemars` to the binary crate, and verify `cargo build` succeeds.
- [x] 3.2 Add the `Mcp` subcommand to `src/cli.rs` (help: "Run an MCP server over stdio for this notebook") and a `src/mcp.rs` module. It resolves the notebook with `Notebook::find`, holds it behind a `Mutex`, and serves on stdio in a current-thread tokio runtime, with server name `aa-denote-mdbook-tool` and the package version. Verify the `mcp` entry in `aa-denote-mdbook-tool --help`.
- [x] 3.3 Add a `tests/mcp.rs` test helper (`test_helper_mcp_session`) that spawns the binary with `mcp`, performs `initialize` and `notifications/initialized`, and sends `tools/call` requests line by line. Verify with `test_mcp_initialize_reports_name_and_version` and `test_mcp_stdout_is_only_json_rpc`.
- [x] 3.4 Verify the startup failure path with `test_mcp_outside_notebook_exits_nonzero`, which checks that the "no notebook found" error is on standard error and nothing is on standard output.
- [x] 3.5 Add a common error mapping from `anyhow::Error` to a tool result with `is_error: true` and the text `format!("{e:#}")`. Verify with `test_mcp_unknown_id_is_tool_error_and_server_continues` (`backlinks` on a bad id, then a successful `list_notes`).

## 4. Read tools

- [x] 4.1 Implement `list_notes` (optional `kind`, `keywords`), returning id, title, kind, keywords and path, newest first. Verify with `test_mcp_list_notes_filters_by_kind_and_keyword`.
- [x] 4.2 Implement `read_note` (`id`), returning the path and full Markdown. Verify with `test_mcp_read_note_returns_front_matter_and_body` and `test_mcp_read_note_rejects_attachment`.
- [x] 4.3 Implement `search_notes` (`pattern`, optional `keywords`, `kind`) on the capturing search: statuses 0 and 1 succeed, others are tool errors. Verify with `test_mcp_search_notes_keyword_filter` and `test_mcp_search_notes_no_match_is_empty_success`.

## 5. Write tools

- [x] 5.1 Implement `new_note` (title, keywords, signature, parent, date, kind), regenerating SUMMARY.md. Verify with `test_mcp_new_note_matches_cli_and_updates_summary`, which compares the file name and front matter with a `new` run under the same `AA_DENOTE_MDBOOK_TOOL_NOW`.
- [x] 5.2 Implement `import_note` (kind, note options, exactly one of `path` or `content` with optional `file_name`), regenerating SUMMARY.md. Verify with `test_mcp_import_note_from_content_links_parent` and `test_mcp_import_note_rejects_path_and_content_together`.
- [x] 5.3 Implement `attach_file` (path, title, keywords, date, for_note), returning the stored path and reference, and regenerating SUMMARY.md. Verify with `test_mcp_attach_pdf_returns_link_syntax` and `test_mcp_attach_unsupported_extension_is_tool_error`.
- [x] 5.4 Implement `make_link` (target, from, append), with append regenerating SUMMARY.md, and `backlinks` (id). Verify with `test_mcp_make_link_and_backlinks`.
- [x] 5.5 Implement `check_links` and `fix_links` (fix regenerates SUMMARY.md), returning structured entries. Verify with `test_mcp_check_links_reports_broken_link_as_success` and `test_mcp_fix_links_repairs_renamed_target`.
- [x] 5.6 Implement `rename_note` (id plus at least one of title, keywords, signature), regenerating SUMMARY.md, and `regenerate_summary`. Verify with `test_mcp_rename_note_updates_summary` and `test_mcp_rename_note_with_nothing_to_change_is_tool_error`.
- [x] 5.7 Verify `tools/list` returns exactly the twelve tools with `test_mcp_tools_list_is_exact`.

## 6. Documentation and integration checks

- [x] 6.1 Add an "MCP server" section to the README covering the `mcp` command, registering it with `claude mcp add aa-denote-mdbook-tool -- aa-denote-mdbook-tool --book ~/work-notes mcp`, the tool list, and the advice to use absolute file paths. Verify the command line matches the `--help` output.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `just crap`, and verify all pass with no function above a CRAP score of 30.
- [ ] 6.3 Manually connect a real MCP client (for example `claude mcp add` then `/mcp`) to a scratch notebook, call `new_note` and `list_notes`, and verify the note shows up in SUMMARY.md.

## Workflow follow-up

- Archive the change with `/opsx:archive` after review.
