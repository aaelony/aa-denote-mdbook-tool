//! Denote file names and note timestamps.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// Format of a Denote identifier.
const ID_FORMAT: &str = "%Y%m%dT%H%M%S";
const ID_LEN: usize = 15;

/// Lowercase the title and turn every run of non-alphanumerics into one hyphen.
pub fn slug_title(title: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_hyphen = true;
        }
    }
    out
}

/// Lowercase a keyword and drop everything that is not an ASCII letter or digit.
pub fn normalize_keyword(keyword: &str) -> String {
    keyword
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Normalize, drop empties and duplicates, and sort keywords.
pub fn normalize_keywords<I, S>(keywords: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<String> = keywords
        .into_iter()
        .map(|k| normalize_keyword(k.as_ref()))
        .filter(|k| !k.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Lowercase a signature, keeping ASCII letters, digits, and `=`.
pub fn normalize_signature(signature: &str) -> String {
    signature
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '=')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The parts of a Denote file name: `ID[==SIG][--TITLE][__KW].EXT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenoteName {
    pub id: String,
    pub signature: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub ext: String,
}

impl DenoteName {
    /// Build a name from raw user input, normalizing every part.
    pub fn new(id: &str, signature: &str, title: &str, keywords: &[String], ext: &str) -> Self {
        Self {
            id: id.to_string(),
            signature: normalize_signature(signature),
            title: slug_title(title),
            keywords: normalize_keywords(keywords),
            ext: ext.to_string(),
        }
    }

    pub fn file_name(&self) -> String {
        let mut name = self.id.clone();
        if !self.signature.is_empty() {
            name.push_str("==");
            name.push_str(&self.signature);
        }
        if !self.title.is_empty() {
            name.push_str("--");
            name.push_str(&self.title);
        }
        if !self.keywords.is_empty() {
            name.push_str("__");
            name.push_str(&self.keywords.join("_"));
        }
        name.push('.');
        name.push_str(&self.ext);
        name
    }

    /// Parse a file name. Returns `None` for anything that is not a Denote
    /// `.md` or `.png` name.
    pub fn parse(file_name: &str) -> Option<Self> {
        let (stem, ext) = file_name.rsplit_once('.')?;
        if ext != "md" && ext != "png" {
            return None;
        }
        let id = stem.get(..ID_LEN)?;
        if !is_identifier(id) {
            return None;
        }
        let mut rest = &stem[ID_LEN..];

        let mut signature = String::new();
        if let Some(after) = rest.strip_prefix("==") {
            let end = [after.find("--"), after.find("__")]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(after.len());
            signature = after[..end].to_string();
            rest = &after[end..];
            if signature.is_empty() || !signature.chars().all(|c| is_lower_alnum(c) || c == '=') {
                return None;
            }
        }

        let mut title = String::new();
        if let Some(after) = rest.strip_prefix("--") {
            let end = after.find("__").unwrap_or(after.len());
            title = after[..end].to_string();
            rest = &after[end..];
            if title.is_empty() || !title.chars().all(|c| is_lower_alnum(c) || c == '-') {
                return None;
            }
        }

        let mut keywords = Vec::new();
        if let Some(after) = rest.strip_prefix("__") {
            keywords = after.split('_').map(str::to_string).collect();
            rest = "";
            if keywords
                .iter()
                .any(|k| k.is_empty() || !k.chars().all(is_lower_alnum))
            {
                return None;
            }
        }

        if !rest.is_empty() {
            return None;
        }
        Some(Self {
            id: id.to_string(),
            signature,
            title,
            keywords,
            ext: ext.to_string(),
        })
    }
}

fn is_lower_alnum(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// True if `s` is exactly a `YYYYMMDDTHHMMSS` identifier.
pub fn is_identifier(s: &str) -> bool {
    s.len() == ID_LEN && parse_id(s).is_some()
}

/// The identifier at the start of a file name, if any.
pub fn leading_identifier(file_name: &str) -> Option<&str> {
    let id = file_name.get(..ID_LEN)?;
    is_identifier(id).then_some(id)
}

pub fn format_id(t: &NaiveDateTime) -> String {
    t.format(ID_FORMAT).to_string()
}

pub fn parse_id(id: &str) -> Option<NaiveDateTime> {
    if id.len() != ID_LEN {
        return None;
    }
    NaiveDateTime::parse_from_str(id, ID_FORMAT).ok()
}

/// Current local time, overridable with the hidden `AA_DENOTE_MDBOOK_TOOL_NOW`
/// variable (RFC 3339) so tests get exact file names.
pub fn now() -> Result<DateTime<Local>> {
    match std::env::var("AA_DENOTE_MDBOOK_TOOL_NOW") {
        Ok(s) => Ok(DateTime::parse_from_rfc3339(&s)
            .with_context(|| format!("AA_DENOTE_MDBOOK_TOOL_NOW is not RFC 3339: {s}"))?
            .with_timezone(&Local)),
        Err(_) => Ok(Local::now()),
    }
}

/// Render a local wall-clock time as RFC 3339 with the local offset.
pub fn to_rfc3339_local(t: &NaiveDateTime) -> String {
    let local = Local
        .from_local_datetime(t)
        .earliest()
        .unwrap_or_else(|| Local.from_utc_datetime(t));
    local.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// Parse a `--date` value as local time. `today` fills in time-only values.
pub fn parse_date_option(value: &str, today: NaiveDate) -> Result<NaiveDateTime> {
    let v = value.trim();
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        ID_FORMAT,
    ] {
        if let Ok(t) = NaiveDateTime::parse_from_str(v, fmt) {
            return Ok(t);
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(v, "%Y-%m-%d") {
        return Ok(d.and_time(NaiveTime::MIN));
    }
    for fmt in ["%H:%M:%S", "%H:%M"] {
        if let Ok(t) = NaiveTime::parse_from_str(v, fmt) {
            return Ok(today.and_time(t));
        }
    }
    bail!(
        "invalid --date {value:?}; accepted formats: \"YYYY-MM-DD HH:MM[:SS]\", \
         \"YYYY-MM-DDTHH:MM[:SS]\", \"YYYY-MM-DD\", \"HH:MM[:SS]\" (today), \
         or a Denote identifier YYYYMMDDTHHMMSS"
    )
}

/// A Zoom `GMTYYYYMMDD-HHMMSS` timestamp in a file name, converted from UTC
/// to local time.
pub fn zoom_timestamp(file_name: &str) -> Option<NaiveDateTime> {
    let mut search = file_name;
    while let Some(pos) = search.find("GMT") {
        let after = &search[pos + 3..];
        // `get(..15)` guarantees the length; the shape check rejects
        // anything chrono would accept loosely.
        if let Some(candidate) = after.get(..15)
            && candidate
                .char_indices()
                .all(|(i, c)| if i == 8 { c == '-' } else { c.is_ascii_digit() })
            && let Ok(utc) = NaiveDateTime::parse_from_str(candidate, "%Y%m%d-%H%M%S")
        {
            return Some(
                Utc.from_utc_datetime(&utc)
                    .with_timezone(&Local)
                    .naive_local(),
            );
        }
        search = after;
    }
    None
}

/// Choose a note's timestamp: `--date`, then the parent's identifier, then a
/// Zoom file-name timestamp, then the clock.
pub fn resolve_timestamp(
    date: Option<&str>,
    parent_id: Option<&str>,
    source_file_name: Option<&str>,
    now: &DateTime<Local>,
) -> Result<NaiveDateTime> {
    if let Some(d) = date {
        return parse_date_option(d, now.date_naive());
    }
    if let Some(t) = parent_id.and_then(parse_id) {
        return Ok(t);
    }
    if let Some(t) = source_file_name.and_then(zoom_timestamp) {
        return Ok(t);
    }
    Ok(now.naive_local().with_nanosecond_zero())
}

trait TruncateNanos {
    fn with_nanosecond_zero(self) -> Self;
}

impl TruncateNanos for NaiveDateTime {
    fn with_nanosecond_zero(self) -> Self {
        use chrono::Timelike;
        self.with_nanosecond(0).unwrap_or(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_helper_keywords(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_slug_rules() {
        assert_eq!(
            slug_title("Q4 Roadmap: Acme / Beta!"),
            "q4-roadmap-acme-beta"
        );
        assert_eq!(slug_title("Weekly Sync"), "weekly-sync");
        assert_eq!(slug_title("  --Hi--  "), "hi");
        assert_eq!(slug_title(""), "");
    }

    #[test]
    fn test_keyword_rules() {
        let k = normalize_keywords(["Zoom", "ACME-corp", "zoom", ""]);
        assert_eq!(k, test_helper_keywords(&["acmecorp", "zoom"]));
        let name = DenoteName::new("20261010T080000", "", "", &k, "md");
        assert_eq!(name.file_name(), "20261010T080000__acmecorp_zoom.md");
    }

    #[test]
    fn test_signature_rules() {
        assert_eq!(normalize_signature("1-A"), "1a");
        let name = DenoteName::new("20261010T080000", "1-A", "", &[], "md");
        assert_eq!(name.file_name(), "20261010T080000==1a.md");
    }

    #[test]
    fn test_full_name() {
        let n = DenoteName::new(
            "20261010T080000",
            "",
            "Weekly Sync",
            &test_helper_keywords(&["meeting", "acme"]),
            "md",
        );
        assert_eq!(
            n.file_name(),
            "20261010T080000--weekly-sync__acme_meeting.md"
        );
        let n = DenoteName::new(
            "20261010T080000",
            "1a",
            "Weekly Sync",
            &test_helper_keywords(&["meeting", "acme"]),
            "md",
        );
        assert_eq!(
            n.file_name(),
            "20261010T080000==1a--weekly-sync__acme_meeting.md"
        );
        let n = DenoteName::new("20261010T080000", "", "", &[], "md");
        assert_eq!(n.file_name(), "20261010T080000.md");
    }

    #[test]
    fn test_round_trip() {
        for f in [
            "20261010T080000--weekly-sync__acme_meeting.md",
            "20261010T080000==1a--weekly-sync__acme_meeting.md",
            "20261010T080000==1a__acme.md",
            "20261010T080000==1=a--weekly-sync.md",
            "20261010T080000.md",
            "20261010T080000--arch-diagram.png",
        ] {
            let n = DenoteName::parse(f).unwrap_or_else(|| panic!("parse {f}"));
            assert_eq!(n.file_name(), f);
        }
    }

    #[test]
    fn test_rejects_non_denote() {
        for f in [
            "README.md",
            "SUMMARY.md",
            "20261010T080000--x.txt",
            "2026101T080000.md",
            "20261310T080000.md",
            "20261010T080000junk.md",
            "20261010T080000--Upper.md",
            "20261010T080000==--empty-signature.md",
            "20261010T080000==Up--upper-signature.md",
            "20261010T080000__a__b.md",
        ] {
            assert!(DenoteName::parse(f).is_none(), "{f} should be rejected");
        }
    }

    #[test]
    fn test_identifier_format() {
        let t = NaiveDate::from_ymd_opt(2026, 10, 12)
            .unwrap()
            .and_hms_opt(14, 5, 9)
            .unwrap();
        assert_eq!(format_id(&t), "20261012T140509");
        assert_eq!(parse_id("20261012T140509"), Some(t));
    }

    fn test_helper_naive_datetime(
        y: i32,
        mo: u32,
        d: u32,
        h: u32,
        mi: u32,
        s: u32,
    ) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, s)
            .unwrap()
    }

    #[test]
    fn test_date_option_formats() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        assert_eq!(
            parse_date_option("2026-10-10 08:30", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 8, 30, 0)
        );
        assert_eq!(
            parse_date_option("2026-10-10 08:30:15", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 8, 30, 15)
        );
        assert_eq!(
            parse_date_option("2026-10-10T08:30", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 8, 30, 0)
        );
        assert_eq!(
            parse_date_option("2026-10-09", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 9, 0, 0, 0)
        );
        assert_eq!(
            parse_date_option("09:00", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 9, 0, 0)
        );
        assert_eq!(
            parse_date_option("20261010T080000", today).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 8, 0, 0)
        );
        assert!(parse_date_option("next tuesday", today).is_err());
    }

    #[test]
    fn test_precedence() {
        let now = Local
            .from_local_datetime(&test_helper_naive_datetime(2026, 10, 10, 12, 0, 0))
            .earliest()
            .unwrap();
        let zoom = "GMT20261010-150012_Recording.transcript.vtt";
        let parent = Some("20261010T080000");
        // date wins over everything
        let t = resolve_timestamp(Some("2026-10-10 08:30"), parent, Some(zoom), &now).unwrap();
        assert_eq!(t, test_helper_naive_datetime(2026, 10, 10, 8, 30, 0));
        // parent wins over file name
        let t = resolve_timestamp(None, parent, Some(zoom), &now).unwrap();
        assert_eq!(t, test_helper_naive_datetime(2026, 10, 10, 8, 0, 0));
        // file name wins over clock
        let expected = Utc
            .from_utc_datetime(&test_helper_naive_datetime(2026, 10, 10, 15, 0, 12))
            .with_timezone(&Local)
            .naive_local();
        assert_eq!(
            resolve_timestamp(None, None, Some(zoom), &now).unwrap(),
            expected
        );
        // clock last
        assert_eq!(
            resolve_timestamp(None, None, Some("notes.vtt"), &now).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 12, 0, 0)
        );
        assert_eq!(
            resolve_timestamp(None, None, None, &now).unwrap(),
            test_helper_naive_datetime(2026, 10, 10, 12, 0, 0)
        );
    }

    #[test]
    fn test_zoom_name_parsing() {
        assert!(zoom_timestamp("GMT20261010-150012_Recording.vtt").is_some());
        assert!(zoom_timestamp("x_GMT20261010-150012.vtt").is_some());
        assert!(zoom_timestamp("GMT2026101-150012.vtt").is_none());
        assert!(zoom_timestamp("GMT20261310-150012.vtt").is_none());
        assert!(zoom_timestamp("meeting.vtt").is_none());
    }
}
