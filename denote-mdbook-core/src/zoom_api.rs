//! Zoom user OAuth (PKCE, loopback redirect), the token file, and the Zoom
//! REST calls `zoom import` needs.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ureq::Agent;

use crate::naming;
use crate::zoom::Meeting;

const DEFAULT_API_URL: &str = "https://api.zoom.us/v2";
const DEFAULT_OAUTH_URL: &str = "https://zoom.us";

/// How long `zoom login` waits for the browser to come back.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Refresh this many seconds before the access token expires.
const REFRESH_MARGIN_SECONDS: i64 = 60;

/// Base URLs, overridable so tests can point at a local mock.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub api: String,
    pub oauth: String,
}

impl Endpoints {
    pub fn from_env() -> Self {
        let var = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.to_string())
        };
        Self {
            api: var("AA_DENOTE_MDBOOK_TOOL_ZOOM_API_URL", DEFAULT_API_URL),
            oauth: var("AA_DENOTE_MDBOOK_TOOL_ZOOM_OAUTH_URL", DEFAULT_OAUTH_URL),
        }
    }
}

/// Contents of `zoom.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoomConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Access token expiry, Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

impl ZoomConfig {
    fn clear_tokens(&mut self) {
        self.access_token = None;
        self.refresh_token = None;
        self.expires_at = None;
    }
}

/// `<config dir>/aa-denote-mdbook-tool/zoom.toml`, where the config dir can
/// be overridden with `AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR`.
pub fn config_path() -> Result<PathBuf> {
    let base = match std::env::var_os("AA_DENOTE_MDBOOK_TOOL_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => dirs::config_dir().context("cannot find the user config directory")?,
    };
    Ok(base.join("aa-denote-mdbook-tool").join("zoom.toml"))
}

/// Read `zoom.toml`; a missing file is an empty config.
pub fn load_config(path: &Path) -> Result<ZoomConfig> {
    match fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ZoomConfig::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Write `zoom.toml` readable only by its owner, replacing it atomically.
pub fn save_config(path: &Path, config: &ZoomConfig) -> Result<()> {
    let dir = path.parent().context("config path has no parent")?;
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let text = toml::to_string(config).context("cannot serialize the Zoom config")?;
    let tmp = dir.join(".zoom.toml.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(&tmp)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    file.write_all(text.as_bytes())?;
    drop(file);
    fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))
}

/// The OAuth app's client ID and optional secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: Option<String>,
}

/// Credentials from `ZOOM_CLIENT_ID` / `ZOOM_CLIENT_SECRET`, else the config.
pub fn credentials(config: &ZoomConfig, path: &Path) -> Result<Credentials> {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let client_id = env("ZOOM_CLIENT_ID").or_else(|| config.client_id.clone());
    let Some(client_id) = client_id else {
        bail!(
            "no Zoom client ID: set ZOOM_CLIENT_ID (and ZOOM_CLIENT_SECRET if your app has one), \
             or add client_id to {}",
            path.display()
        );
    };
    let client_secret = env("ZOOM_CLIENT_SECRET").or_else(|| config.client_secret.clone());
    Ok(Credentials {
        client_id,
        client_secret,
    })
}

/// A PKCE verifier and its S256 challenge.
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// A random URL-safe string from `bytes` random bytes.
pub fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| anyhow::anyhow!("no randomness available: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub fn pkce() -> Result<Pkce> {
    // 48 bytes encode to 64 characters, within PKCE's 43 to 128.
    let verifier = random_token(48)?;
    Ok(Pkce {
        challenge: pkce_challenge(&verifier),
        verifier,
    })
}

pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn authorize_url(
    endpoints: &Endpoints,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> String {
    let query = [
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ]
    .iter()
    .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
    .collect::<Vec<_>>()
    .join("&");
    format!("{}/oauth/authorize?{query}", endpoints.oauth)
}

/// Listen on a free port of 127.0.0.1 and return the listener and its
/// redirect URI. Zoom ignores the port of a loopback redirect.
pub fn bind_loopback() -> Result<(TcpListener, String)> {
    let listener =
        TcpListener::bind("127.0.0.1:0").context("cannot listen on 127.0.0.1 for the callback")?;
    let port = listener.local_addr()?.port();
    Ok((listener, format!("http://127.0.0.1:{port}/callback")))
}

/// Wait for the browser's redirect to `/callback`, check `state`, and return
/// the authorization code. Other paths get a 404 and are ignored.
pub fn wait_for_callback(listener: &TcpListener, expected_state: &str) -> Result<String> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(result) = handle_callback(stream, expected_state)? {
                    return result;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    bail!("timed out waiting for Zoom sign-in in the browser");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e).context("callback listener failed"),
        }
    }
}

