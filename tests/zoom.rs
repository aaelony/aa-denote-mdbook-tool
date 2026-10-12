//! zoom-import spec, against a local mock of Zoom's OAuth and REST API.

mod common;

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use common::*;
use serde_json::{Value, json};
use tempfile::TempDir;

/// One request the mock received.
#[derive(Debug, Clone)]
struct RecordedRequest {
    method: String,
    path: String,
    query: String,
    authorization: Option<String>,
    body: String,
}

#[derive(Default)]
struct MockState {
    /// Responses per path; the last one repeats once the others are used.
    routes: HashMap<String, VecDeque<(u16, String)>>,
    requests: Vec<RecordedRequest>,
}

/// A local HTTP server standing in for zoom.us and api.zoom.us.
struct MockZoom {
    base: String,
    state: Arc<Mutex<MockState>>,
    server: Arc<tiny_http::Server>,
}

impl MockZoom {
    /// Respond to `path` (no query) with `status` and `body`; repeated calls
    /// queue responses for successive requests.
    fn test_helper_route(&self, path: &str, status: u16, body: Value) {
        let text = match body {
            Value::String(s) => s,
            other => other.to_string(),
        };
        self.state
            .lock()
            .unwrap()
            .routes
            .entry(path.to_string())
            .or_default()
            .push_back((status, text));
    }

    fn test_helper_requests(&self) -> Vec<RecordedRequest> {
        self.state.lock().unwrap().requests.clone()
    }

    fn test_helper_requests_to(&self, path: &str) -> Vec<RecordedRequest> {
        self.test_helper_requests()
            .into_iter()
            .filter(|r| r.path == path)
            .collect()
    }
}

impl Drop for MockZoom {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

fn test_helper_mock_zoom() -> MockZoom {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
    let port = server.server_addr().to_ip().unwrap().port();
    let state = Arc::new(Mutex::new(MockState::default()));
    let (thread_server, thread_state) = (Arc::clone(&server), Arc::clone(&state));
    std::thread::spawn(move || {
        for mut request in thread_server.incoming_requests() {
            let (path, query) = match request.url().split_once('?') {
                Some((p, q)) => (p.to_string(), q.to_string()),
                None => (request.url().to_string(), String::new()),
            };
            let authorization = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .map(|h| h.value.to_string());
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let (status, text) = {
                let mut state = thread_state.lock().unwrap();
                state.requests.push(RecordedRequest {
                    method: request.method().to_string(),
                    path: path.clone(),
                    query,
                    authorization,
                    body,
                });
                match state.routes.get_mut(&path) {
                    Some(queue) if queue.len() > 1 => queue.pop_front().unwrap(),
                    Some(queue) if !queue.is_empty() => queue[0].clone(),
                    _ => (
                        404,
                        json!({"code": 404, "message": "not mocked"}).to_string(),
                    ),
                }
            };
            let response = tiny_http::Response::from_string(text)
                .with_status_code(status)
                .with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                );
            let _ = request.respond(response);
        }
    });
    MockZoom {
        base: format!("http://127.0.0.1:{port}"),
        state,
        server,
    }
}

/// A command for `root` at 2026-10-10 12:00 local (UTC-7), with Zoom's URLs
/// pointing at `mock` and the config in `config_dir`.
fn test_helper_zoom_cli(root: &Path, mock: &MockZoom, config_dir: &Path) -> assert_cmd::Command {
    let mut c = test_helper_cli_at_time(root, "2026-10-10", "12:00:00");
    c.env("AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR", config_dir)
        .env(
            "AA_DENOTE_MDBOOK_TOOL_ZOOM_API_URL",
            format!("{}/v2", mock.base),
        )
        .env("AA_DENOTE_MDBOOK_TOOL_ZOOM_OAUTH_URL", &mock.base)
        .env("BROWSER", "true")
        .env_remove("ZOOM_CLIENT_ID")
        .env_remove("ZOOM_CLIENT_SECRET");
    c
}

fn test_helper_config_file(config_dir: &Path) -> PathBuf {
    config_dir.join("aa-denote-mdbook-tool/zoom.toml")
}

/// A signed-in config whose access token expires at Unix time `expires_at`.
fn test_helper_write_signed_in_config(config_dir: &Path, expires_at: i64) {
    let path = test_helper_config_file(config_dir);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "client_id = \"cid\"\naccess_token = \"at-old\"\nrefresh_token = \"rt-old\"\n\
             expires_at = {expires_at}\n"
        ),
    )
    .unwrap();
}

