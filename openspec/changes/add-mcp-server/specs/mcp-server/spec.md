# Spec Delta

## Purpose

Lets AI assistants and other Model Context Protocol (MCP) clients work with a notebook through typed tools that follow the same naming, linking, and layout rules as the CLI.

## ADDED Requirements

### Requirement: MCP subcommand over stdio
The `mcp` command SHALL run an MCP server that reads JSON-RPC messages from standard input and writes them to standard output until standard input closes. Standard output SHALL carry only protocol messages. The startup banner and any diagnostics SHALL go to standard error.

#### Scenario: Client handshake
- **WHEN** an MCP client launches `aa-denote-mdbook-tool --book ~/work-notes mcp` and sends `initialize`
- **THEN** the server replies with its name `aa-denote-mdbook-tool`, its package version, and a `tools` capability
- **AND** every line on standard output is a JSON-RPC message

#### Scenario: Client disconnects
- **WHEN** the client closes the server's standard input
- **THEN** the server exits with status 0

### Requirement: One notebook per server
The server SHALL find its notebook once at startup, in the same order as other commands: `--book DIR`, then `$AA_DENOTE_MDBOOK_TOOL_DIR`, then the nearest ancestor with `book.toml`. If none is found, the server SHALL print the usual error to standard error and exit non-zero before answering any request.

#### Scenario: Notebook from the environment
- **WHEN** `$AA_DENOTE_MDBOOK_TOOL_DIR` points to a notebook and the client starts `aa-denote-mdbook-tool mcp` from an unrelated directory
- **THEN** every tool acts on that notebook

#### Scenario: No notebook
- **WHEN** the server starts outside any notebook with no `--book` and no environment variable
- **THEN** it exits non-zero with the "no notebook found" error on standard error

### Requirement: Tool list
The server SHALL list exactly these tools, each with a description and a JSON Schema for its input: `new_note`, `import_note`, `attach_file`, `make_link`, `check_links`, `fix_links`, `backlinks`, `rename_note`, `regenerate_summary`, `search_notes`, `list_notes`, `read_note`. It SHALL NOT expose `init`, `build`, `serve`, or `preprocess`.

#### Scenario: Listing tools
- **WHEN** the client sends `tools/list`
- **THEN** the response names the twelve tools above and no others

### Requirement: Tools follow CLI rules
Each tool that mirrors a CLI command SHALL produce the same files, names, links, and SUMMARY.md changes as that command given the same inputs. Every tool that writes to the notebook (`new_note`, `import_note`, `attach_file`, `make_link` with `append`, `fix_links`, `rename_note`) SHALL regenerate SUMMARY.md and the day pages before returning.

#### Scenario: new_note matches new
- **WHEN** the client calls `new_note` with title "Acme sync", keywords `["acme","meeting"]`, and kind `manual`
- **THEN** the note is created at the same Denote path and with the same front matter as `new --title "Acme sync" --keywords acme,meeting` would create
- **AND** the result holds the note's identifier and its path relative to the notebook root
- **AND** SUMMARY.md lists the new note under today's day entry

### Requirement: Note creation tools
`new_note` SHALL accept `title`, `keywords`, `signature`, `parent`, `date`, and `kind` (`manual`, `summary`, or `transcript`, default `manual`). `import_note` SHALL accept `kind` (`summary` or `transcript`), the same note options, and exactly one of `path` (a file to read) or `content` (the text itself, with an optional `file_name` used for Zoom timestamp detection).

#### Scenario: Import from content
- **WHEN** the client calls `import_note` with kind `transcript`, `content` holding WebVTT text, and `parent` set to a meeting's identifier
- **THEN** a transcript note is created under `src/notes/zoom-transcripts/` with the text kept verbatim and linked from its parent

#### Scenario: Both path and content
- **WHEN** the client calls `import_note` with both `path` and `content`
- **THEN** the call fails with a tool error saying exactly one is required and no file is created

### Requirement: Attachment tool
`attach_file` SHALL accept `path` (a file on the server's machine), and optional `title`, `keywords`, `date`, and `for_note`. It SHALL copy the file into `src/<ext>/` under the same rules as `attach` and return the stored path and the Markdown reference that `attach` would print.

#### Scenario: Attach a pdf
- **WHEN** the client calls `attach_file` with `path` `/tmp/plan.pdf` and `title` "Plan"
- **THEN** the file is stored under `src/pdf/` with a Denote name and the result's reference uses link syntax `[Plan](...)`

### Requirement: Linking tools
`make_link` SHALL accept `target` and optional `from` and `append`, and return the link `link` would print. `check_links` SHALL return each broken link's file, line, destination, and fixable identifier if any. `fix_links` SHALL return the fixed, unresolved, and linkified entries `links fix` reports. `backlinks` SHALL accept `id` and return each linking note's path and title.

#### Scenario: Check finds a broken link
- **WHEN** a note links to a file that no longer exists and the client calls `check_links`
- **THEN** the result lists that file, line, and destination
- **AND** the call is not reported as a tool error

### Requirement: Rename and summary tools
`rename_note` SHALL accept `id` and at least one of `title`, `keywords`, or `signature`, and return the note's new path after updating links as `rename` does. `regenerate_summary` SHALL take no input and rewrite SUMMARY.md and the day pages as `summary` does.

#### Scenario: Rename with nothing to change
- **WHEN** the client calls `rename_note` with only `id`
- **THEN** the call fails with a tool error and no file is renamed

### Requirement: Search tool
`search_notes` SHALL accept `pattern` and optional `keywords` and `kind`, apply the same filters as `search`, and return ripgrep's matching lines as text in the result. It SHALL NOT accept extra ripgrep arguments. No matches SHALL be a successful result with empty text.

#### Scenario: Search with a keyword filter
- **WHEN** the client calls `search_notes` with pattern `roadmap` and keywords `["acme"]`
- **THEN** the result holds `path:line:text` lines only from notes tagged `acme`

#### Scenario: Nothing matches
- **WHEN** no note contains the pattern
- **THEN** the call succeeds with empty text

### Requirement: Read tools
`list_notes` SHALL return every note's identifier, title, kind, keywords, and path, newest identifier first, filtered by optional `kind` and `keywords` (all must match). `read_note` SHALL accept `id` and return the note's path and full Markdown, front matter included.

#### Scenario: List summaries
- **WHEN** the client calls `list_notes` with kind `summary`
- **THEN** only notes under `src/notes/zoom-summaries/` are returned, newest first

#### Scenario: Read an attachment by mistake
- **WHEN** the client calls `read_note` with the identifier of a png attachment
- **THEN** the call fails with a tool error saying it is an attachment, not a note

### Requirement: Tool errors keep the server running
When a tool cannot complete, the server SHALL return a tool result marked as an error, with the same message the CLI would print after `error:`, and SHALL keep serving requests. Invalid input that does not match a tool's schema SHALL also be reported without ending the server.

#### Scenario: Unknown identifier
- **WHEN** the client calls `backlinks` with an identifier no file has
- **THEN** the result is an error naming the identifier
- **AND** a following `list_notes` call succeeds
