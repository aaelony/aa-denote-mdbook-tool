//! `zoom login`, `zoom logout`, and `zoom import`.

use std::process::ExitCode;

use anyhow::{Result, bail};

use aa_denote_mdbook_tool_core::capture::{self, NoteOptions, Source};
use aa_denote_mdbook_tool_core::frontmatter::Kind;
use aa_denote_mdbook_tool_core::notebook::Notebook;
use aa_denote_mdbook_tool_core::zoom::{self, Action, Meeting, PlannedMeeting};
use aa_denote_mdbook_tool_core::zoom_api::{self, TranscriptOutcome, ZoomClient};
use aa_denote_mdbook_tool_core::{naming, summary};

pub fn login() -> Result<ExitCode> {
    let path = zoom_api::config_path()?;
    let mut config = zoom_api::load_config(&path)?;
    let creds = zoom_api::credentials(&config, &path)?;
    let endpoints = zoom_api::Endpoints::from_env();
    let (listener, redirect_uri) = zoom_api::bind_loopback()?;
    let pkce = zoom_api::pkce()?;
    let state = zoom_api::random_token(24)?;
    let url = zoom_api::authorize_url(
        &endpoints,
        &creds.client_id,
        &redirect_uri,
        &state,
        &pkce.challenge,
    );
    println!("Open this URL to sign in to Zoom:\n{url}");
    zoom_api::open_in_browser(&url);
    let code = zoom_api::wait_for_callback(&listener, &state)?;
    zoom_api::exchange_code(
        &endpoints,
        &creds,
        &mut config,
        &code,
        &redirect_uri,
        &pkce.verifier,
    )?;
    zoom_api::save_config(&path, &config)?;
    let email = zoom_api::signed_in_client(&path)?.me_email()?;
    println!("Signed in to Zoom as {email}");
    Ok(ExitCode::SUCCESS)
}

pub fn logout() -> Result<ExitCode> {
    if zoom_api::logout(&zoom_api::config_path()?)? {
        println!("Signed out of Zoom");
    } else {
        println!("Not signed in to Zoom");
    }
    Ok(ExitCode::SUCCESS)
}

/// Options of `zoom import`.
pub struct ImportArgs {
    pub date: String,
    pub parent: Option<String>,
    pub title: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub signature: Option<String>,
    pub dry_run: bool,
}