/// Answer one request. `None` means it was not the callback.
fn handle_callback(mut stream: TcpStream, expected_state: &str) -> Result<Option<Result<String>>> {
    stream.set_nonblocking(false)?;
    let mut request_line = String::new();
    BufReader::new(&stream).read_line(&mut request_line)?;
    let target = request_line.split_whitespace().nth(1).unwrap_or("");
    let Some(query) = target.strip_prefix("/callback?") else {
        respond(&mut stream, "404 Not Found", "Not found.")?;
        return Ok(None);
    };
    let params = parse_query(query);
    let outcome = callback_outcome(&params, expected_state);
    let page = match &outcome {
        Ok(_) => "Signed in to Zoom. You can close this tab.",
        Err(_) => "Zoom sign-in failed. See the terminal for details.",
    };
    respond(&mut stream, "200 OK", page)?;
    Ok(Some(outcome))
}

fn callback_outcome(params: &BTreeMap<String, String>, expected_state: &str) -> Result<String> {
    if let Some(error) = params.get("error") {
        bail!("Zoom sign-in was not approved: {error}");
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        bail!("Zoom sign-in failed: the callback's state does not match");
    }
    params
        .get("code")
        .cloned()
        .context("Zoom sign-in failed: the callback has no code")
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let html = format!("<!doctype html><title>aa-denote-mdbook-tool</title><p>{body}</p>\n");
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )?;
    Ok(())
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| {
            let decode = |s: &str| {
                urlencoding::decode(&s.replace('+', " "))
                    .map(|c| c.into_owned())
                    .unwrap_or_default()
            };
            (decode(k), decode(v))
        })
        .collect()
}

/// Try `$BROWSER`, then the platform opener; failure is fine because the URL
/// is printed too.
pub fn open_in_browser(url: &str) {
    let program = std::env::var("BROWSER")
        .ok()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "open".to_string()
            } else {
                "xdg-open".to_string()
            }
        });
    let _ = std::process::Command::new(program)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into()
}

/// POST to the token endpoint: Basic auth with a secret, otherwise
/// `client_id` in the form as a public client.
fn token_request(
    endpoints: &Endpoints,
    creds: &Credentials,
    form: &[(&str, &str)],
) -> Result<TokenResponse> {
    let mut fields: Vec<(&str, &str)> = form.to_vec();
    let mut request = agent().post(format!("{}/oauth/token", endpoints.oauth));
    match &creds.client_secret {
        Some(secret) => {
            let basic = STANDARD.encode(format!("{}:{secret}", creds.client_id));
            request = request.header("Authorization", format!("Basic {basic}"));
        }
        None => fields.push(("client_id", &creds.client_id)),
    }
    let response = request
        .send_form(fields)
        .context("cannot reach Zoom's token endpoint")?;
    read_json(response, "Zoom token request")
}

/// Exchange an authorization code for tokens and record them in `config`.
pub fn exchange_code(
    endpoints: &Endpoints,
    creds: &Credentials,
    config: &mut ZoomConfig,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<()> {
    let tokens = token_request(
        endpoints,
        creds,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ],
    )?;
    store_tokens(config, tokens)
}

fn store_tokens(config: &mut ZoomConfig, tokens: TokenResponse) -> Result<()> {
    config.access_token = Some(tokens.access_token);
    config.refresh_token = Some(tokens.refresh_token);
    config.expires_at = Some(naming::now()?.timestamp() + tokens.expires_in);
    Ok(())
}

/// Remove stored tokens, keeping any client ID and secret.
pub fn logout(path: &Path) -> Result<bool> {
    let mut config = load_config(path)?;
    let was_signed_in = config.refresh_token.is_some() || config.access_token.is_some();
    config.clear_tokens();
    if path.exists() {
        save_config(path, &config)?;
    }
    Ok(was_signed_in)
}

