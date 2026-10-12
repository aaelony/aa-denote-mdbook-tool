# Design

## Context

- The workspace has two crates. The binary `aa-denote-mdbook-tool` holds `src/cli.rs` (clap parsing and printing). The library `aa-denote-mdbook-tool-core` holds all notebook logic, written as plain synchronous functions that return `anyhow::Result`.
- Most core functions already return data and leave printing to the CLI: `create_note`, `add_attachment`, `make_link`, `check`, `fix`, `backlinks`, `rename` and `summary::write`.
- The exception is `search::search`. It runs `rg` with inherited standard output and returns only the exit status. Under a stdio MCP server, that output would land in the protocol stream.
- The startup banner already goes to standard error, so it does not interfere with stdio.
- Today only `summary`, `build` and `serve` call `summary::write`.

See proposal.md for motivation and specs/ for the required behavior.

## Goals / Non-Goals

**Goals:**
- One place for each rule: tools call the same core functions as the CLI commands.
- Keep the core crate free of async and MCP dependencies.
- Make the server testable end to end with the built binary, like the existing integration tests.

**Non-Goals:**
- No concurrent request handling.
- No streaming of long search results.
- No pagination.

## Decisions

### Use the `rmcp` SDK with its stdio transport
The server uses `rmcp` (official Rust SDK, 3.5.x at the time of writing), its `#[tool_router]` / `#[tool]` macros, and `rmcp::transport::stdio()`. The SDK handles protocol version negotiation, `initialize`, `tools/list` and input schemas generated from `schemars` structs.
- *Alternative:* a hand-written JSON-RPC loop with `serde_json`. It has no new dependencies, but we would own protocol versioning, capability negotiation and schema generation, which drift as MCP evolves. Rejected.
- If the macro API in the current `rmcp` release differs from these names, follow the SDK's current examples. The spec, not the macro names, is the contract.

### Async only inside `mcp`
`cmd_mcp` builds a single-threaded `tokio` runtime and blocks on the server. No other command touches tokio.
- Tool handlers call the synchronous core functions directly. They do quick local file work, and `rg` is bounded by notebook size.
- *Alternative:* making core async. Rejected because it gains nothing for local file I/O and touches every function.

### Serialize tool calls with a mutex
The server holds the `Notebook` behind a `Mutex`, and each tool locks it for its whole call.
- Identifier assignment scans for collisions and then writes. Two concurrent `new_note` calls in the same second could otherwise race.
- One client drives the server, so the lock costs nothing in practice.

### Capture search output in core
`search.rs` gets a function that returns `(status, stdout)` by running `rg` with `Command::output()`. The existing streaming `search` stays as it is for the CLI.
- Both functions share one helper that builds the filtered path list and the batch loop, so the keyword and kind filters live in one place.
- `search_notes` maps status 0 and 1 to success (1 with empty text) and anything else to a tool error carrying `rg`'s standard error.
- `--`-style extra `rg` arguments are not exposed, so a client cannot pass flags such as `--pre` that run programs.

### Regenerate SUMMARY.md in the callers, not in core write functions
- In the CLI, a small `after_write(nb)` helper in `cli.rs` calls `summary::write` after `new`, `import`, `attach`, `link --append`, `links fix` and `rename`. With `--edit`, it runs after the editor exits.
- The MCP write tools call the same core `summary::write` after their operation.
- Core functions like `create_note` stay single-purpose, and their unit tests stay unaffected.
- *Alternative:* call `summary::write` inside each core function. That guarantees no caller forgets, but couples every write to a full notebook scan and changes core unit-test behavior. Rejected. Integration tests per command and per tool cover the "forgot to call it" risk instead.
- If the operation succeeds but regeneration fails, the command or tool reports the error. The note is still on disk, and running `summary` fixes the book.

### Results are structured JSON plus text
- Each tool returns a serde-serializable struct as structured content, which gives clients an output schema.
- The text content is the same JSON, for clients that ignore structured content.
- Errors use `CallToolResult` with `is_error: true`, with the text `format!("{e:#}")`. That is the message the CLI prints after `error: `.

### Paths given to tools are resolved against the server's working directory
- This applies to `import_note.path` and `attach_file.path`.
- Tool descriptions recommend absolute paths, because the client's idea of the current directory may differ from the server's.
- Returned paths are relative to the notebook root, as the CLI prints them via `Notebook::display`.

### `list_notes` and `read_note` built from existing scanning
- `list_notes` uses `Notebook::scan`, `Entry::is_note`, `Notebook::kind_of`, `Entry::title` and the parsed name's keywords, sorted by identifier, newest first.
- `read_note` uses `Notebook::find_note`, which already returns the "is an attachment, not a note" error, and then reads the file.
- Both live in `src/mcp.rs` unless a second caller appears.

## Risks / Trade-offs

- [Breaking change for CLI users] Write commands now also rewrite SUMMARY.md and `src/days/`. A user who hand-edits inside the generated region loses those edits sooner. → The region has always been documented as generated, and content outside the markers is preserved.
- [Slower write commands on large notebooks] Each write now rescans the notebook. → The scan is a directory walk plus front matter reads, already fast enough for `summary`. Measure with `just` profiling if it becomes noticeable.
- [rmcp API churn] The SDK changes quickly between major versions. → Pin the exact version in Cargo.toml and keep the MCP glue in one file.
- [Large search results] A broad pattern can return a large text block. → Accepted for now. The client chooses the pattern, and pagination is a non-goal.
- [Server reads arbitrary local files through `attach_file` and `import_note`] → It runs as the same user who launched the client, with the same access the CLI already has. No new privilege is added.

## Migration Plan

No data migration is needed. Existing notebooks work unchanged. The first write command after upgrading regenerates SUMMARY.md, just as running `summary` would.

## Open Questions

None.
