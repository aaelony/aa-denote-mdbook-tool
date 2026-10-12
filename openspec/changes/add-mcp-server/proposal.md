# Proposal

## Why

AI assistants such as Claude Code can only use the notebook today by shelling out to the CLI and parsing its text output. A Model Context Protocol (MCP) server would let an assistant create, find, read, link, and rename notes through typed tools with structured results, using the same rules as the CLI.

## What Changes

- New `mcp` subcommand: `aa-denote-mdbook-tool [--book DIR] mcp` runs an MCP server over standard input and output (stdio transport). An MCP client launches it as a child process.
- The server works on one notebook, found at startup the same way as every other command (`--book`, then `$AA_DENOTE_MDBOOK_TOOL_DIR`, then the nearest ancestor with `book.toml`).
- The server exposes tools that mirror the CLI:
  - `new_note`, `import_note`, `attach_file`
  - `make_link`, `check_links`, `fix_links`, `backlinks`
  - `rename_note`, `regenerate_summary`, `search_notes`
- The server adds two read tools the CLI does not have as commands, because an assistant cannot open files the way an editor user can:
  - `list_notes` (identifier, title, kind, keywords, path, filterable by kind and keyword)
  - `read_note` (the full Markdown of a note by identifier)
- **BREAKING** (minor): SUMMARY.md and the day pages are regenerated after every operation that writes to the notebook, in both the MCP tools and the CLI. The CLI commands `new`, `import`, `attach`, `link --append`, `links fix` and `rename` do not regenerate it today. Only `summary`, `build` and `serve` do.
- Tool failures, such as an unknown identifier or an unsupported attachment type, come back as MCP tool errors carrying the same message the CLI prints. They do not end the server.
- Standard output carries only MCP protocol messages. The startup banner stays on standard error, as it is today.
- Out of scope:
  - `init`, `build`, `serve` and `preprocess` (notebook creation, long-running processes, and the mdBook hook);
  - the `--edit` flag (there is no interactive editor);
  - HTTP and SSE transports;
  - MCP resources and prompts. Notes are reached through tools only in this change.

Assumptions made without asking (change them with `/opsx:update` if they are wrong):
- The server exposes the write operations (create, import, attach, rename, fix, summary), not only the read operations.
- Only stdio, one notebook per server process.

## Capabilities

### New Capabilities
- `mcp-server`: the `mcp` subcommand, its stdio transport, which notebook it serves, the tools it exposes with their inputs and outputs, and how tool errors are reported.

### Modified Capabilities
- `book-rendering`: SUMMARY.md is regenerated after every command that writes to the notebook, not only by `summary`, `build` and `serve`.

The `search` change that lets the server capture ripgrep output is internal and changes no CLI behavior.

## Impact

- `src/cli.rs`: new `Mcp` subcommand. The write commands call the SUMMARY.md regeneration after they succeed.
- New `src/mcp.rs` in the binary crate holding the server and the tool definitions. Notebook logic stays in `aa-denote-mdbook-tool-core`.
- `denote-mdbook-core/src/search.rs`: a variant that returns ripgrep's output as a string instead of inheriting standard output, because inherited output would corrupt the protocol stream.
- New dependencies in the binary crate: `rmcp` (the official Rust MCP SDK, currently 3.5.x), `tokio`, `serde`, and `schemars` for tool input schemas.
- New integration tests that drive the server over stdio.
- README: a section on registering the server with an MCP client, for example `claude mcp add`.
