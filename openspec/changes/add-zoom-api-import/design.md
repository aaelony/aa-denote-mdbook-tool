# Design

## Context

- `capture::create_note` already does the import work: picking the timestamp (`--date`, then the parent, then a Zoom file-name timestamp), defaulting titles and keywords from the parent, linking from the parent, and fencing `.vtt` content verbatim through `import_body` and `Source::Content`.
- After the MCP change, write commands call `summary::write`.
- The core crate is synchronous and has no network code.
- Front matter is rendered and parsed in `frontmatter.rs`, which ignores unknown keys, so adding an optional field is backward compatible.

See proposal.md for motivation and specs/ for the required behavior.

## Goals / Non-Goals

**Goals:**
- Reuse `create_note` for every note so naming, timestamps, defaults and parent links stay identical to `import`.
- Keep all decision logic pure and unit-testable without a network:
  - which meetings match a `--date`;
  - which note is the parent;
  - how a summary renders as Markdown;
  - what is a repeat.
- Make the whole command testable end to end against a local mock of Zoom.

**Non-Goals:**
- No async runtime for HTTP.
- No keyring integration.
- No retry or backoff beyond reporting Zoom's rate-limit error.

## Decisions

### Module split
- **`zoom.rs` (pure).** Types for a meeting (`uuid`, `topic`, `start` and `end` as local time, `has_summary`, `transcript_url`), plus these functions:
  - `select_meetings(meetings, when, has_time)`;
  - `match_parent(notes, meeting)`;
  - `render_summary(json) -> String`;
  - `already_imported(nb, uuid, kind)`;
  - `plan(...)`, which returns a list of planned imports used by both `--dry-run` and the real run.
- **`zoom_api.rs` (I/O).**
  - OAuth: the PKCE pair, the authorize URL, the one-shot callback listener, and the token exchange and refresh.
  - The config file.
  - The Zoom REST calls.
- The CLI only parses arguments, calls `plan`, prints, and calls `create_note` for each planned import.

### HTTP client: `ureq` with rustls
- It is synchronous, small, and needs no tokio in the CLI path. It handles the form posts, JSON and redirects (Zoom transcript downloads redirect).
- *Alternative:* `reqwest` blocking. It pulls in tokio and hyper for no gain here. Rejected.

