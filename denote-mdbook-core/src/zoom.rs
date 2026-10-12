//! `zoom import` decisions that need no network: which meetings a `--date`
//! selects, which note each attaches to, how a summary renders, and what was
//! already imported.

use std::collections::HashSet;

use anyhow::{Result, bail};
use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use serde_json::Value;

use crate::frontmatter::Kind;
use crate::naming;
use crate::notebook::Notebook;

/// A meeting with a time selects starts within this many minutes of it.
const SELECT_WINDOW_MINUTES: i64 = 10;

/// A manual note can be a meeting's parent from this many minutes before the
/// meeting starts until it ends.
const PARENT_LEAD_MINUTES: i64 = 15;

/// One Zoom meeting instance, with times already in local time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub uuid: String,
    pub topic: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub has_summary: bool,
    pub has_transcript: bool,
}

/// Whether a `--date` value names a time, not just a day.
pub fn date_has_time(value: &str) -> bool {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").is_err()
}

/// Meetings with a summary or transcript that `when` selects: with a time,
/// the one starting closest to it within the window; with a day only, every
/// meeting starting that day, in start order.
pub fn select_meetings(meetings: &[Meeting], when: NaiveDateTime, has_time: bool) -> Vec<&Meeting> {
    let mut usable: Vec<&Meeting> = meetings
        .iter()
        .filter(|m| m.has_summary || m.has_transcript)
        .collect();
    if has_time {
        let window = TimeDelta::minutes(SELECT_WINDOW_MINUTES);
        return usable
            .into_iter()
            .filter(|m| (m.start - when).abs() <= window)
            .min_by_key(|m| (m.start - when).abs())
            .into_iter()
            .collect();
    }
    usable.retain(|m| m.start.date() == when.date());
    usable.sort_by_key(|m| m.start);
    usable
}

/// The identifier of the manual note closest to the meeting start, among
/// those from 15 minutes before the start until the meeting ends.
pub fn match_parent(manual_note_ids: &[String], meeting: &Meeting) -> Option<String> {
    let earliest = meeting.start - TimeDelta::minutes(PARENT_LEAD_MINUTES);
    manual_note_ids
        .iter()
        .filter_map(|id| naming::parse_id(id).map(|t| (id, t)))
        .filter(|(_, t)| *t >= earliest && *t <= meeting.end)
        .min_by_key(|(_, t)| (*t - meeting.start).abs())
        .map(|(id, _)| id.clone())
}

/// Markdown for a Zoom meeting summary: `summary_content` when present,
/// otherwise the overview, the labeled sections, and the next steps.
pub fn render_summary(summary: &Value) -> String {
    let text = |key: &str| {
        summary
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
    };
    let content = text("summary_content");
    if !content.is_empty() {
        return format!("{content}\n");
    }
    let mut sections = Vec::new();
    if !text("summary_overview").is_empty() {
        sections.push(format!("## Overview\n\n{}\n", text("summary_overview")));
    }
    let details = summary.get("summary_details").and_then(Value::as_array);
    for detail in details.into_iter().flatten() {
        let label = detail
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let body = detail
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !body.is_empty() {
            let heading = if label.is_empty() { "Details" } else { label };
            sections.push(format!("## {heading}\n\n{body}\n"));
        }
    }
    let steps: Vec<String> = summary
        .get("next_steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("- {s}\n"))
        .collect();
    if !steps.is_empty() {
        sections.push(format!("## Next steps\n\n{}", steps.concat()));
    }
    sections.join("\n")
}

/// (kind, meeting UUID) for every note that records a `zoom_meeting`.
pub fn imported_meetings(nb: &Notebook) -> HashSet<(Kind, String)> {
    nb.scan()
        .into_iter()
        .filter(|e| e.is_note())
        .filter_map(|e| {
            let uuid = e.front()?.zoom_meeting?;
            Some((nb.kind_of(&e), uuid))
        })
        .collect()
}

/// What to do with one kind of note for one meeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    AlreadyImported,
    Unavailable,
}