/// Far in the future, so no refresh happens.
const NEVER_EXPIRES: i64 = 4_102_444_800;

/// 2026-10-10: "Acme Sync" 08:01-08:45 local with summary and transcript,
/// and "Beta Review" 09:00-09:30 local with a summary only.
fn test_helper_mock_meeting_day(mock: &MockZoom) {
    mock.test_helper_route(
        "/v2/users/me/meeting_summaries",
        200,
        json!({"summaries": [
            {"meeting_uuid": "acme+uuid==", "meeting_topic": "Acme Sync",
             "meeting_start_time": "2026-10-10T15:01:00Z", "meeting_end_time": "2026-10-10T15:45:00Z"},
            {"meeting_uuid": "beta-uuid", "meeting_topic": "Beta Review",
             "meeting_start_time": "2026-10-10T16:00:00Z", "meeting_end_time": "2026-10-10T16:30:00Z"}
        ], "next_page_token": ""}),
    );
    mock.test_helper_route(
        "/v2/users/me/recordings",
        200,
        json!({"meetings": [
            {"uuid": "acme+uuid==", "topic": "Acme Sync", "start_time": "2026-10-10T15:01:00Z",
             "duration": 44, "recording_files": [{"file_type": "MP4"}, {"file_type": "TRANSCRIPT"}]}
        ], "next_page_token": ""}),
    );
    mock.test_helper_route(
        "/v2/meetings/acme%2Buuid%3D%3D/meeting_summary",
        200,
        json!({"summary_content": "## Quick recap\n\nBudget approved. See https://example.com/plan"}),
    );
    mock.test_helper_route(
        "/v2/meetings/beta-uuid/meeting_summary",
        200,
        json!({"summary_overview": "Beta is on track.", "next_steps": ["Ship beta"]}),
    );
    mock.test_helper_route(
        "/v2/meetings/acme%2Buuid%3D%3D/transcript",
        200,
        json!({"can_download": true, "download_url": format!("{}/rec/download/acme", mock.base)}),
    );
    mock.test_helper_route(
        "/rec/download/acme",
        200,
        Value::String(
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nhttps://example.com said hi\n".into(),
        ),
    );
}

