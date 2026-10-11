//! Generating the aa-denote-mdbook-tool region of `src/SUMMARY.md` and the
//! per-day pages it links to.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::process::Command;

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::capture::escape_link_text;
use crate::external;
use crate::frontmatter::Kind;
use crate::notebook::{self, BEGIN_MARKER, END_MARKER, Notebook};

/// Folder under `src/` holding one generated page per day.
pub const DAYS_DIR: &str = "days";

struct Item {
    id: String,
    title: String,
    parent: Option<String>,
    kind: Kind,
    /// Path relative to `src/`.
    link: String,
}

/// One day's notes: `date` is `YYYY-MM-DD`, and each entry is a nesting
/// depth (0 for a meeting, 1 for its attachments) and the note.
struct Day<'a> {
    date: String,
    entries: Vec<(usize, &'a Item)>,
}

/// Build the generated region (without markers): one linked list item per
/// day, newest first, with that day's notes nested beneath it.
pub fn generate(nb: &Notebook) -> String {
    render_region(&group_by_day(&collect_items(nb)))
}

fn render_region(days: &[Day]) -> String {
    let mut out = String::new();
    for day in days {
        out.push_str(&format!("- [{}]({DAYS_DIR}/{}.md)\n", day.date, day.date));
        for (depth, item) in &day.entries {
            out.push_str(&entry_line(&"  ".repeat(depth + 1), "", item));
        }
    }
    if out.is_empty() {
        return out;
    }
    format!("\n{out}\n")
}

/// The page for one day: its date as the heading and its notes as links
/// relative to `src/days/`.
fn day_page(day: &Day) -> String {
    let mut out = format!("# {}\n\n", day.date);
    for (depth, item) in &day.entries {
        out.push_str(&entry_line(&"  ".repeat(*depth), "../", item));
    }
    out
}

/// Every Denote note under `src/notes/`, with its title, parent, and kind.
fn collect_items(nb: &Notebook) -> Vec<Item> {
    let src = nb.src();
    let notes_dir = src.join("notes");
    nb.scan()
        .into_iter()
        .filter(|e| e.is_note() && e.path.starts_with(&notes_dir))
        .map(|e| {
            let front = e.front();
            Item {
                id: e.name.id.clone(),
                title: front
                    .as_ref()
                    .map(|f| f.title.clone())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| e.name.id.clone()),
                parent: front.and_then(|f| f.parent),
                kind: nb.kind_of(&e),
                link: notebook::relative(&src, &e.path),
            }
        })
        .collect()
}

/// Group notes by identifier date, newest day first. Within a day, meetings
/// are in identifier order, each followed by its summaries then transcripts.
fn group_by_day(items: &[Item]) -> Vec<Day<'_>> {
    let by_id: HashMap<&str, &Item> = items.iter().map(|i| (i.id.as_str(), i)).collect();
    let valid_parent = |i: &Item| -> Option<String> {
        let p = i.parent.as_deref()?;
        (p != i.id && by_id.contains_key(p)).then(|| p.to_string())
    };
    // A note nests under its parent only when that parent is itself top-level.
    // Chains and cycles flatten to top-level entries.
    let nested_under = |i: &Item| -> Option<String> {
        let p = valid_parent(i)?;
        let parent = by_id.get(p.as_str())?;
        valid_parent(parent).is_none().then_some(p)
    };

    let mut children: HashMap<String, Vec<&Item>> = HashMap::new();
    let mut days: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for item in items {
        match nested_under(item) {
            Some(p) => children.entry(p).or_default().push(item),
            None => days.entry(item.id[..8].to_string()).or_default().push(item),
        }
    }

    let mut out = Vec::new();
    for (day, mut tops) in days.into_iter().rev() {
        tops.sort_by(|a, b| a.id.cmp(&b.id));
        let mut entries = Vec::new();
        for top in tops {
            entries.push((0, top));
            if let Some(kids) = children.get_mut(&top.id) {
                kids.sort_by(|a, b| {
                    a.kind
                        .attachment_rank()
                        .cmp(&b.kind.attachment_rank())
                        .then(a.id.cmp(&b.id))
                });
                entries.extend(kids.iter().map(|kid| (1, *kid)));
            }
        }
        let date = format!("{}-{}-{}", &day[..4], &day[4..6], &day[6..8]);
        out.push(Day { date, entries });
    }
    out
}

