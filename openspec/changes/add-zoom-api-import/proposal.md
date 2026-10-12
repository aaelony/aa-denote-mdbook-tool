# Proposal

## Why

Bringing Zoom output into the notebook today takes four manual steps: download the AI summary and transcript from the Zoom web portal, find the meeting's note identifier, then run `import` twice with `--parent`. Fetching both directly from Zoom's API by the meeting's date and time removes the downloads and the identifier lookup. That makes the noon "attach Zoom output" step of the Monday workflow a single command.

## What Changes

- New `zoom login` command:
  - signs in with Zoom user OAuth: authorization code with PKCE, through the browser and a one-time loopback callback on `http://127.0.0.1:<any free port>/callback`;
  - stores the tokens in the user's config directory with owner-only permissions.
  - The user's own Zoom Marketplace app supplies the client ID, ideally with "Use Public Client OAuth" so no client secret is needed. A client secret is used when one is configured. No Zoom account admin is needed.
- New `zoom logout` command, which deletes the stored tokens.
- New `zoom import --date <WHEN>` command:
  - `--date "YYYY-MM-DD HH:MM"` imports the one meeting that started at that time (within a few minutes).
  - `--date YYYY-MM-DD` imports every meeting that started that local day.
  - For each meeting, it creates a `summary` note from the AI Companion meeting summary and a `transcript` note from the cloud recording transcript (VTT, kept verbatim), whichever exist.
  - Each meeting's notes attach to `--parent ID` when given. Otherwise they attach to the manual note whose identifier time is closest to the meeting's start on that day, within the matching window. If no note is in the window, they are imported without a parent.
  - Imported notes record the Zoom meeting UUID in front matter. Running the command again skips meetings already imported, so it is safe to repeat.
  - `--dry-run` lists what would be imported, and under which parent, without writing anything.
  - The existing `--title`, `--keywords` and `--signature` defaults from the parent apply, as with `import`.
  - SUMMARY.md is regenerated after the import, as for every other write command.
- Out of scope:
  - Server-to-Server OAuth and account-wide (admin) access;
  - importing by meeting ID;
  - downloading video, audio or chat files;
  - an MCP tool for Zoom import;
  - background or scheduled syncing.

Decisions checked against Zoom's OAuth guide and the official Meetings API spec:
- Credentials come from `ZOOM_CLIENT_ID` (required) and `ZOOM_CLIENT_SECRET` (optional), or the same keys in the config file.
- Zoom ignores the port of a `127.0.0.1` loopback redirect for PKCE clients, so the tool listens on any free port and has no `--port` option. Zoom says not to use `localhost`.
- Meetings are found with `GET /users/me/meeting_summaries` and `GET /users/me/recordings`. `users/me/meetings?type=previous_meetings` is not used, because it skips instant meetings and covers at most 6 months.
- The Zoom app needs the user-level scopes `meeting:read:list_summaries`, `meeting:read:summary`, `cloud_recording:read:list_user_recordings`, `cloud_recording:read:meeting_transcript`, and `user:read:user`.

Assumptions made without asking:
- The matching window for "closest manual note" runs from 15 minutes before the meeting start until the meeting ends. The window for a `--date` with a time is ±10 minutes around the meeting start.

## Capabilities

### New Capabilities
- `zoom-import`: Zoom sign-in and sign-out, token storage and refresh, choosing meetings by date and time, turning summaries and transcripts into notes, matching parents, skipping repeats, dry runs, and the errors shown for missing credentials, expired sign-in, and missing Zoom data.

### Modified Capabilities
- `note-naming`: the "Note front matter" requirement gains an optional `zoom_meeting` field (the Zoom meeting UUID) on notes imported from the Zoom API.

## Impact

- Code:
  - New `denote-mdbook-core/src/zoom.rs` holds the pure logic: selecting meetings, matching parents, rendering a summary as Markdown and building notes. A Zoom client module holds the OAuth and HTTP calls.
  - New `zoom` subcommands in `src/cli.rs`.
  - `frontmatter.rs` gains the `zoom_meeting` field.
- Dependencies in the core crate:
  - an HTTP client (`ureq`, sync, with rustls);
  - `sha2` and `base64` for PKCE;
  - `getrandom` for the state and verifier values;
  - `dirs` for the config directory.
  - A mock HTTP server crate is added as a dev-dependency for tests.
- External: the user creates a Zoom Marketplace "General" app with user-managed OAuth and "Use Public Client OAuth" turned on. The app registers the redirect URL `http://127.0.0.1/callback` and the five scopes above. Summaries need a Pro or higher plan with "Meeting Summary with AI Companion" on. Transcripts need Cloud Recording with audio transcripts on.
- README: a section on creating the Zoom app, `zoom login`, and the import workflow.
