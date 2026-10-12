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

/// Generated page under `src/` listing every reference note.
pub const REFERENCE_PAGE: &str = "reference.md";

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

/// Build the generated region (without markers): a Reference item when
/// reference notes exist, then one linked list item per day, newest first,
/// with that day's notes nested beneath it.
pub fn generate(nb: &Notebook) -> String {
    let references = reference_items(nb);
    render_region(&references, &group_by_day(&collect_items(nb)))
}

fn render_region(references: &[Item], days: &[Day]) -> String {
    let mut out = String::new();
    if !references.is_empty() {
        out.push_str(&format!("- [Reference]({REFERENCE_PAGE})\n"));
        for item in references {
            out.push_str(&entry_line("  ", "", item));
        }
    }
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

/// The page listing every reference note, with links relative to `src/`.
fn reference_page(references: &[Item]) -> String {
    let mut out = "# Reference\n\n".to_string();
    for item in references {
        out.push_str(&entry_line("", "", item));
    }
    out
}

/// Every Denote note under `src/notes/`, with its title, parent, and kind.
fn collect_items(nb: &Notebook) -> Vec<Item> {
    let notes_dir = nb.src().join("notes");
    items_where(nb, |e| e.path.starts_with(&notes_dir))
}

/// Every reference note, ordered by title ignoring case, then identifier.
fn reference_items(nb: &Notebook) -> Vec<Item> {
    let mut items = items_where(nb, |e| nb.kind_of(e) == Kind::Reference);
    sort_references(&mut items);
    items
}

fn sort_references(items: &mut [Item]) {
    items.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Every Denote note matching `keep`, with its title, parent, and kind.
fn items_where(nb: &Notebook, keep: impl Fn(&notebook::Entry) -> bool) -> Vec<Item> {
    let src = nb.src();
    nb.scan()
        .into_iter()
        .filter(|e| e.is_note() && keep(e))
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
    let references = reference_items(nb);
    write_reference_page(nb, &references)?;
    let path = nb.src().join("SUMMARY.md");
    let existing = fs::read_to_string(&path).unwrap_or_else(|_| "# Summary\n".to_string());
    let region = render_region(&references, &days);
    notebook::safe_write(&path, &replace_region(&existing, &region))
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

/// Write `src/reference.md` when reference notes exist; remove it otherwise.
fn write_reference_page(nb: &Notebook, references: &[Item]) -> Result<()> {
    let path = nb.src().join(REFERENCE_PAGE);
    if references.is_empty() {
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
        }
        return Ok(());
    }
    let page = reference_page(references);
    if fs::read_to_string(&path).ok().as_deref() != Some(page.as_str()) {
        notebook::safe_write(&path, &page)?;
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

    fn test_helper_item(id: &str, title: &str, kind: Kind, link: &str) -> Item {
        Item {
            id: id.to_string(),
            title: title.to_string(),
            parent: None,
            kind,
            link: link.to_string(),
        }
    }

    #[test]
    fn test_references_sorted_by_title_ignoring_case_then_id() {
        let mut items = vec![
            test_helper_item("20261012T090002", "Product: Widget", Kind::Reference, "p"),
            test_helper_item("20261012T090001", "glossary", Kind::Reference, "g2"),
            test_helper_item("20261012T090000", "glossary", Kind::Reference, "g1"),
            test_helper_item("20261012T090003", "Acronyms", Kind::Reference, "a"),
        ];
        sort_references(&mut items);
        let links: Vec<&str> = items.iter().map(|i| i.link.as_str()).collect();
        assert_eq!(links, ["a", "g1", "g2", "p"]);
    }

    #[test]
    fn test_reference_item_comes_before_days() {
        let references = [test_helper_item(
            "20261012T090000",
            "Glossary",
            Kind::Reference,
            "reference/20261012T090000--glossary.md",
        )];
        let meeting = test_helper_item(
            "20261010T080000",
            "Weekly Sync",
            Kind::Manual,
            "notes/20261010T080000--weekly-sync.md",
        );
        let days = [Day {
            date: "2026-10-10".to_string(),
            entries: vec![(0, &meeting)],
        }];
        assert_eq!(
            render_region(&references, &days),
            "\n- [Reference](reference.md)\n  \
             - [Glossary](reference/20261012T090000--glossary.md)\n\
             - [2026-10-10](days/2026-10-10.md)\n  \
             - [Weekly Sync](notes/20261010T080000--weekly-sync.md)\n\n"
        );
        assert_eq!(
            reference_page(&references),
            "# Reference\n\n- [Glossary](reference/20261012T090000--glossary.md)\n"
        );
    }

    #[test]
    fn test_region_without_references_unchanged() {
        let meeting = test_helper_item(
            "20261010T080000",
            "Weekly Sync",
            Kind::Manual,
            "notes/20261010T080000--weekly-sync.md",
        );
        let days = [Day {
            date: "2026-10-10".to_string(),
            entries: vec![(0, &meeting)],
        }];
        assert_eq!(
            render_region(&[], &days),
            "\n- [2026-10-10](days/2026-10-10.md)\n  \
             - [Weekly Sync](notes/20261010T080000--weekly-sync.md)\n\n"
        );
        assert_eq!(render_region(&[], &[]), "");
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