const LOGIN_AGAIN: &str = "run `aa-denote-mdbook-tool zoom login`";

/// A client with a valid access token, refreshing (and saving the rotated
/// tokens) first when the stored one has expired.
pub fn signed_in_client(path: &Path) -> Result<ZoomClient> {
    let mut config = load_config(path)?;
    let creds = credentials(&config, path)?;
    let endpoints = Endpoints::from_env();
    let (Some(access_token), Some(refresh_token)) =
        (config.access_token.clone(), config.refresh_token.clone())
    else {
        bail!("not signed in to Zoom: {LOGIN_AGAIN}");
    };
    let now = naming::now()?.timestamp();
    let expired = config
        .expires_at
        .is_none_or(|t| t - REFRESH_MARGIN_SECONDS <= now);
    let access_token = if expired {
        let tokens = token_request(
            &endpoints,
            &creds,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
            ],
        )
        .with_context(|| format!("Zoom sign-in expired or was revoked; {LOGIN_AGAIN}"))?;
        store_tokens(&mut config, tokens)?;
        save_config(path, &config)?;
        config.access_token.clone().unwrap_or_default()
    } else {
        access_token
    };
    Ok(ZoomClient {
        agent: agent(),
        endpoints,
        access_token,
    })
}

/// Read a JSON body, or turn an error status into Zoom's message.
fn read_json<T: serde::de::DeserializeOwned>(
    mut response: ureq::http::Response<ureq::Body>,
    what: &str,
) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        bail!(
            "{what} failed: HTTP {}: {}",
            status.as_u16(),
            zoom_message(&body)
        );
    }
    response
        .body_mut()
        .read_json()
        .with_context(|| format!("{what} returned unexpected JSON"))
}

/// Zoom's `message` (and `reason` for OAuth errors) from an error body.
fn zoom_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return body.trim().to_string();
    };
    ["message", "reason", "error"]
        .iter()
        .find_map(|k| value.get(k).and_then(Value::as_str))
        .unwrap_or(body.trim())
        .to_string()
}

/// A meeting UUID ready for a URL path: encoded once, or twice when it
/// starts with `/` or contains `//`, as Zoom requires.
pub fn encode_uuid(uuid: &str) -> String {
    let once = urlencoding::encode(uuid).into_owned();
    if uuid.starts_with('/') || uuid.contains("//") {
        urlencoding::encode(&once).into_owned()
    } else {
        once
    }
}

/// Whether a transcript could be fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptOutcome {
    Text(String),
    /// Zoom's `download_restriction_reason`, or its error message.
    Unavailable(String),
}

pub struct ZoomClient {
    agent: Agent,
    endpoints: Endpoints,
    access_token: String,
}

impl ZoomClient {
    fn get(&self, path_and_query: &str) -> Result<ureq::http::Response<ureq::Body>> {
        self.get_url(&format!("{}{path_and_query}", self.endpoints.api))
    }