fn test_helper_stdout_of(c: &mut assert_cmd::Command) -> String {
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn test_helper_stderr_of_failure(c: &mut assert_cmd::Command) -> String {
    let out = c.output().unwrap();
    assert!(!out.status.success(), "unexpected success");
    String::from_utf8(out.stderr).unwrap()
}

fn test_helper_decode_percent(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn test_helper_query_param(url: &str, key: &str) -> String {
    let query = url.split_once('?').unwrap().1;
    query
        .split('&')
        .filter_map(|p| p.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| test_helper_decode_percent(v))
        .unwrap()
}

/// Send the browser's redirect to the login command's callback.
fn test_helper_send_callback(redirect_uri: &str, code: &str, state: &str) -> String {
    let rest = redirect_uri.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    write!(
        stream,
        "GET /{path}?code={code}&state={state} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// Run `zoom login`, answer its callback with `state_override` (or the real
/// state), and return (success, stdout, stderr, authorize URL).
fn test_helper_run_login(
    mock: &MockZoom,
    config_dir: &Path,
    envs: &[(&str, &str)],
    state_override: Option<&str>,
) -> (bool, String, String, String) {
    let tmp = TempDir::new().unwrap();
    let mut child = Command::new(BIN)
        .args(["zoom", "login"])
        .current_dir(tmp.path())
        .env("TZ", "Etc/GMT+7")
        .env("AA_DENOTE_MDBOOK_TOOL_NOW", "2026-10-10T12:00:00-07:00")
        .env("AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR", config_dir)
        .env(
            "AA_DENOTE_MDBOOK_TOOL_ZOOM_API_URL",
            format!("{}/v2", mock.base),
        )
        .env("AA_DENOTE_MDBOOK_TOOL_ZOOM_OAUTH_URL", &mock.base)
        .env("BROWSER", "true")
        .env_remove("ZOOM_CLIENT_ID")
        .env_remove("ZOOM_CLIENT_SECRET")
        .envs(envs.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut printed = String::new();
    let url = loop {
        let mut line = String::new();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "login printed no URL"
        );
        printed.push_str(&line);
        if line.contains("/oauth/authorize?") {
            break line.trim().to_string();
        }
    };
    let redirect_uri = test_helper_query_param(&url, "redirect_uri");
    let state = state_override
        .map(str::to_string)
        .unwrap_or_else(|| test_helper_query_param(&url, "state"));
    test_helper_send_callback(&redirect_uri, "the-code", &state);
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    printed.push_str(&rest);
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let ok = child.wait().unwrap().success();
    (ok, printed, stderr, url)
}

fn test_helper_mock_token_and_me(mock: &MockZoom) {
    mock.test_helper_route(
        "/oauth/token",
        200,
        json!({"access_token": "at-new", "refresh_token": "rt-new", "expires_in": 3600, "token_type": "bearer"}),
    );
    mock.test_helper_route("/v2/users/me", 200, json!({"email": "dana@example.com"}));
}

#[test]
fn test_zoom_listed_in_help() {
    let out = test_helper_run_ok_stdout(
        test_helper_isolated_cli(Path::new(".")).args(["zoom", "--help"]),
    );
    for line in [
        "login   Sign in to Zoom in the browser and store the tokens",
        "logout  Remove the stored Zoom tokens",
        "import  Import Zoom AI summaries and transcripts by meeting date and time",
    ] {
        assert!(out.contains(line), "{out}");
    }
}

#[test]
fn test_zoom_login_stores_tokens() {
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    let config_dir = TempDir::new().unwrap();
    let (ok, stdout, stderr, url) = test_helper_run_login(
        &mock,
        config_dir.path(),
        &[("ZOOM_CLIENT_ID", "cid"), ("ZOOM_CLIENT_SECRET", "secret")],
        None,
    );
    assert!(ok, "{stderr}");
    assert!(
        stdout.contains("Signed in to Zoom as dana@example.com"),
        "{stdout}"
    );
    assert!(url.starts_with(&format!(
        "{}/oauth/authorize?response_type=code&client_id=cid",
        mock.base
    )));
    assert_eq!(
        test_helper_query_param(&url, "code_challenge_method"),
        "S256"
    );
    let redirect = test_helper_query_param(&url, "redirect_uri");
    assert!(redirect.starts_with("http://127.0.0.1:") && redirect.ends_with("/callback"));

    let token_request = &mock.test_helper_requests_to("/oauth/token")[0];
    assert_eq!(token_request.method, "POST");
    // "cid:secret" in Base64.
    assert_eq!(
        token_request.authorization.as_deref(),
        Some("Basic Y2lkOnNlY3JldA==")
    );
    assert!(token_request.body.contains("grant_type=authorization_code"));
    assert!(token_request.body.contains("code=the-code"));
    assert!(token_request.body.contains("code_verifier="));

    let config = fs::read_to_string(test_helper_config_file(config_dir.path())).unwrap();
    assert!(config.contains("access_token = \"at-new\""), "{config}");
    assert!(config.contains("refresh_token = \"rt-new\""), "{config}");
    assert!(!stdout.contains("at-new") && !stdout.contains("secret") && !stderr.contains("at-new"));
}

#[test]
fn test_zoom_config_saved_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    let config_dir = TempDir::new().unwrap();
    let (ok, _, stderr, _) =
        test_helper_run_login(&mock, config_dir.path(), &[("ZOOM_CLIENT_ID", "cid")], None);
    assert!(ok, "{stderr}");
    let mode = fs::metadata(test_helper_config_file(config_dir.path()))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn test_zoom_login_public_client_sends_no_authorization_header() {
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    let config_dir = TempDir::new().unwrap();
    let (ok, _, stderr, _) = test_helper_run_login(
        &mock,
        config_dir.path(),
        &[("ZOOM_CLIENT_ID", "public-id")],
        None,
    );
    assert!(ok, "{stderr}");
    let token_request = &mock.test_helper_requests_to("/oauth/token")[0];
    assert_eq!(token_request.authorization, None);
    assert!(
        token_request.body.contains("client_id=public-id"),
        "{}",
        token_request.body
    );
    assert!(token_request.body.contains("code_verifier="));
}

#[test]
fn test_zoom_config_env_overrides_file() {
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    let config_dir = TempDir::new().unwrap();
    let path = test_helper_config_file(config_dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "client_id = \"file-id\"\n").unwrap();
    let (ok, _, stderr, url) = test_helper_run_login(
        &mock,
        config_dir.path(),
        &[("ZOOM_CLIENT_ID", "env-id")],
        None,
    );
    assert!(ok, "{stderr}");
    assert_eq!(test_helper_query_param(&url, "client_id"), "env-id");
    // The file's client ID is kept.
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("client_id = \"file-id\"")
    );
}

#[test]
fn test_zoom_login_rejects_state_mismatch() {
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    let config_dir = TempDir::new().unwrap();
    let (ok, _, stderr, _) = test_helper_run_login(
        &mock,
        config_dir.path(),
        &[("ZOOM_CLIENT_ID", "cid")],
        Some("forged"),
    );
    assert!(!ok);
    assert!(stderr.contains("state does not match"), "{stderr}");
    assert!(mock.test_helper_requests_to("/oauth/token").is_empty());
    assert!(!test_helper_config_file(config_dir.path()).exists());
}

#[test]
fn test_zoom_missing_credentials_error_names_variables() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    let config_dir = TempDir::new().unwrap();
    for args in [
        vec!["zoom", "login"],
        vec!["zoom", "import", "--date", "2026-10-10"],
    ] {
        let stderr = test_helper_stderr_of_failure(
            test_helper_zoom_cli(&root, &mock, config_dir.path()).args(&args),
        );
        assert!(stderr.contains("ZOOM_CLIENT_ID"), "{stderr}");
        assert!(
            stderr.contains("aa-denote-mdbook-tool/zoom.toml"),
            "{stderr}"
        );
    }
}

#[test]
fn test_zoom_logout_keeps_credentials_removes_tokens() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args(["zoom", "logout"]),
    );
    assert_eq!(out, "Signed out of Zoom\n");
    let config = fs::read_to_string(test_helper_config_file(config_dir.path())).unwrap();
    assert_eq!(config, "client_id = \"cid\"\n");
    let again = test_helper_stdout_of(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args(["zoom", "logout"]),
    );
    assert_eq!(again, "Not signed in to Zoom\n");
}