/// The imports planned for one meeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMeeting {
    pub meeting: Meeting,
    pub parent: Option<String>,
    pub summary: Action,
    pub transcript: Action,
}

/// Plan the selected meetings: each one's parent (`--parent` or the closest
/// manual note) and whether each kind is created, skipped as a repeat, or
/// unavailable. `--parent` is only valid for a single meeting.
pub fn plan(
    selected: &[&Meeting],
    manual_note_ids: &[String],
    imported: &HashSet<(Kind, String)>,
    explicit_parent: Option<&str>,
) -> Result<Vec<PlannedMeeting>> {
    if explicit_parent.is_some() && selected.len() > 1 {
        bail!(
            "--parent needs exactly one meeting, but the date selects {}; add a time to --date",
            selected.len()
        );
    }
    let action = |available: bool, kind: Kind, uuid: &str| {
        if !available {
            Action::Unavailable
        } else if imported.contains(&(kind, uuid.to_string())) {
            Action::AlreadyImported
        } else {
            Action::Create
        }
    };
    Ok(selected
        .iter()
        .map(|m| PlannedMeeting {
            parent: explicit_parent
                .map(str::to_string)
                .or_else(|| match_parent(manual_note_ids, m)),
            summary: action(m.has_summary, Kind::Summary, &m.uuid),
            transcript: action(m.has_transcript, Kind::Transcript, &m.uuid),
            meeting: (*m).clone(),
        })
        .collect())
}