fn entry_line(indent: &str, link_prefix: &str, item: &Item) -> String {
    format!(
        "{indent}- [{}]({link_prefix}{})\n",
        escape_link_text(&item.title),
        item.link
    )
}

/// Rewrite the marker region of `src/SUMMARY.md`, keeping everything else,
/// and rewrite the day pages it links to.
pub fn write(nb: &Notebook) -> Result<()> {
    let items = collect_items(nb);
    let days = group_by_day(&items);
    write_day_pages(nb, &days)?;
    let path = nb.src().join("SUMMARY.md");
    let existing = fs::read_to_string(&path).unwrap_or_else(|_| "# Summary\n".to_string());
    notebook::safe_write(&path, &replace_region(&existing, &render_region(&days)))
}

/// Write `src/days/YYYY-MM-DD.md` for every day with notes and delete
/// date-named pages for days that no longer have any. Other files in the
/// folder are left alone.
fn write_day_pages(nb: &Notebook, days: &[Day]) -> Result<()> {
    let dir = nb.src().join(DAYS_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let mut current = HashSet::new();
    for day in days {
        let path = dir.join(format!("{}.md", day.date));
        let page = day_page(day);
        if fs::read_to_string(&path).ok().as_deref() != Some(page.as_str()) {
            notebook::safe_write(&path, &page)?;
        }
        current.insert(day.date.clone());
    }
    for entry in fs::read_dir(&dir)?.filter_map(Result::ok) {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let is_md = path.extension().is_some_and(|e| e == "md");
        let is_date = NaiveDate::parse_from_str(stem, "%Y-%m-%d").is_ok();
        if is_md && is_date && !current.contains(stem) {
            fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
        }
    }
    Ok(())
}

/// Replace the text between the marker lines with `region`, appending the
/// markers when missing.
pub fn replace_region(existing: &str, region: &str) -> String {
    let begin = existing.find(BEGIN_MARKER);
    let end = existing.find(END_MARKER);
    match (begin, end) {
        (Some(b), Some(e)) if e > b => {
            let after_begin = b + BEGIN_MARKER.len();
            format!("{}\n{region}{}", &existing[..after_begin], &existing[e..])
        }
        _ => {
            let mut out = existing.trim_end().to_string();
            out.push_str(&format!("\n\n{BEGIN_MARKER}\n{region}{END_MARKER}\n"));
            out
        }
    }
}

/// Regenerate SUMMARY.md, then run `mdbook <args> <root>` and return its
/// exit status. The running executable's directory is put first on `PATH` so
/// mdBook finds the `aa-denote-mdbook-tool preprocess` command configured in
/// `book.toml` even when it is not installed.
pub fn run_mdbook(nb: &Notebook, args: &[String]) -> Result<u8> {
    write(nb)?;
    let mut cmd = Command::new("mdbook");
    cmd.args(args).arg(&nb.root);
    if let Some(path) = external::path_with_self() {
        cmd.env("PATH", path);
    }
    external::run(
        cmd,
        "Install mdBook with `cargo install mdbook` (https://rust-lang.github.io/mdBook/).",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_replaced_and_outside_kept() {
        let s =
            format!("# Summary\n\n[About](about.md)\n\n{BEGIN_MARKER}\nold\n{END_MARKER}\ntail\n");
        let out = replace_region(&s, "\nnew\n\n");
        assert_eq!(
            out,
            format!(
                "# Summary\n\n[About](about.md)\n\n{BEGIN_MARKER}\n\nnew\n\n{END_MARKER}\ntail\n"
            )
        );
    }

    #[test]
    fn test_markers_out_of_order_are_appended() {
        let s = format!("# Summary\n{END_MARKER}\nkeep\n{BEGIN_MARKER}\n");
        let out = replace_region(&s, "x\n");
        assert_eq!(
            out,
            format!("{}\n\n{BEGIN_MARKER}\nx\n{END_MARKER}\n", s.trim_end())
        );
    }

    #[test]
    fn test_markers_appended_when_missing() {
        let out = replace_region("# Summary\n", "x\n");
        assert_eq!(
            out,
            format!("# Summary\n\n{BEGIN_MARKER}\nx\n{END_MARKER}\n")
        );
    }
}