#[test]
fn test_zoom_expired_token_refreshed_and_saved() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    test_helper_mock_token_and_me(&mock);
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), 0);
    test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10",
        "--dry-run",
    ]));
    let refresh = &mock.test_helper_requests_to("/oauth/token")[0];
    assert!(refresh.body.contains("grant_type=refresh_token"));
    assert!(refresh.body.contains("refresh_token=rt-old"));
    assert!(refresh.body.contains("client_id=cid"));
    let listed = &mock.test_helper_requests_to("/v2/users/me/meeting_summaries")[0];
    assert_eq!(listed.authorization.as_deref(), Some("Bearer at-new"));
    let config = fs::read_to_string(test_helper_config_file(config_dir.path())).unwrap();
    assert!(config.contains("refresh_token = \"rt-new\""), "{config}");
}

#[test]
fn test_zoom_rejected_refresh_asks_to_login() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/oauth/token",
        400,
        json!({"reason": "Invalid Token!", "error": "invalid_request"}),
    );
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), 0);
    let stderr = test_helper_stderr_of_failure(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
            "zoom",
            "import",
            "--date",
            "2026-10-10",
        ]),
    );
    assert!(
        stderr.contains("run `aa-denote-mdbook-tool zoom login`"),
        "{stderr}"
    );
    assert!(stderr.contains("Invalid Token!"), "{stderr}");
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}

#[test]
fn test_zoom_meetings_merged_by_uuid_across_pages() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/v2/users/me/meeting_summaries",
        200,
        json!({"summaries": [
            {"meeting_uuid": "u1", "meeting_topic": "First",
             "meeting_start_time": "2026-10-10T15:00:00Z", "meeting_end_time": "2026-10-10T15:30:00Z"}
        ], "next_page_token": "page-2"}),
    );
    mock.test_helper_route(
        "/v2/users/me/meeting_summaries",
        200,
        json!({"summaries": [
            {"meeting_uuid": "u2", "meeting_topic": "Second",
             "meeting_start_time": "2026-10-10T17:00:00Z", "meeting_end_time": "2026-10-10T17:30:00Z"}
        ], "next_page_token": ""}),
    );
    mock.test_helper_route(
        "/v2/users/me/recordings",
        200,
        json!({"meetings": [
            {"uuid": "u1", "topic": "First", "start_time": "2026-10-10T15:00:00Z", "duration": 30,
             "recording_files": [{"file_type": "TRANSCRIPT"}]}
        ]}),
    );
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10",
        "--dry-run",
    ]));
    assert_eq!(
        out,
        "2026-10-10 08:00 First -> with no parent\n  summary: would import\n  transcript: would import\n\
         2026-10-10 10:00 Second -> with no parent\n  summary: would import\n  transcript: not available\n"
    );
    let pages = mock.test_helper_requests_to("/v2/users/me/meeting_summaries");
    assert_eq!(pages.len(), 2);
    assert!(
        pages[0]
            .query
            .contains("from=2026-10-10T07:00:00Z&to=2026-10-11T07:00:00Z")
    );
    assert!(
        pages[0]
            .query
            .contains("time_filter_field=summary_start_time")
    );
    assert!(pages[1].query.contains("next_page_token=page-2"));
    let recordings = &mock.test_helper_requests_to("/v2/users/me/recordings")[0];
    assert!(recordings.query.contains("from=2026-10-10&to=2026-10-11"));
}