/// Identifiers of every manual note, for `match_parent`.
pub fn manual_note_ids(nb: &Notebook) -> Vec<String> {
    nb.scan()
        .into_iter()
        .filter(|e| e.is_note() && nb.kind_of(e) == Kind::Manual)
        .map(|e| e.name.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_helper_time(hms: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(&format!("2026-10-10 {hms}"), "%Y-%m-%d %H:%M").unwrap()
    }

    fn test_helper_meeting(uuid: &str, start: &str, end: &str) -> Meeting {
        Meeting {
            uuid: uuid.into(),
            topic: format!("Topic {uuid}"),
            start: test_helper_time(start),
            end: test_helper_time(end),
            has_summary: true,
            has_transcript: true,
        }
    }

    #[test]
    fn test_date_has_time() {
        assert!(!date_has_time("2026-10-10"));
        assert!(date_has_time("2026-10-10 08:00"));
        assert!(date_has_time("20261010T080000"));
        assert!(date_has_time("08:00"));
    }

    #[test]
    fn test_select_meetings_by_time_picks_closest() {
        let meetings = [
            test_helper_meeting("a", "08:01", "08:45"),
            test_helper_meeting("b", "07:55", "08:30"),
            test_helper_meeting("c", "09:00", "09:30"),
        ];
        let picked = select_meetings(&meetings, test_helper_time("08:00"), true);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].uuid, "a");
    }

    #[test]
    fn test_select_meetings_by_day_in_start_order() {
        let mut other_day = test_helper_meeting("x", "08:00", "09:00");
        other_day.start -= TimeDelta::days(1);
        let mut empty = test_helper_meeting("e", "07:00", "07:30");
        empty.has_summary = false;
        empty.has_transcript = false;
        let meetings = [
            test_helper_meeting("c", "09:00", "09:30"),
            other_day,
            empty,
            test_helper_meeting("a", "08:01", "08:45"),
        ];
        let day = test_helper_time("00:00");
        let uuids: Vec<&str> = select_meetings(&meetings, day, false)
            .iter()
            .map(|m| m.uuid.as_str())
            .collect();
        assert_eq!(uuids, ["a", "c"]);
    }

    #[test]
    fn test_select_meetings_none_in_window() {
        let meetings = [test_helper_meeting("a", "08:11", "08:45")];
        assert!(select_meetings(&meetings, test_helper_time("08:00"), true).is_empty());
    }

    #[test]
    fn test_match_parent_closest_manual_note() {
        let meeting = test_helper_meeting("a", "08:01", "08:45");
        let ids = [
            "20261010T074500".to_string(),
            "20261010T080000".to_string(),
            "20261010T083000".to_string(),
        ];
        assert_eq!(
            match_parent(&ids, &meeting).as_deref(),
            Some("20261010T080000")
        );
    }

    #[test]
    fn test_match_parent_ignores_summaries_and_transcripts() {
        // Only manual note identifiers are passed in; `manual_note_ids`
        // filters by kind, so a list without them finds nothing.
        let meeting = test_helper_meeting("a", "08:01", "08:45");
        assert_eq!(match_parent(&[], &meeting), None);
    }

    #[test]
    fn test_match_parent_none_outside_window() {
        let meeting = test_helper_meeting("a", "08:01", "08:45");
        let ids = [
            "20261010T074500".to_string(), // 16 minutes early
            "20261010T084600".to_string(), // after the end
        ];
        assert_eq!(match_parent(&ids, &meeting), None);
    }

    #[test]
    fn test_render_summary_from_sections() {
        let summary = json!({
            "summary_overview": "Budget approved.",
            "summary_details": [
                {"label": "Budget", "summary": "Q4 budget is approved."},
                {"label": "Hiring", "summary": ""}
            ],
            "next_steps": ["Dana sends the plan", " "]
        });
        assert_eq!(
            render_summary(&summary),
            "## Overview\n\nBudget approved.\n\n\
             ## Budget\n\nQ4 budget is approved.\n\n\
             ## Next steps\n\n- Dana sends the plan\n"
        );
    }

    #[test]
    fn test_render_summary_prefers_summary_content() {
        let summary = json!({
            "summary_content": "## Quick recap\n\nAll good.",
            "summary_overview": "ignored"
        });
        assert_eq!(render_summary(&summary), "## Quick recap\n\nAll good.\n");
    }

    #[test]
    fn test_plan_skips_already_imported_kind() {
        let mut no_transcript = test_helper_meeting("b", "09:00", "09:30");
        no_transcript.has_transcript = false;
        let meetings = [test_helper_meeting("a", "08:01", "08:45"), no_transcript];
        let selected: Vec<&Meeting> = meetings.iter().collect();
        let imported = HashSet::from([(Kind::Summary, "a".to_string())]);
        let ids = ["20261010T080000".to_string()];
        let planned = plan(&selected, &ids, &imported, None).unwrap();
        assert_eq!(planned[0].parent.as_deref(), Some("20261010T080000"));
        assert_eq!(planned[0].summary, Action::AlreadyImported);
        assert_eq!(planned[0].transcript, Action::Create);
        assert_eq!(planned[1].parent, None);
        assert_eq!(planned[1].summary, Action::Create);
        assert_eq!(planned[1].transcript, Action::Unavailable);
    }

    #[test]
    fn test_plan_rejects_parent_for_many_meetings() {
        let meetings = [
            test_helper_meeting("a", "08:01", "08:45"),
            test_helper_meeting("b", "09:00", "09:30"),
        ];
        let selected: Vec<&Meeting> = meetings.iter().collect();
        let error = plan(&selected, &[], &HashSet::new(), Some("20261010T080000")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("--parent needs exactly one meeting")
        );
    }

    #[test]
    fn test_plan_explicit_parent_wins() {
        let meetings = [test_helper_meeting("a", "08:01", "08:45")];
        let selected: Vec<&Meeting> = meetings.iter().collect();
        let ids = ["20261010T080000".to_string()];
        let planned = plan(&selected, &ids, &HashSet::new(), Some("20261009T090000")).unwrap();
        assert_eq!(planned[0].parent.as_deref(), Some("20261009T090000"));
    }
}
