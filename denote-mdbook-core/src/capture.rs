//! Creating notes, importing Zoom output, and adding attachments.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::frontmatter::{FrontMatter, Kind};
use crate::naming::{self, DenoteName};
use crate::notebook::{self, Entry, Notebook};

/// Options shared by `new` and `import`.
#[derive(Debug, Default)]
pub struct NoteOptions {
    pub title: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub signature: Option<String>,
    pub parent: Option<String>,
    pub date: Option<String>,
    /// Zoom meeting UUID recorded in front matter by `zoom import`.
    pub zoom_meeting: Option<String>,
}

/// What `import` reads from.
pub enum Source<'a> {
    File(&'a Path),
    Stdin,
    /// Text supplied directly; `file_name` (if any) decides verbatim
    /// handling the same way a file's extension does.
    Content {
        text: &'a str,
        file_name: Option<&'a str>,
    },
}

impl<'a> Source<'a> {
    /// The source's file name, used to read a Zoom timestamp from it.
    pub fn file_name(&self) -> Option<&'a str> {
        match self {
            Source::File(p) => p.file_name().and_then(|n| n.to_str()),
            Source::Content { file_name, .. } => *file_name,
            Source::Stdin => None,
        }
    }
}

/// Create a note and return its path.
pub fn create_note(
    nb: &Notebook,
    kind: Kind,
    opts: &NoteOptions,
    body: &str,
    source_file_name: Option<&str>,
    imported: bool,
) -> Result<PathBuf> {
    let parent: Option<Entry> = opts
        .parent
        .as_deref()
        .map(|id| nb.find_note(id))
        .transpose()?;

    let (default_title, default_keywords) = match (&parent, kind) {
        (Some(p), Kind::Summary | Kind::Transcript) => {
            let suffix = if kind == Kind::Summary {
                "AI Summary"
            } else {
                "Transcript"
            };
            let extra = if kind == Kind::Summary {
                "zoomsummary"
            } else {
                "zoomtranscript"
            };
            let mut kw = p.name.keywords.clone();
            kw.push(extra.to_string());
            (Some(format!("{} {suffix}", p.title())), kw)
        }
        _ => (None, Vec::new()),
    };
    let title = opts.title.clone().or(default_title).unwrap_or_default();
    let keywords = naming::normalize_keywords(opts.keywords.clone().unwrap_or(default_keywords));

    let now = naming::now()?;
    let start = naming::resolve_timestamp(
        opts.date.as_deref(),
        parent.as_ref().map(|p| p.name.id.as_str()),
        source_file_name,
        &now,
    )?;
    let id = nb.allocate_id(start);
    let id_time = naming::parse_id(&id).context("allocated an invalid identifier")?;

    let name = DenoteName::new(
        &id,
        opts.signature.as_deref().unwrap_or(""),
        &title,
        &keywords,
        "md",
    );
    let path = nb.kind_dir(kind)?.join(name.file_name());

    let front = FrontMatter {
        title: title.clone(),
        date: naming::to_rfc3339_local(&id_time),
        tags: keywords,
        identifier: id.clone(),
        kind: kind.as_str().to_string(),
        parent: parent.as_ref().map(|p| p.name.id.clone()),
        imported: imported.then(|| naming::to_rfc3339_local(&now.naive_local())),
        zoom_meeting: opts.zoom_meeting.clone(),
    };
    let heading = if title.is_empty() { &id } else { &title };
    let mut content = format!("{}\n# {heading}\n", front.render());
    if !body.is_empty() {
        content.push('\n');
        content.push_str(body);
        if !body.ends_with('\n') {
            content.push('\n');
        }
    }

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, content.as_bytes()))
        .with_context(|| format!("cannot create {}", path.display()))?;

    if let Some(p) = &parent {
        attach(p, &path, heading)?;
    }
    Ok(path)
}

/// Append a link to `child` under `## Attachments` in the parent note.
fn attach(parent: &Entry, child: &Path, child_title: &str) -> Result<()> {
    let content = fs::read_to_string(&parent.path)
        .with_context(|| format!("cannot read {}", parent.path.display()))?;
    let line = format!(
        "- [{}]({})",
        escape_link_text(child_title),
        notebook::relative(parent.dir(), child)
    );
    notebook::safe_write(&parent.path, &insert_attachment(&content, &line))
}

/// Insert `line` at the end of the `## Attachments` section, creating it at
/// the end of the note when absent.
pub fn insert_attachment(content: &str, line: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let Some(start) = lines.iter().position(|l| l.trim_end() == "## Attachments") else {
        let mut out = content.trim_end().to_string();
        out.push_str("\n\n## Attachments\n\n");
        out.push_str(line);
        out.push('\n');
        return out;
    };
    let section_end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("# ") || l.starts_with("## "))
        .map_or(lines.len(), |i| start + 1 + i);
    let mut insert_at = section_end;
    while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() {
        insert_at -= 1;
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    if insert_at == start + 1 {
        out.insert(insert_at, String::new());
        insert_at += 1;
    }
    out.insert(insert_at, line.to_string());
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

pub fn escape_link_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Read an import source as UTF-8, wrap plain text and VTT in a fence, and
/// turn bare URLs in Markdown into links.
pub fn import_body(source: &Source) -> Result<String> {
    let bytes = match source {
        Source::File(p) => fs::read(p).with_context(|| format!("cannot read {}", p.display()))?,
        Source::Stdin => {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf)?;
            buf
        }
        Source::Content { text, .. } => text.as_bytes().to_vec(),
    };
    let text = String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("input is not valid UTF-8"))?;
    let verbatim = source
        .file_name()
        .map(Path::new)
        .and_then(Path::extension)
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("txt") || e.eq_ignore_ascii_case("vtt"));
    Ok(if verbatim {
        fence(&text)
    } else {
        crate::links::linkify_bare_urls(&text).0
    })
}

