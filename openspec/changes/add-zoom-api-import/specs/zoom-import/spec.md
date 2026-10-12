# Spec Delta

## Purpose

Fetches Zoom AI Companion meeting summaries and cloud recording transcripts from Zoom's API and imports them as notes attached to the meeting they belong to, chosen by the meeting's date and time.

## ADDED Requirements

### Requirement: Zoom sign-in
`zoom login` SHALL run Zoom's user OAuth authorization code flow with PKCE (S256) and a random `state`. It SHALL listen on a free port of `127.0.0.1` (never `localhost`), print the authorization URL with redirect `http://127.0.0.1:<port>/callback` (and try to open it in a browser), accept one redirect, then exchange the code for tokens and store them.

#### Scenario: Successful sign-in
- **WHEN** credentials are configured and the user approves access in the browser
- **THEN** the command prints the signed-in Zoom user's email and exits 0
- **AND** the tokens are stored in the config file

#### Scenario: State mismatch
- **WHEN** the callback's `state` differs from the one sent
- **THEN** the command exits non-zero and no tokens are stored

### Requirement: Credentials and token storage
The client ID SHALL come from `ZOOM_CLIENT_ID`, else `client_id` in `<config dir>/aa-denote-mdbook-tool/zoom.toml`. A client secret (`ZOOM_CLIENT_SECRET` or `client_secret`) is optional: with one, token requests use HTTP Basic auth; without one, they send `client_id` as a public client. Tokens SHALL be stored in that file with owner-only permissions (0600 on Unix) and never printed.

#### Scenario: Missing client ID
- **WHEN** neither the environment nor the config file provides a client ID
- **THEN** `zoom login` and `zoom import` exit non-zero with an error naming `ZOOM_CLIENT_ID` and the config file path

#### Scenario: Public client without a secret
- **WHEN** only `ZOOM_CLIENT_ID` is set and the user signs in
- **THEN** the token request carries `client_id` and `code_verifier` in the form body and no `Authorization` header

#### Scenario: Config file permissions
- **WHEN** `zoom login` writes `zoom.toml` on Unix
- **THEN** the file's mode is `0600`

### Requirement: Token refresh and sign-out
Before calling Zoom, the tool SHALL refresh an expired access token (Zoom access tokens last one hour) with the stored refresh token and save the new access and refresh tokens, since Zoom returns a new refresh token each time and the latest must be used. If refreshing fails, it SHALL exit non-zero asking the user to run `zoom login` again. `zoom logout` SHALL remove the stored tokens and keep the credentials.

#### Scenario: Expired access token
- **WHEN** the stored access token has expired and the refresh token is valid
- **THEN** `zoom import` refreshes it, saves the new tokens, and continues

#### Scenario: Refresh rejected
- **WHEN** Zoom rejects the refresh token
- **THEN** `zoom import` exits non-zero with "run `aa-denote-mdbook-tool zoom login`" in the error and no note is created

### Requirement: Select meetings by date and time
`zoom import --date <WHEN>` SHALL accept the `--date` formats used by other commands. A date with a time SHALL select the one meeting of the signed-in user whose start is closest to that time, within 10 minutes either side. A date alone SHALL select every meeting that started on that local day. Candidates SHALL come from the user's meeting summaries and cloud recordings, so instant meetings are included.

#### Scenario: Date and time
- **WHEN** the user had meetings starting at 08:01 and 09:00 on 2026-10-10 and runs `zoom import --date "2026-10-10 08:00"`
- **THEN** only the 08:01 meeting is imported

#### Scenario: Whole day
- **WHEN** the user runs `zoom import --date 2026-10-10`
- **THEN** both meetings are imported, in start-time order

#### Scenario: Nothing at that time
- **WHEN** no meeting with a summary or transcript started within 10 minutes of the given time
- **THEN** the command exits non-zero, lists that day's meetings with their start times and topics, and creates no note

### Requirement: Notes created per meeting
For each selected meeting, the tool SHALL create a `summary` note from the meeting's AI summary (overview, sections, next steps as Markdown) and a `transcript` note from its VTT transcript, kept verbatim like an imported `.vtt` file. A meeting without one of them SHALL get only the other note. The note timestamp, title and keyword defaults SHALL follow the same rules as `import`, with the meeting's start time in place of a Zoom file name timestamp.

#### Scenario: Summary and transcript
- **WHEN** a meeting has both an AI summary and a cloud recording transcript
- **THEN** one note is created in `src/notes/zoom-summaries/` and one in `src/notes/zoom-transcripts/`, and the command prints both paths

#### Scenario: Transcript only
- **WHEN** a meeting has a transcript but AI Companion was off
- **THEN** only the transcript note is created and the command says the summary was not available

#### Scenario: Transcript not ready
- **WHEN** Zoom reports the meeting's transcript cannot be downloaded yet
- **THEN** only the summary note is created and the command prints Zoom's reason (for example `NOT_READY`)

### Requirement: Parent matching
With `--parent ID`, all imported notes SHALL attach to that note (only valid when one meeting is selected). Without it, each meeting's notes SHALL attach to the `manual` note whose identifier time is closest to the meeting start within [start - 15 min, end]. If none qualifies, the notes SHALL be imported without a parent and the command SHALL say so.

#### Scenario: Matching note found
- **WHEN** manual note `20261010T080000` exists and a meeting ran from 08:01 to 08:45
- **THEN** the summary and transcript are attached to `20261010T080000`, with titles defaulting to its title plus ` AI Summary` and ` Transcript`

#### Scenario: Parent with a whole day
- **WHEN** the user passes `--parent` with a date that selects two meetings
- **THEN** the command exits non-zero before creating any note

### Requirement: Repeat imports are skipped
Notes imported from Zoom SHALL record the meeting UUID in front matter. Before creating a note, the tool SHALL skip any meeting and kind whose UUID is already in a note of that kind, and SHALL say it was skipped.

#### Scenario: Running twice
- **WHEN** the user runs the same `zoom import --date 2026-10-10` twice
- **THEN** the second run creates no notes and reports each meeting as already imported

### Requirement: Dry run
`zoom import --dry-run` SHALL print, for each selected meeting, its start time, topic, which of summary and transcript are available, and the parent it would attach to, and SHALL write nothing to the notebook.

#### Scenario: Preview
- **WHEN** the user runs `zoom import --date 2026-10-10 --dry-run`
- **THEN** the planned imports are printed and no file under `src/` changes

### Requirement: Summary regenerated after import
After creating at least one note, `zoom import` SHALL regenerate SUMMARY.md and the day pages, as other write commands do.

#### Scenario: Imported notes in the book
- **WHEN** `zoom import` creates a summary under meeting `20261010T080000`
- **THEN** SUMMARY.md lists it nested under that meeting

### Requirement: Zoom API errors
HTTP failures, rate limits, and missing scopes SHALL make the command exit non-zero with Zoom's error message and the HTTP status. Notes already created in the same run SHALL be kept and listed.

#### Scenario: Missing scope
- **WHEN** the Zoom app lacks the meeting summary scope
- **THEN** the error names the failing request and Zoom's message about the missing scope
