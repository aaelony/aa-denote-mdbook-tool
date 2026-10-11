//! `+++` TOML front matter (Denote's `markdown-toml` format).

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Manual,
    Summary,
    Transcript,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Manual => "manual",
            Kind::Summary => "summary",
            Kind::Transcript => "transcript",
        }
    }

    /// Folder under `src/` that holds notes of this kind.
    pub fn folder(self) -> &'static str {
        match self {
            Kind::Manual => "notes",
            Kind::Summary => "notes/zoom-summaries",
            Kind::Transcript => "notes/zoom-transcripts",
        }
    }

    /// Sort order of attachments under a parent: summaries, then transcripts.
    pub fn attachment_rank(self) -> u8 {
        match self {
            Kind::Summary => 0,
            Kind::Transcript => 1,
            Kind::Manual => 2,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontMatter {
    pub title: String,
    /// RFC 3339 datetime with offset.
    pub date: String,
    pub tags: Vec<String>,
    pub identifier: String,
    pub kind: String,
    pub parent: Option<String>,
    pub imported: Option<String>,
}

fn toml_string(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

impl FrontMatter {
    /// Render the block, including both `+++` lines and a trailing newline.
    /// One `key = value` per line so plain `rg` patterns match.
    pub fn render(&self) -> String {
        let tags: Vec<String> = self.tags.iter().map(|t| toml_string(t)).collect();
        let mut out = String::from("+++\n");
        let _ = writeln!(out, "title = {}", toml_string(&self.title));
        let _ = writeln!(out, "date = {}", self.date);
        let _ = writeln!(out, "tags = [{}]", tags.join(", "));
        let _ = writeln!(out, "identifier = {}", toml_string(&self.identifier));
        let _ = writeln!(out, "kind = {}", toml_string(&self.kind));
        if let Some(p) = &self.parent {
            let _ = writeln!(out, "parent = {}", toml_string(p));
        }
        if let Some(i) = &self.imported {
            let _ = writeln!(out, "imported = {i}");
        }
        out.push_str("+++\n");
        out
    }

    /// Parse the front matter at the start of `content`, ignoring unknown keys
    /// and key order. Returns `None` when there is no valid block.
    pub fn parse(content: &str) -> Option<Self> {
        let (block, _) = split(content)?;
        let table: toml::Table = toml::from_str(block).ok()?;
        let text = |k: &str| match table.get(k) {
            Some(toml::Value::String(s)) => Some(s.clone()),
            Some(toml::Value::Datetime(d)) => Some(d.to_string()),
            _ => None,
        };
        let tags = match table.get("tags") {
            Some(toml::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            title: text("title").unwrap_or_default(),
            date: text("date").unwrap_or_default(),
            tags,
            identifier: text("identifier").unwrap_or_default(),
            kind: text("kind").unwrap_or_default(),
            parent: text("parent"),
            imported: text("imported"),
        })
    }
}

/// Split `content` into (TOML block without delimiters, rest after the
/// closing `+++` line).
pub fn split(content: &str) -> Option<(&str, &str)> {
    let first_end = content.find('\n')?;
    if content[..first_end].trim_end() != "+++" {
        return None;
    }
    let body_start = first_end + 1;
    let mut pos = body_start;
    while pos <= content.len() {
        let line_end = content[pos..].find('\n').map_or(content.len(), |i| pos + i);
        if content[pos..line_end].trim_end() == "+++" {
            let rest_start = (line_end + 1).min(content.len());
            return Some((&content[body_start..pos], &content[rest_start..]));
        }
        if line_end == content.len() {
            break;
        }
        pos = line_end + 1;
    }
    None
}

/// Remove a leading front matter block, if any.
pub fn strip(content: &str) -> &str {
    split(content).map_or(content, |(_, rest)| rest)
}

/// Replace (or insert) a `key = value` line inside the front matter block,
/// leaving every other line untouched.
pub fn set_field(content: &str, key: &str, value: &str) -> String {
    let Some((block, rest)) = split(content) else {
        return content.to_string();
    };
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in block.lines() {
        let is_key = line.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if is_key {
            lines.push(format!("{key} = {value}"));
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.push(format!("{key} = {value}"));
    }
    format!("+++\n{}\n+++\n{rest}", lines.join("\n"))
}

pub fn toml_str(s: &str) -> String {
    toml_string(s)
}

pub fn toml_str_array(items: &[String]) -> String {
    let v: Vec<String> = items.iter().map(|t| toml_string(t)).collect();
    format!("[{}]", v.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_helper_weekly_sync_front_matter() -> FrontMatter {
        FrontMatter {
            title: "Weekly Sync".into(),
            date: "2026-10-10T08:00:00-07:00".into(),
            tags: vec!["acme".into(), "meeting".into()],
            identifier: "20261010T080000".into(),
            kind: "manual".into(),
            parent: None,
            imported: None,
        }
    }

    #[test]
    fn test_render_matches_spec() {
        let r = test_helper_weekly_sync_front_matter().render();
        assert_eq!(
            r,
            "+++\ntitle = \"Weekly Sync\"\ndate = 2026-10-10T08:00:00-07:00\n\
             tags = [\"acme\", \"meeting\"]\nidentifier = \"20261010T080000\"\n\
             kind = \"manual\"\n+++\n"
        );
        let (block, _) = split(&r).unwrap();
        assert!(toml::from_str::<toml::Table>(block).is_ok());
    }

    #[test]
    fn test_split_returns_exact_block_and_rest() {
        assert_eq!(
            split("+++\na = 1\n+++\nbody\n"),
            Some(("a = 1\n", "body\n"))
        );
    }

    #[test]
    fn test_round_trip_with_optional_fields() {
        let mut fm = test_helper_weekly_sync_front_matter();
        fm.kind = "transcript".into();
        fm.parent = Some("20261010T080000".into());
        fm.imported = Some("2026-10-10T12:00:00-07:00".into());
        let content = format!("{}\n# Weekly Sync\n", fm.render());
        assert_eq!(FrontMatter::parse(&content), Some(fm));
    }

    #[test]
    fn test_parse_is_order_independent_and_lenient() {
        let content = "+++\nidentifier = \"20261010T080000\"\nextra = 1\ntitle = \"T\"\n+++\nbody";
        let fm = FrontMatter::parse(content).unwrap();
        assert_eq!(fm.title, "T");
        assert_eq!(fm.identifier, "20261010T080000");
        assert!(fm.parent.is_none());
        assert_eq!(strip(content), "body");
    }

    #[test]
    fn test_set_field_replaces_only_that_line() {
        let content = format!(
            "{}\n# Weekly Sync\n",
            test_helper_weekly_sync_front_matter().render()
        );
        let out = set_field(&content, "title", &toml_str("Acme Weekly"));
        let fm = FrontMatter::parse(&out).unwrap();
        assert_eq!(fm.title, "Acme Weekly");
        assert_eq!(fm.tags, vec!["acme", "meeting"]);
        assert!(out.ends_with("\n# Weekly Sync\n"));
    }
}