/// Wrap text in a `text` code fence longer than any backtick run inside it.
fn fence(text: &str) -> String {
    let mut longest = 0usize;
    let mut run = 0usize;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let ticks = "`".repeat(longest.max(2) + 1);
    let body = text.trim_end_matches('\n');
    format!("{ticks}text\n{body}\n{ticks}\n")
}

/// Copy a supported file into `src/<ext>/` and return (path, Markdown reference).
pub fn add_attachment(
    nb: &Notebook,
    file: &Path,
    title: Option<&str>,
    keywords: &[String],
    date: Option<&str>,
    for_note: Option<&str>,
) -> Result<(PathBuf, String)> {
    let ext = attachment_extension(file)?;
    if !file.is_file() {
        bail!("{} does not exist", file.display());
    }
    let from_dir = match for_note {
        Some(id) => nb.find_note(id)?.dir().to_path_buf(),
        None => nb.kind_dir(Kind::Manual)?,
    };
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = title.map_or(stem, str::to_string);

    let now = naming::now()?;
    let start = naming::resolve_timestamp(date, None, None, &now)?;
    let id = nb.allocate_id(start);
    let name = DenoteName::new(&id, "", &title, keywords, &ext);
    let dest = nb.attachment_dir(&ext)?.join(name.file_name());
    fs::copy(file, &dest).with_context(|| format!("cannot copy to {}", dest.display()))?;
    let reference = markdown_reference(&title, &notebook::relative(&from_dir, &dest), &ext);
    Ok((dest, reference))
}

/// The lowercase extension of `file` if it is a supported attachment type.
fn attachment_extension(file: &Path) -> Result<String> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if !naming::is_attachment_ext(ext) {
        bail!(
            "{} is not a supported attachment; supported extensions: {}",
            file.display(),
            naming::ATTACHMENT_EXTENSIONS.join(", ")
        );
    }
    Ok(ext.to_ascii_lowercase())
}

/// `![text](path)` for renderable attachments, `[text](path)` otherwise.
pub fn markdown_reference(text: &str, path: &str, ext: &str) -> String {
    let bang = if naming::is_renderable_ext(ext) {
        "!"
    } else {
        ""
    };
    format!("{bang}[{}]({path})", escape_link_text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_reference_by_renderability() {
        assert_eq!(
            markdown_reference("Data Flow", "../svg/a.svg", "svg"),
            "![Data Flow](../svg/a.svg)"
        );
        assert_eq!(
            markdown_reference("Vendor [Contract]", "../pdf/b.pdf", "pdf"),
            "[Vendor \\[Contract\\]](../pdf/b.pdf)"
        );
        assert_eq!(
            markdown_reference("Q", "../sql/c.sql", "sql"),
            "[Q](../sql/c.sql)"
        );
    }

    #[test]
    fn test_attachment_section_created() {
        let out = insert_attachment("# Sync\n\nnotes\n", "- [A](a.md)");
        assert_eq!(out, "# Sync\n\nnotes\n\n## Attachments\n\n- [A](a.md)\n");
    }

    #[test]
    fn test_attachment_appended_to_existing_section() {
        let content = "# Sync\n\n## Attachments\n\n- [A](a.md)\n\n## Later\n\nx\n";
        let out = insert_attachment(content, "- [B](b.md)");
        assert_eq!(
            out,
            "# Sync\n\n## Attachments\n\n- [A](a.md)\n- [B](b.md)\n\n## Later\n\nx\n"
        );
    }

    #[test]
    fn test_attachment_inserted_before_heading_without_blank_line() {
        let content = "# Sync\n## Attachments\n- [A](a.md)\n## Later\nx\n";
        let out = insert_attachment(content, "- [B](b.md)");
        assert_eq!(
            out,
            "# Sync\n## Attachments\n- [A](a.md)\n- [B](b.md)\n## Later\nx\n"
        );
    }

    #[test]
    fn test_attachment_added_to_empty_section() {
        let content = "# Sync\n\n## Attachments\n\n## Later\n";
        let out = insert_attachment(content, "- [B](b.md)");
        assert_eq!(out, "# Sync\n\n## Attachments\n\n- [B](b.md)\n\n## Later\n");
    }

    #[test]
    fn test_attachment_section_on_first_line() {
        let out = insert_attachment("## Attachments\n- [A](a.md)\n", "- [B](b.md)");
        assert_eq!(out, "## Attachments\n- [A](a.md)\n- [B](b.md)\n");
    }

    #[test]
    fn test_fence_outgrows_backticks() {
        assert_eq!(fence("a\n"), "```text\na\n```\n");
        assert_eq!(fence("x ```` y"), "`````text\nx ```` y\n`````\n");
    }
}
