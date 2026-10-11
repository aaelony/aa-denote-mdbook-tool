//! Relative links between notes and images, resolved by Denote identifier.

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pulldown_cmark::{Event, Parser, Tag};

use crate::capture::escape_link_text;
use crate::frontmatter::{self, FrontMatter};
use crate::naming::{self, DenoteName};
use crate::notebook::{self, Entry, Notebook};

/// A link destination found in a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Byte range of the destination text in the file.
    pub range: Range<usize>,
    pub dest: String,
    pub line: usize,
}

/// Find inline links and images whose target is a `.md`/`.png` path or a
/// `denote:` identifier. Code blocks and external URLs are skipped.
pub fn extract(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for (event, range) in Parser::new(text).into_offset_iter() {
        let dest = match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => dest_url,
            _ => continue,
        };
        let dest = dest.to_string();
        if !is_local_target(&dest) {
            continue;
        }
        let slice = &text[range.clone()];
        // The destination is the last occurrence of its text in an inline link.
        let Some(rel) = slice.rfind(dest.as_str()) else {
            continue;
        };
        let start = range.start + rel;
        let line = 1 + text[..start].matches('\n').count();
        out.push(Link {
            range: start..start + dest.len(),
            dest,
            line,
        });
    }
    out
}

fn is_local_target(dest: &str) -> bool {
    if dest.starts_with("denote:") {
        return true;
    }
    if dest.contains("://") || dest.starts_with('#') || dest.starts_with("mailto:") {
        return false;
    }
    let path = split_fragment(dest).0;
    path.ends_with(".md") || path.ends_with(".png")
}

fn split_fragment(dest: &str) -> (&str, &str) {
    match dest.find('#') {
        Some(i) => (&dest[..i], &dest[i..]),
        None => (dest, ""),
    }
}

/// Where a link points.
pub enum Resolution {
    /// Target exists at the written path.
    Valid(PathBuf),
    /// Target is missing (or a `denote:` link) but its identifier resolves.
    Fixable(PathBuf),
    Broken,
}

pub struct Index {
    by_id: HashMap<String, Entry>,
}

impl Index {
    pub fn new(nb: &Notebook) -> Self {
        let by_id = nb
            .scan()
            .into_iter()
            .map(|e| (e.name.id.clone(), e))
            .collect();
        Self { by_id }
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.by_id.get(id)
    }

    pub fn resolve(&self, file_dir: &Path, dest: &str) -> Resolution {
        if let Some(id) = dest.strip_prefix("denote:") {
            return self
                .get(id)
                .map_or(Resolution::Broken, |e| Resolution::Fixable(e.path.clone()));
        }
        let path = split_fragment(dest).0;
        let target = notebook::normalize(&file_dir.join(path));
        if target.is_file() {
            return Resolution::Valid(target);
        }
        let file_name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        match naming::leading_identifier(file_name).and_then(|id| self.get(id)) {
            Some(e) => Resolution::Fixable(e.path.clone()),
            None => Resolution::Broken,
        }
    }
}

/// One problem found by `links check`.
pub struct Problem {
    pub file: PathBuf,
    pub line: usize,
    pub dest: String,
    pub fixable_id: Option<String>,
}

pub fn check(nb: &Notebook) -> Result<Vec<Problem>> {
    let index = Index::new(nb);
    let mut problems = Vec::new();
    for file in nb.markdown_files() {
        let text = fs::read_to_string(&file)?;
        let dir = file.parent().unwrap_or(Path::new("."));
        for link in extract(&text) {
            let fixable_id = match index.resolve(dir, &link.dest) {
                Resolution::Valid(_) => continue,
                Resolution::Fixable(p) => p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(naming::leading_identifier)
                    .map(str::to_string),
                Resolution::Broken => None,
            };
            problems.push(Problem {
                file: file.clone(),
                line: link.line,
                dest: link.dest,
                fixable_id,
            });
        }
    }
    Ok(problems)
}

/// Result of `links fix`.
#[derive(Default)]
pub struct FixReport {
    /// (file, line, old dest, new dest)
    pub fixed: Vec<(PathBuf, usize, String, String)>,
    /// (file, line, dest)
    pub unresolved: Vec<(PathBuf, usize, String)>,
}

/// Rewrite every fixable link to its target's current relative path.
pub fn fix(nb: &Notebook) -> Result<FixReport> {
    let index = Index::new(nb);
    let mut report = FixReport::default();
    for file in nb.markdown_files() {
        let mut text = fs::read_to_string(&file)?;
        let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut edits = Vec::new();
        for link in extract(&text) {
            match index.resolve(&dir, &link.dest) {
                Resolution::Valid(_) => {}
                Resolution::Fixable(target) => {
                    let fragment = if link.dest.starts_with("denote:") {
                        ""
                    } else {
                        split_fragment(&link.dest).1
                    };
                    let new = format!("{}{fragment}", notebook::relative(&dir, &target));
                    report
                        .fixed
                        .push((file.clone(), link.line, link.dest.clone(), new.clone()));
                    edits.push((link.range, new));
                }
                Resolution::Broken => {
                    report.unresolved.push((file.clone(), link.line, link.dest));
                }
            }
        }
        if edits.is_empty() {
            continue;
        }
        // Apply back to front so earlier ranges stay valid.
        edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
        for (range, new) in edits {
            text.replace_range(range, &new);
        }
        notebook::safe_write(&file, &text)?;
    }
    Ok(report)
}