### OAuth: authorization code with PKCE and a loopback redirect
Checked against Zoom's OAuth guide (https://developers.zoom.us/docs/integrations/oauth/).
- **Endpoints:** authorize at `https://zoom.us/oauth/authorize` with `response_type=code`, `client_id`, `redirect_uri`, `code_challenge` (Base64URL SHA-256 of a 43–128 character verifier), `code_challenge_method=S256` and a random `state`; token at `https://zoom.us/oauth/token`.
- **Client authentication:**
  - With a client secret, token requests use HTTP Basic `client_id:client_secret`.
  - Without one (the app's "Use Public Client OAuth" setting), they send `client_id` and `code_verifier` in the form body with no `Authorization` header, as Zoom documents for PKCE public clients.
  - Zoom does not document the refresh grant for public clients, so the refresh also sends `client_id` in the body. Task 6.2 verifies this on a real account.
- **Callback:** a `std::net::TcpListener` bound to `127.0.0.1:0` (any free port) accepts exactly one request.
  - Zoom ignores the port of a loopback redirect for PKCE clients, and the user registers `http://127.0.0.1/callback` once.
  - `localhost` is never used, as Zoom requires.
  - It checks `state`, answers with a short "you can close this tab" page, and stops listening.
  - The 5-minute wait ends with an error.
- **Browser:** the tool tries `xdg-open` or `open`, and always prints the URL.
- **Refresh:** access tokens last one hour and refresh tokens 90 days. Each refresh returns a new refresh token, and Zoom says to always use the latest, so every refresh saves the new pair before continuing.
- **Expiry:** the expiry is stored as an absolute time, and the token is refreshed 60 seconds early.

### Config file
- **Location:** `dirs::config_dir()/aa-denote-mdbook-tool/zoom.toml`. Tests override the directory with `AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR`.
- **Contents:** `client_id` and `client_secret` (both optional in the file, because environment variables win, and the secret is optional altogether), `access_token`, `refresh_token`, `expires_at`.
- **Writing:** a temporary file with mode 0600 (via `OpenOptionsExt` on Unix), then a rename. `logout` removes only the three token keys.

### Finding meetings and their artifacts
Checked against the official Meetings API spec (https://developers.zoom.us/api-hub/meetings/methods/endpoints.json).
- **Meetings with a summary:** `GET /v2/users/me/meeting_summaries?from=<UTC>&to=<UTC>&time_filter_field=summary_start_time`, paged by `next_page_token`, with scope `meeting:read:list_summaries`. `from` and `to` are `yyyy-MM-ddTHH:mm:ssZ` covering the local day. Each entry gives `meeting_uuid`, `meeting_topic`, `meeting_start_time` and `meeting_end_time`.
- **Meetings with a recording:** `GET /v2/users/me/recordings?from=<UTC date>&to=<UTC date>` covering the local day (the range is at most one month), paged by `next_page_token`, with scope `cloud_recording:read:list_user_recordings`. Each entry gives `uuid`, `topic`, `start_time`, `duration`, and `recording_files` whose `file_type == "TRANSCRIPT"` (`file_extension` VTT) marks a transcript.
- **Merging:** candidates from both lists are merged by UUID. `users/me/meetings?type=previous_meetings` is not used, because it returns only scheduled meetings and at most 6 months.
- **Summary body:** `GET /v2/meetings/{uuid}/meeting_summary` with scope `meeting:read:summary`. It is called only for meetings in the summaries list.
- **Transcript:** `GET /v2/meetings/{uuid}/transcript` with scope `cloud_recording:read:meeting_transcript`.
  - When `can_download` is true, the tool fetches `download_url` with the bearer token, following redirects.
  - Otherwise it reports `download_restriction_reason` (`NOT_READY`, `NO_TRANSCRIPT_DATA`, `DELETED_OR_TRASHED`, `UNSUPPORTED`).
- **UUID encoding:** a UUID that starts with `/` or contains `//` is URL-encoded twice in paths, as Zoom requires.
- **Signed-in user:** `GET /v2/users/me` with scope `user:read:user`, to print the email after `zoom login`.
- **Local time:** start times are converted from UTC to local time with chrono, the same way Zoom file-name timestamps are.
- **Base URLs:** tests replace them with `AA_DENOTE_MDBOOK_TOOL_ZOOM_API_URL` and `AA_DENOTE_MDBOOK_TOOL_ZOOM_OAUTH_URL`.

### Summary rendering
- If the response has `summary_content` (Markdown; Zoom calls it the unified field for all summaries), it is used as is.
- Otherwise the note is built as:
  - `## Overview` with `summary_overview`;
  - one `## <label>` per `summary_details` entry;
  - `## Next steps` as a bullet list.
- The result goes through `Source::Content` with file name `summary.md`, so bare URLs become links exactly as for a Markdown import.

### Transcript content
The VTT text goes through `Source::Content` with file name `transcript.vtt`, so it is fenced verbatim exactly like `import t.vtt`.

### Timestamps and parents
- **No parent:** the meeting start is passed to `create_note` as the `--date` value (an identifier string). The note's identifier is then the meeting start, the same rule as a Zoom file-name timestamp.
- **With a parent:** the parent's timestamp wins, as with `import --parent`.
- **Matching:** `match_parent` looks only at `manual` notes, computes the distance between each identifier time and the meeting start within [start - 15 min, end], and picks the smallest.

### Repeat detection via a `zoom_meeting` front matter field
- `FrontMatter` gains `zoom_meeting: Option<String>`, rendered after `imported`. `create_note` takes it through `NoteOptions`.
- `already_imported` scans notes of the given kind for that value.
- *Alternative:* skip a meeting when its parent already has a summary. That is wrong when a note has two meetings, or a meeting has no parent. Rejected.

### Failure handling
- Planning (all GET calls) finishes before any note is written. Most errors therefore create nothing.
- A failed transcript download midway keeps the notes already created, lists them, and exits non-zero.
- SUMMARY.md is regenerated if at least one note was created, even when the run then fails.

## Risks / Trade-offs

- [Zoom changes endpoints or scopes] → Endpoint paths and scope names sit only in `zoom_api.rs`. Zoom's error message and HTTP status are always shown.
- [Public-client token refresh is undocumented] → The refresh sends `client_id` in the body, and the user can configure a client secret to use Basic auth instead. Task 6.2 checks refresh on a real account.
- [Developers report `meeting_summary` errors such as 3001 "Meeting does not exist" or missing scopes, even when the list call works] → The UUID comes straight from the summaries list and is double-encoded when needed. On failure the tool prints Zoom's message and still imports the transcript.
- [Plan requirements] → Summaries need a Pro or higher plan with "Meeting Summary with AI Companion" on, and end-to-end encrypted meetings have none. Transcripts need Cloud Recording with audio transcripts. The README lists these, and `--dry-run` shows what is available.
- [Tokens stored in a plain file] → Owner-only permissions, never printed, and `zoom logout` removes them. A keyring is a non-goal.
- [Clock skew or DST around the matching window] → All comparisons use local `NaiveDateTime` after one conversion, the same as existing Zoom timestamp handling.

## Migration Plan

This is additive. Existing notes have no `zoom_meeting` field and parse unchanged. Users who never run `zoom` commands see no change.

## Open Questions

None.