#[test]
fn test_zoom_api_error_includes_status_and_message() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/v2/users/me/meeting_summaries",
        400,
        json!({"code": 4711, "message": "Invalid access token, does not contain scopes:[meeting:read:list_summaries]."}),
    );
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let stderr = test_helper_stderr_of_failure(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
            "zoom",
            "import",
            "--date",
            "2026-10-10",
        ]),
    );
    assert!(
        stderr.contains("GET /users/me/meeting_summaries failed: HTTP 400"),
        "{stderr}"
    );
    assert!(
        stderr.contains("does not contain scopes:[meeting:read:list_summaries]"),
        "{stderr}"
    );
}

#[test]
fn test_zoom_import_day_creates_summary_and_transcript_under_matched_parent() {
    let (_t, root) = test_helper_init_notebook();
    let parent = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme,meeting");
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10",
    ]));
    let summary = "src/notes/zoom-summaries/20261010T080001--acme-sync-ai-summary__acme_meeting_zoomsummary.md";
    let transcript = "src/notes/zoom-transcripts/20261010T080002--acme-sync-transcript__acme_meeting_zoomtranscript.md";
    let beta = "src/notes/zoom-summaries/20261010T090000--beta-review-ai-summary.md";
    assert_eq!(
        out,
        format!(
            "2026-10-10 08:01 Acme Sync -> under 20261010T080000 (Acme Sync)\n  summary: {summary}\n  transcript: {transcript}\n\
             2026-10-10 09:00 Beta Review -> with no parent\n  summary: {beta}\n  transcript: not available\n"
        )
    );
    let summary_note = test_helper_read_file(root.join(summary));
    assert!(summary_note.contains("parent = \"20261010T080000\"\n"));
    assert!(summary_note.contains("zoom_meeting = \"acme+uuid==\"\n"));
    assert!(summary_note.contains("## Quick recap\n\nBudget approved. See [https://example.com/plan](https://example.com/plan)\n"));
    let transcript_note = test_helper_read_file(root.join(transcript));
    assert!(transcript_note.contains("```text\nWEBVTT\n"));
    assert!(transcript_note.contains("\nhttps://example.com said hi\n"));
    let beta_note = test_helper_read_file(root.join(beta));
    assert!(
        beta_note.contains("## Overview\n\nBeta is on track.\n\n## Next steps\n\n- Ship beta\n")
    );
    let parent_note = test_helper_read_file(root.join(parent));
    assert!(parent_note.contains("## Attachments\n"));
    let summary_md = test_helper_read_file(root.join("src/SUMMARY.md"));
    assert!(
        summary_md.contains("    - [Acme Sync AI Summary](notes/zoom-summaries/20261010T080001--"),
        "{summary_md}"
    );
    let transcript_request = &mock.test_helper_requests_to("/rec/download/acme")[0];
    assert_eq!(
        transcript_request.authorization.as_deref(),
        Some("Bearer at-old")
    );
}

#[test]
fn test_zoom_import_time_selects_one_meeting() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10 08:00",
    ]));
    assert!(
        out.starts_with("2026-10-10 08:01 Acme Sync -> with no parent\n"),
        "{out}"
    );
    assert!(!out.contains("Beta Review"), "{out}");
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")),
        ["20261010T080100--acme-sync-ai-summary.md"]
    );
}