/// Notes that link to the note or image with identifier `id`.
pub fn backlinks(nb: &Notebook, id: &str) -> Result<Vec<PathBuf>> {
    let index = Index::new(nb);
    let target = index
        .get(id)
        .with_context(|| format!("no note or image with identifier {id}"))?;
    let target_path = target.path.clone();
    let mut out = Vec::new();
    // Only notes count; SUMMARY.md and README.md are book plumbing.
    let notes = nb.scan().into_iter().filter(Entry::is_note).map(|e| e.path);
    for file in notes {
        if file == target_path {
            continue;
        }
        let text = fs::read_to_string(&file)?;
        let dir = file.parent().unwrap_or(Path::new("."));
        let links_here = extract(&text)
            .iter()
            .any(|l| match index.resolve(dir, &l.dest) {
                Resolution::Valid(p) => p == target_path,
                Resolution::Fixable(p) => l.dest.starts_with("denote:") && p == target_path,
                Resolution::Broken => false,
            });
        if links_here {
            out.push(file);
        }
    }
    Ok(out)
}

/// A Markdown link to `target_id`, relative to the `from` note (or to
/// `src/notes/`).
pub fn make_link(nb: &Notebook, target_id: &str, from: Option<&str>) -> Result<String> {
    let target = nb.find_id(target_id)?;
    let from_dir = match from {
        Some(id) => nb.find_note(id)?.dir().to_path_buf(),
        None => nb.src().join("notes"),
    };
    let text = if target.is_note() {
        target.title()
    } else {
        target.name.title.clone()
    };
    let text = if text.is_empty() {
        target.name.id.clone()
    } else {
        text
    };
    Ok(format!(
        "[{}]({})",
        escape_link_text(&text),
        notebook::relative(&from_dir, &target.path)
    ))
}

/// Append `line` as its own paragraph at the end of `path`.
pub fn append_line(path: &Path, line: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let mut out = content.trim_end().to_string();
    out.push_str("\n\n");
    out.push_str(line);
    out.push('\n');
    notebook::safe_write(path, &out)
}

/// Rename a note's title, keywords, or signature, keeping its identifier and
/// folder, then repair inbound links. Returns the new path.
pub fn rename(
    nb: &Notebook,
    id: &str,
    title: Option<&str>,
    keywords: Option<&[String]>,
    signature: Option<&str>,
) -> Result<PathBuf> {
    let entry = nb.find_note(id)?;
    if title.is_none() && keywords.is_none() && signature.is_none() {
        bail!("nothing to rename: pass --title, --keywords, or --signature");
    }
    let content = fs::read_to_string(&entry.path)?;
    let front = FrontMatter::parse(&content).unwrap_or_default();
    let old_title = if front.title.is_empty() {
        entry.name.title.clone()
    } else {
        front.title
    };

    let new_title = title.map_or(old_title.clone(), str::to_string);
    let new_keywords = keywords.map_or(entry.name.keywords.clone(), naming::normalize_keywords);
    let new_signature = signature.map_or(entry.name.signature.clone(), str::to_string);
    let name = DenoteName::new(id, &new_signature, &new_title, &new_keywords, "md");
    let new_path = entry.dir().join(name.file_name());
    if new_path != entry.path && new_path.exists() {
        bail!("{} already exists", new_path.display());
    }

    let mut updated = content.clone();
    if frontmatter::split(&updated).is_some() {
        updated = frontmatter::set_field(&updated, "title", &frontmatter::toml_str(&new_title));
        updated = frontmatter::set_field(
            &updated,
            "tags",
            &frontmatter::toml_str_array(&new_keywords),
        );
    }
    if title.is_some() {
        updated = replace_first_heading(&updated, &new_title);
    }

    notebook::safe_write(&new_path, &updated)?;
    if new_path != entry.path {
        fs::remove_file(&entry.path)
            .with_context(|| format!("cannot remove {}", entry.path.display()))?;
    }
    fix(nb)?;
    Ok(new_path)
}

/// Replace the first `# ` heading after the front matter.
fn replace_first_heading(content: &str, title: &str) -> String {
    let body = frontmatter::strip(content);
    let head = &content[..content.len() - body.len()];
    let mut done = false;
    let mut lines: Vec<String> = Vec::new();
    for line in body.lines() {
        if !done && line.starts_with("# ") {
            lines.push(format!("# {title}"));
            done = true;
        } else {
            lines.push(line.to_string());
        }
    }
    let mut out = format!("{head}{}", lines.join("\n"));
    if body.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracts_local_links_only() {
        let text = "See [a](../a.md) and ![p](../png/x.png).\n\
                    [web](https://example.com/x.md) [d](denote:20261010T080000)\n\
                    [anchor](#top) [frag](b.md#sec)\n\
                    ```\n[in code](skip.md)\n```\n\
                    `[inline code](skip2.md)`\n";
        let dests: Vec<String> = extract(text).into_iter().map(|l| l.dest).collect();
        assert_eq!(
            dests,
            vec![
                "../a.md",
                "../png/x.png",
                "denote:20261010T080000",
                "b.md#sec"
            ]
        );
    }

    /// `.md` is also Moldova's top-level domain, so an email address can end
    /// in `.md` without being a note.
    #[test]
    fn test_mailto_ending_in_md_is_not_local() {
        assert!(!is_local_target("mailto:info@gov.md"));
    }

    #[test]
    fn test_ranges_and_lines() {
        let text = "x\n[a](one.md)\n";
        let links = extract(text);
        assert_eq!(links.len(), 1);
        assert_eq!(&text[links[0].range.clone()], "one.md");
        assert_eq!(links[0].line, 2);
    }

    #[test]
    fn test_heading_replaced_once() {
        let c = "+++\ntitle = \"A\"\n+++\n\n# A\n\n# Other\n";
        assert_eq!(
            replace_first_heading(c, "B"),
            "+++\ntitle = \"A\"\n+++\n\n# B\n\n# Other\n"
        );
    }
}
