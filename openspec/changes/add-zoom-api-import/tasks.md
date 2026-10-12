# Tasks

## 1. Confirm the Zoom API

- [x] 1.1 Check the user OAuth endpoints, PKCE and public-client support, the loopback redirect rules, the `users/me/meeting_summaries`, `users/me/recordings`, `meetings/{uuid}/meeting_summary` and `meetings/{uuid}/transcript` endpoints, their JSON fields, and the scope names against Zoom's OAuth guide and official Meetings API spec. Record the differences in design.md and confirm with `openspec validate add-zoom-api-import --strict`.

## 2. Front matter field

- [x] 2.1 Add `zoom_meeting: Option<String>` to `FrontMatter` (rendered after `imported`, parsed when present) and to `NoteOptions`, passed through by `create_note`. Verify with unit tests `test_zoom_meeting_round_trips` and `test_notes_without_zoom_meeting_render_unchanged`, and with the existing `cargo test` suite.

## 3. Pure Zoom logic (core `zoom.rs`)

- [x] 3.1 Implement `select_meetings`: time given means the closest start within ±10 min; date only means every meeting starting that local day, in start order; only meetings with a summary or transcript count. Verify with `test_select_meetings_by_time_picks_closest`, `test_select_meetings_by_day_in_start_order`, and `test_select_meetings_none_in_window`.
- [x] 3.2 Implement `match_parent`: the closest `manual` note identifier in [start - 15 min, end]. Verify with `test_match_parent_closest_manual_note`, `test_match_parent_ignores_summaries_and_transcripts`, and `test_match_parent_none_outside_window`.
- [x] 3.3 Implement `render_summary`: `summary_content` is used as is, otherwise Overview, labeled sections and Next steps. Verify with `test_render_summary_from_sections` and `test_render_summary_prefers_summary_content`.
- [x] 3.4 Implement `already_imported` and `plan`. `plan` returns the per-meeting kinds to create or skip and the parent, and rejects `--parent` when more than one meeting is selected. Verify with `test_plan_skips_already_imported_kind` and `test_plan_rejects_parent_for_many_meetings`.

## 4. Zoom client and sign-in

- [x] 4.1 Add `ureq` (rustls), `sha2`, `base64`, `rand` and `dirs` to the core crate and a mock HTTP server dev-dependency, then verify `cargo build` succeeds.
- [x] 4.2 Implement config loading (environment first, then `zoom.toml`), the 0600 atomic save, and the `AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR` and base URL overrides. Verify with `test_zoom_config_env_overrides_file`, `test_zoom_config_saved_owner_only`, and `test_zoom_missing_credentials_error_names_variables`.
- [x] 4.3 Implement the PKCE pair (S256), the authorize URL, the one-shot callback on `127.0.0.1:0` with a `state` check and timeout, the token exchange (Basic auth with a secret, `client_id` in the body without one), and refresh with rotation (saved before use). Verify against the mock server with `test_zoom_login_stores_tokens`, `test_zoom_login_public_client_sends_no_authorization_header`, `test_zoom_login_rejects_state_mismatch`, `test_zoom_expired_token_refreshed_and_saved`, and `test_zoom_rejected_refresh_asks_to_login`.
- [x] 4.4 Implement the REST calls: `users/me` (email), the `users/me/meeting_summaries` and `users/me/recordings` lists with paging and UTC ranges covering the local day, `meetings/{uuid}/meeting_summary`, `meetings/{uuid}/transcript` (with `can_download` and `download_restriction_reason`), the authenticated download following redirects, and double-encoded UUIDs. Errors include the HTTP status and Zoom's message. Verify with `test_zoom_meetings_merged_by_uuid_across_pages`, `test_zoom_uuid_double_encoded`, `test_zoom_transcript_not_ready_reports_reason`, and `test_zoom_api_error_includes_status_and_message`.

## 5. Commands

- [x] 5.1 Add `zoom login` and `zoom logout` to `src/cli.rs`. Verify with `test_zoom_logout_keeps_credentials_removes_tokens` and the `zoom` entries in `--help`.
- [x] 5.2 Add `zoom import --date WHEN [--parent ID] [--title] [--keywords] [--signature] [--dry-run]`: plan, print, create notes through `create_note` with `Source::Content`, and regenerate SUMMARY.md when anything was created. Verify end to end against the mock Zoom with `test_zoom_import_day_creates_summary_and_transcript_under_matched_parent`, `test_zoom_import_time_selects_one_meeting`, `test_zoom_import_twice_skips_repeats`, `test_zoom_import_dry_run_writes_nothing`, `test_zoom_import_transcript_only_reports_missing_summary`, `test_zoom_import_transcript_not_ready_imports_summary_only`, and `test_zoom_import_nothing_in_window_lists_meetings`.
- [x] 5.3 Document in the README:
  - creating the Zoom app: General app, user-managed, "Use Public Client OAuth", redirect `http://127.0.0.1/callback`;
  - the scopes `meeting:read:list_summaries`, `meeting:read:summary`, `cloud_recording:read:list_user_recordings`, `cloud_recording:read:meeting_transcript` and `user:read:user`;
  - the plan and setting requirements;
  - `ZOOM_CLIENT_ID` and the optional `ZOOM_CLIENT_SECRET`;
  - `zoom login` and `zoom import`, in the commands table and the Monday workflow.

  Verify the commands shown match `--help`.

## 6. Integration checks

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `just crap`, and verify all pass with no function above a CRAP score of 30. Also verify every new test name starts with `test_` and every test helper (functions and methods) starts with `test_helper_`.
- [ ] 6.2 Manually run `zoom login` with a public client ID (no secret), then `zoom import --date <a real meeting day> --dry-run`, then a real import against a real Zoom account, and verify the notes and SUMMARY.md. Then set `expires_at` in the past and run the import again to verify that public-client token refresh works.

## Workflow follow-up

- Archive the change with `/opsx:archive` after review.