#[test]
fn test_zoom_import_twice_skips_repeats() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let import = || {
        test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
            "zoom",
            "import",
            "--date",
            "2026-10-10",
        ]))
    };
    import();
    let second = import();
    assert_eq!(
        second,
        "2026-10-10 08:01 Acme Sync -> with no parent\n  summary: already imported\n  transcript: already imported\n\
         2026-10-10 09:00 Beta Review -> with no parent\n  summary: already imported\n  transcript: not available\n"
    );
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).len(),
        2
    );
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes/zoom-transcripts")).len(),
        1
    );
}

#[test]
fn test_zoom_import_dry_run_writes_nothing() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let summary_before = test_helper_read_file(root.join("src/SUMMARY.md"));
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10",
        "--dry-run",
    ]));
    assert!(out.starts_with(
        "2026-10-10 08:01 Acme Sync -> under 20261010T080000 (Acme Sync)\n  summary: would import\n  transcript: would import\n"
    ), "{out}");
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-transcripts")).is_empty());
    assert_eq!(
        test_helper_read_file(root.join("src/SUMMARY.md")),
        summary_before
    );
    assert!(
        mock.test_helper_requests_to("/rec/download/acme")
            .is_empty()
    );
}

#[test]
fn test_zoom_import_transcript_only_reports_missing_summary() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/v2/users/me/meeting_summaries",
        200,
        json!({"summaries": []}),
    );
    mock.test_helper_route(
        "/v2/users/me/recordings",
        200,
        json!({"meetings": [{"uuid": "t-only", "topic": "Standup", "start_time": "2026-10-10T16:00:00Z",
                             "duration": 15, "recording_files": [{"file_type": "TRANSCRIPT"}]}]}),
    );
    mock.test_helper_route(
        "/v2/meetings/t-only/transcript",
        200,
        json!({"can_download": true, "download_url": format!("{}/rec/download/t-only", mock.base)}),
    );
    mock.test_helper_route(
        "/rec/download/t-only",
        200,
        Value::String("WEBVTT\n".into()),
    );
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10",
    ]));
    assert_eq!(
        out,
        "2026-10-10 09:00 Standup -> with no parent\n  summary: not available\n  \
         transcript: src/notes/zoom-transcripts/20261010T090000--standup-transcript.md\n"
    );
}

#[test]
fn test_zoom_import_transcript_not_ready_imports_summary_only() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/v2/meetings/acme%2Buuid%3D%3D/transcript",
        200,
        json!({"can_download": false, "download_restriction_reason": "NOT_READY"}),
    );
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10 08:00",
    ]));
    assert_eq!(
        out,
        "2026-10-10 08:01 Acme Sync -> with no parent\n  \
         summary: src/notes/zoom-summaries/20261010T080100--acme-sync-ai-summary.md\n  \
         transcript: not available (NOT_READY)\n"
    );
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-transcripts")).is_empty());
}

#[test]
fn test_zoom_transcript_not_ready_reports_reason() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    mock.test_helper_route(
        "/v2/meetings/acme%2Buuid%3D%3D/transcript",
        200,
        json!({"can_download": false, "download_restriction_reason": "NO_TRANSCRIPT_DATA"}),
    );
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let out = test_helper_stdout_of(test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
        "zoom",
        "import",
        "--date",
        "2026-10-10 08:01",
    ]));
    assert!(
        out.ends_with("  transcript: not available (NO_TRANSCRIPT_DATA)\n"),
        "{out}"
    );
}

#[test]
fn test_zoom_import_nothing_in_window_lists_meetings() {
    let (_t, root) = test_helper_init_notebook();
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let stderr = test_helper_stderr_of_failure(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
            "zoom",
            "import",
            "--date",
            "2026-10-10 11:00",
        ]),
    );
    assert!(
        stderr.contains(
            "no Zoom meeting with a summary or transcript matches --date \"2026-10-10 11:00\"; \
             that day has:\n  08:01 Acme Sync\n  09:00 Beta Review"
        ),
        "{stderr}"
    );
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}

#[test]
fn test_zoom_import_parent_with_whole_day_rejected() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let mock = test_helper_mock_zoom();
    test_helper_mock_meeting_day(&mock);
    let config_dir = TempDir::new().unwrap();
    test_helper_write_signed_in_config(config_dir.path(), NEVER_EXPIRES);
    let stderr = test_helper_stderr_of_failure(
        test_helper_zoom_cli(&root, &mock, config_dir.path()).args([
            "zoom",
            "import",
            "--date",
            "2026-10-10",
            "--parent",
            "20261010T080000",
        ]),
    );
    assert!(
        stderr.contains("--parent needs exactly one meeting"),
        "{stderr}"
    );
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}