    fn get_url(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>> {
        self.agent
            .get(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .call()
            .with_context(|| format!("cannot reach Zoom ({url})"))
    }

    /// The signed-in user's email.
    pub fn me_email(&self) -> Result<String> {
        let me: Value = read_json(self.get("/users/me")?, "GET /users/me")?;
        Ok(me
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or("unknown email")
            .to_string())
    }

    /// Every page of a list endpoint; `query` excludes the page token.
    fn pages(&self, path: &str, query: &str) -> Result<Vec<Value>> {
        let mut pages = Vec::new();
        let mut token = String::new();
        loop {
            let mut url = format!("{path}?page_size=300&{query}");
            if !token.is_empty() {
                url.push_str(&format!("&next_page_token={}", urlencoding::encode(&token)));
            }
            let page: Value = read_json(self.get(&url)?, &format!("GET {path}"))?;
            token = page
                .get("next_page_token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            pages.push(page);
            if token.is_empty() {
                return Ok(pages);
            }
        }
    }

    /// Meetings that started on local `day` with an AI summary or a cloud
    /// recording transcript, merged by UUID.
    pub fn meetings_on(&self, day: NaiveDate) -> Result<Vec<Meeting>> {
        let (from, to) = utc_bounds(day)?;
        let mut meetings: BTreeMap<String, Meeting> = BTreeMap::new();
        let summary_query = format!(
            "from={}&to={}&time_filter_field=summary_start_time",
            from.format("%Y-%m-%dT%H:%M:%SZ"),
            to.format("%Y-%m-%dT%H:%M:%SZ")
        );
        for page in self.pages("/users/me/meeting_summaries", &summary_query)? {
            for item in array(&page, "summaries") {
                merge_summary_item(&mut meetings, item);
            }
        }
        let recording_query = format!(
            "from={}&to={}",
            from.format("%Y-%m-%d"),
            to.format("%Y-%m-%d")
        );
        for page in self.pages("/users/me/recordings", &recording_query)? {
            for item in array(&page, "meetings") {
                merge_recording_item(&mut meetings, item);
            }
        }
        Ok(meetings
            .into_values()
            .filter(|m| m.start.date() == day)
            .collect())
    }

    /// The meeting summary JSON.
    pub fn summary(&self, uuid: &str) -> Result<Value> {
        let path = format!("/meetings/{}/meeting_summary", encode_uuid(uuid));
        read_json(self.get(&path)?, &format!("GET {path}"))
    }

    /// The transcript VTT, or why Zoom cannot provide it.
    pub fn transcript(&self, uuid: &str) -> Result<TranscriptOutcome> {
        let path = format!("/meetings/{}/transcript", encode_uuid(uuid));
        let response = self.get(&path)?;
        if response.status().as_u16() == 404 {
            let mut response = response;
            let body = response.body_mut().read_to_string().unwrap_or_default();
            return Ok(TranscriptOutcome::Unavailable(zoom_message(&body)));
        }
        let info: Value = read_json(response, &format!("GET {path}"))?;
        let download_url = info.get("download_url").and_then(Value::as_str);
        let can_download = info.get("can_download").and_then(Value::as_bool) == Some(true);
        match (can_download, download_url) {
            (true, Some(url)) => {
                let mut response = self.get_url(url)?;
                let status = response.status();
                let body = response.body_mut().read_to_string()?;
                if !status.is_success() {
                    bail!(
                        "transcript download failed: HTTP {}: {}",
                        status.as_u16(),
                        zoom_message(&body)
                    );
                }
                Ok(TranscriptOutcome::Text(body))
            }
            _ => Ok(TranscriptOutcome::Unavailable(
                info.get("download_restriction_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("NO_TRANSCRIPT_DATA")
                    .to_string(),
            )),
        }
    }
}

fn array<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// UTC instants of local midnight at the start and end of `day`.
fn utc_bounds(day: NaiveDate) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let to_utc = |date: NaiveDate| -> Result<DateTime<Utc>> {
        Local
            .from_local_datetime(&date.and_time(chrono::NaiveTime::MIN))
            .earliest()
            .map(|t| t.with_timezone(&Utc))
            .with_context(|| format!("{date} has no local midnight"))
    };
    let next = day.succ_opt().context("date out of range")?;
    Ok((to_utc(day)?, to_utc(next)?))
}

/// Parse a Zoom UTC timestamp into local time.
pub fn zoom_time_to_local(value: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.with_timezone(&Local).naive_local())
}

fn merge_summary_item(meetings: &mut BTreeMap<String, Meeting>, item: &Value) {
    let uuid = text(item, "meeting_uuid");
    let Some(start) = zoom_time_to_local(text(item, "meeting_start_time")) else {
        return;
    };
    let end = zoom_time_to_local(text(item, "meeting_end_time")).unwrap_or(start);
    let meeting = meetings
        .entry(uuid.to_string())
        .or_insert_with(|| new_meeting(uuid, text(item, "meeting_topic"), start, end));
    meeting.has_summary = true;
}

fn merge_recording_item(meetings: &mut BTreeMap<String, Meeting>, item: &Value) {
    let uuid = text(item, "uuid");
    let Some(start) = zoom_time_to_local(text(item, "start_time")) else {
        return;
    };
    let minutes = item.get("duration").and_then(Value::as_i64).unwrap_or(0);
    let end = start + TimeDelta::minutes(minutes);
    let has_transcript =
        array(item, "recording_files").any(|f| text(f, "file_type") == "TRANSCRIPT");
    let meeting = meetings
        .entry(uuid.to_string())
        .or_insert_with(|| new_meeting(uuid, text(item, "topic"), start, end));
    meeting.has_transcript |= has_transcript;
}

fn new_meeting(uuid: &str, topic: &str, start: NaiveDateTime, end: NaiveDateTime) -> Meeting {
    Meeting {
        uuid: uuid.to_string(),
        topic: topic.to_string(),
        start,
        end,
        has_summary: false,
        has_transcript: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pkce_challenge_matches_rfc_7636_example() {
        // RFC 7636 appendix B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn test_pkce_verifier_length_in_range() {
        let pair = pkce().unwrap();
        assert!((43..=128).contains(&pair.verifier.len()));
        assert_ne!(pair.verifier, pkce().unwrap().verifier);
    }

    #[test]
    fn test_zoom_uuid_double_encoded() {
        assert_eq!(
            encode_uuid("aDYlohsHRtCd4ii1uC2+hA=="),
            "aDYlohsHRtCd4ii1uC2%2BhA%3D%3D"
        );
        assert_eq!(
            encode_uuid("/ajXp112QmuoKj4854875=="),
            "%252FajXp112QmuoKj4854875%253D%253D"
        );
        assert_eq!(encode_uuid("ab//cd"), "ab%252F%252Fcd");
    }

    #[test]
    fn test_authorize_url_has_pkce_and_state() {
        let endpoints = Endpoints {
            api: "http://api".into(),
            oauth: "https://zoom.us".into(),
        };
        let url = authorize_url(
            &endpoints,
            "cid",
            "http://127.0.0.1:5000/callback",
            "st",
            "ch",
        );
        assert_eq!(
            url,
            "https://zoom.us/oauth/authorize?response_type=code&client_id=cid\
             &redirect_uri=http%3A%2F%2F127.0.0.1%3A5000%2Fcallback&state=st\
             &code_challenge=ch&code_challenge_method=S256"
        );
    }

    #[test]
    fn test_callback_outcome_checks_state_and_error() {
        let params = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        assert_eq!(
            callback_outcome(&params(&[("code", "c"), ("state", "s")]), "s").unwrap(),
            "c"
        );
        assert!(callback_outcome(&params(&[("code", "c"), ("state", "x")]), "s").is_err());
        assert!(callback_outcome(&params(&[("error", "access_denied")]), "s").is_err());
    }

    #[test]
    fn test_parse_query_decodes() {
        let q = parse_query("code=a%2Bb&state=x+y");
        assert_eq!(q["code"], "a+b");
        assert_eq!(q["state"], "x y");
    }

    #[test]
    fn test_zoom_message_prefers_message_field() {
        assert_eq!(
            zoom_message(
                r#"{"code":4711,"message":"Invalid access token, does not contain scopes:[meeting:read:summary]."}"#
            ),
            "Invalid access token, does not contain scopes:[meeting:read:summary]."
        );
        assert_eq!(
            zoom_message(r#"{"reason":"Invalid Token!","error":"invalid_request"}"#),
            "Invalid Token!"
        );
        assert_eq!(zoom_message("plain text\n"), "plain text");
    }

    #[test]
    fn test_merge_marks_summary_and_transcript_by_uuid() {
        let mut meetings = BTreeMap::new();
        merge_summary_item(
            &mut meetings,
            &json!({"meeting_uuid": "u1", "meeting_topic": "Sync",
                    "meeting_start_time": "2026-10-10T15:01:00Z",
                    "meeting_end_time": "2026-10-10T15:45:00Z"}),
        );
        merge_recording_item(
            &mut meetings,
            &json!({"uuid": "u1", "topic": "Sync", "start_time": "2026-10-10T15:01:00Z",
                    "duration": 44, "recording_files": [{"file_type": "MP4"}, {"file_type": "TRANSCRIPT"}]}),
        );
        merge_recording_item(
            &mut meetings,
            &json!({"uuid": "u2", "topic": "Video only", "start_time": "2026-10-10T16:00:00Z",
                    "duration": 30, "recording_files": [{"file_type": "MP4"}]}),
        );
        let u1 = &meetings["u1"];
        assert!(u1.has_summary && u1.has_transcript);
        assert_eq!(u1.end - u1.start, TimeDelta::minutes(44));
        assert!(!meetings["u2"].has_summary && !meetings["u2"].has_transcript);
    }
}