pub fn import(nb: &Notebook, args: &ImportArgs) -> Result<ExitCode> {
    let when = naming::parse_date_option(&args.date, naming::now()?.date_naive())?;
    if let Some(parent) = &args.parent {
        nb.find_note(parent)?;
    }
    let client = zoom_api::signed_in_client(&zoom_api::config_path()?)?;
    let meetings = client.meetings_on(when.date())?;
    let selected = zoom::select_meetings(&meetings, when, zoom::date_has_time(&args.date));
    if selected.is_empty() {
        bail!("{}", no_meeting_message(&args.date, &meetings));
    }
    let planned = zoom::plan(
        &selected,
        &zoom::manual_note_ids(nb),
        &zoom::imported_meetings(nb),
        args.parent.as_deref(),
    )?;
    if args.dry_run {
        for p in &planned {
            print_plan(nb, p);
        }
        return Ok(ExitCode::SUCCESS);
    }
    // Fetch everything before writing, so a Zoom error leaves no partial import.
    let fetched = planned
        .into_iter()
        .map(|p| fetch(&client, p))
        .collect::<Result<Vec<_>>>()?;
    let mut created = 0;
    for meeting in &fetched {
        created += write_meeting(nb, args, meeting)?;
    }
    if created > 0 {
        summary::write(nb)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn no_meeting_message(date: &str, meetings: &[Meeting]) -> String {
    let mut message =
        format!("no Zoom meeting with a summary or transcript matches --date {date:?}");
    let usable: Vec<&Meeting> = meetings
        .iter()
        .filter(|m| m.has_summary || m.has_transcript)
        .collect();
    if usable.is_empty() {
        message.push_str("; that day has none");
    } else {
        message.push_str("; that day has:");
        for m in usable {
            message.push_str(&format!("\n  {} {}", m.start.format("%H:%M"), m.topic));
        }
    }
    message
}

fn meeting_heading(nb: &Notebook, p: &PlannedMeeting) -> String {
    let parent = match &p.parent {
        Some(id) => {
            let title = nb.find_note(id).map(|e| e.title()).unwrap_or_default();
            format!("under {id} ({title})")
        }
        None => "with no parent".to_string(),
    };
    format!(
        "{} {} -> {parent}",
        p.meeting.start.format("%Y-%m-%d %H:%M"),
        p.meeting.topic
    )
}

fn print_plan(nb: &Notebook, p: &PlannedMeeting) {
    println!("{}", meeting_heading(nb, p));
    for (label, action) in [("summary", p.summary), ("transcript", p.transcript)] {
        let what = match action {
            Action::Create => "would import",
            Action::AlreadyImported => "already imported",
            Action::Unavailable => "not available",
        };
        println!("  {label}: {what}");
    }
}

/// What will be written for one meeting, fetched up front.
struct FetchedMeeting {
    planned: PlannedMeeting,
    summary: Fetched,
    transcript: Fetched,
}

enum Fetched {
    Body(String),
    Skip(String),
}

fn fetch(client: &ZoomClient, planned: PlannedMeeting) -> Result<FetchedMeeting> {
    let uuid = planned.meeting.uuid.clone();
    let summary = match planned.summary {
        Action::Create => match client.summary(&uuid) {
            Ok(json) => Fetched::Body(zoom::render_summary(&json)),
            Err(e) => Fetched::Skip(format!("not available ({e:#})")),
        },
        other => Fetched::Skip(skip_reason(other)),
    };
    let transcript = match planned.transcript {
        Action::Create => match client.transcript(&uuid)? {
            TranscriptOutcome::Text(vtt) => Fetched::Body(vtt),
            TranscriptOutcome::Unavailable(reason) => {
                Fetched::Skip(format!("not available ({reason})"))
            }
        },
        other => Fetched::Skip(skip_reason(other)),
    };
    Ok(FetchedMeeting {
        planned,
        summary,
        transcript,
    })
}

fn skip_reason(action: Action) -> String {
    match action {
        Action::AlreadyImported => "already imported".to_string(),
        _ => "not available".to_string(),
    }
}

/// Create the meeting's notes; returns how many were created.
fn write_meeting(nb: &Notebook, args: &ImportArgs, m: &FetchedMeeting) -> Result<usize> {
    println!("{}", meeting_heading(nb, &m.planned));
    let mut created = 0;
    for (kind, fetched, file_name) in [
        (Kind::Summary, &m.summary, "summary.md"),
        (Kind::Transcript, &m.transcript, "transcript.vtt"),
    ] {
        let label = kind.as_str();
        match fetched {
            Fetched::Skip(reason) => println!("  {label}: {reason}"),
            Fetched::Body(body) => {
                let path = create_meeting_note(nb, args, &m.planned, kind, body, file_name)?;
                println!("  {label}: {}", nb.display(&path));
                created += 1;
            }
        }
    }
    Ok(created)
}

fn create_meeting_note(
    nb: &Notebook,
    args: &ImportArgs,
    planned: &PlannedMeeting,
    kind: Kind,
    text: &str,
    file_name: &str,
) -> Result<std::path::PathBuf> {
    let meeting = &planned.meeting;
    let topic_title = || {
        let suffix = if kind == Kind::Summary {
            "AI Summary"
        } else {
            "Transcript"
        };
        format!("{} {suffix}", meeting.topic)
    };
    let opts = NoteOptions {
        // With a parent, the parent supplies the default title and timestamp.
        title: args
            .title
            .clone()
            .or_else(|| planned.parent.is_none().then(topic_title)),
        keywords: args.keywords.clone(),
        signature: args.signature.clone(),
        parent: planned.parent.clone(),
        date: planned
            .parent
            .is_none()
            .then(|| naming::format_id(&meeting.start)),
        zoom_meeting: Some(meeting.uuid.clone()),
    };
    let source = Source::Content {
        text,
        file_name: Some(file_name),
    };
    let body = capture::import_body(&source)?;
    capture::create_note(nb, kind, &opts, &body, source.file_name(), true)
}
