//! Notebook root discovery, layout, and the scan of notes and attachments.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{NaiveDateTime, TimeDelta};
use walkdir::WalkDir;

use crate::frontmatter::{FrontMatter, Kind};
use crate::naming::{self, DenoteName};

pub const BEGIN_MARKER: &str = "<!-- aa-denote-mdbook-tool:begin -->";
pub const END_MARKER: &str = "<!-- aa-denote-mdbook-tool:end -->";

pub struct Notebook {
    pub root: PathBuf,
}

/// A note or attachment found under `src/`.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: DenoteName,
}

impl Entry {
    pub fn is_note(&self) -> bool {
        self.name.ext == "md"
    }

    pub fn front(&self) -> Option<FrontMatter> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|c| FrontMatter::parse(&c))
    }

    /// Title from front matter, falling back to the identifier.
    pub fn title(&self) -> String {
        self.front()
            .map(|f| f.title)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| self.name.id.clone())
    }

    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }
}

impl Notebook {
    /// Locate the root: `--book`, then `AA_DENOTE_MDBOOK_TOOL_DIR`, then the nearest
    /// ancestor of the current directory that holds `book.toml`. `program` is
    /// the command name used in the hint when no notebook is found.
    pub fn find(book: Option<&Path>, program: &str) -> Result<Self> {
        let explicit = book
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("AA_DENOTE_MDBOOK_TOOL_DIR").map(PathBuf::from));
        if let Some(dir) = explicit {
            if !dir.join("book.toml").is_file() {
                bail!("{} is not a notebook (no book.toml)", dir.display());
            }
            return Self::at(&dir);
        }
        let cwd = std::env::current_dir()?;
        for dir in cwd.ancestors() {
            if dir.join("book.toml").is_file() {
                return Self::at(dir);
            }
        }
        bail!(
            "no notebook found: run `{program} init`, pass --book <DIR>, \
             or set AA_DENOTE_MDBOOK_TOOL_DIR"
        )
    }

    pub fn at(dir: &Path) -> Result<Self> {
        let root = dir
            .canonicalize()
            .with_context(|| format!("cannot open {}", dir.display()))?;
        Ok(Self { root })
    }

    pub fn src(&self) -> PathBuf {
        self.root.join("src")
    }

    /// Folder for a note kind, created if missing.
    pub fn kind_dir(&self, kind: Kind) -> Result<PathBuf> {
        let d = self.src().join(kind.folder());
        fs::create_dir_all(&d).with_context(|| format!("cannot create {}", d.display()))?;
        Ok(d)
    }

    /// The `src/<ext>/` folder for an attachment type, created if missing.
    pub fn attachment_dir(&self, ext: &str) -> Result<PathBuf> {
        let d = self.src().join(ext);
        fs::create_dir_all(&d).with_context(|| format!("cannot create {}", d.display()))?;
        Ok(d)
    }

    /// Every Denote-named note and attachment under `src/`.
    pub fn scan(&self) -> Vec<Entry> {
        let mut out: Vec<Entry> = WalkDir::new(self.src())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let name = DenoteName::parse(e.file_name().to_str()?)?;
                Some(Entry {
                    path: e.into_path(),
                    name,
                })
            })
            .collect();
        out.sort_by(|a, b| a.name.id.cmp(&b.name.id).then(a.path.cmp(&b.path)));
        out
    }

    /// Every Markdown file under `src/`, Denote-named or not.
    pub fn markdown_files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = WalkDir::new(self.src())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        out.sort();
        out
    }

    pub fn find_id(&self, id: &str) -> Result<Entry> {
        self.scan()
            .into_iter()
            .find(|e| e.name.id == id)
            .with_context(|| format!("no note or attachment with identifier {id}"))
    }

    pub fn find_note(&self, id: &str) -> Result<Entry> {
        let e = self.find_id(id)?;
        if !e.is_note() {
            bail!("{id} is an attachment, not a note");
        }
        Ok(e)
    }

    /// The first unused identifier at or after `start`, one second at a time.
    pub fn allocate_id(&self, start: NaiveDateTime) -> String {
        let used: HashSet<String> = self.scan().into_iter().map(|e| e.name.id).collect();
        let mut t = start;
        loop {
            let id = naming::format_id(&t);
            if !used.contains(&id) {
                return id;
            }
            t += TimeDelta::seconds(1);
        }
    }

    /// Path relative to the root, with `/` separators, for display.
    pub fn display(&self, path: &Path) -> String {
        slash(path.strip_prefix(&self.root).unwrap_or(path))
    }

    /// Kind of a note, from the folder it lives in.
    pub fn kind_of(&self, entry: &Entry) -> Kind {
        let dir = entry.dir();
        if dir == self.src().join(Kind::Summary.folder()) {
            Kind::Summary
        } else if dir == self.src().join(Kind::Transcript.folder()) {
            Kind::Transcript
        } else if dir == self.src().join(Kind::Reference.folder()) {
            Kind::Reference
        } else {
            Kind::Manual
        }
    }
}

pub fn slash(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Path of `target` relative to directory `from_dir`, with `/` separators.
pub fn relative(from_dir: &Path, target: &Path) -> String {
    let diff = pathdiff::diff_paths(target, from_dir).unwrap_or_else(|| target.to_path_buf());
    slash(&diff)
}

/// Resolve `.` and `..` lexically, without touching the file system.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Write via a temp file in the same directory, then rename into place, so an
/// interrupted run never leaves a half-written file.
pub fn safe_write(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    fs::write(&tmp, content).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

/// Stylesheet `init` registers in `book.toml`, relative to the notebook root.
pub const FONT_CSS: &str = "theme/font.css";

/// Renders the whole book, code included, in Courier New.
const FONT_CSS_CONTENT: &str = "\
:root {
    --mono-font: \"Courier New\", Courier, monospace;
}

html,
body {
    font-family: \"Courier New\", Courier, monospace;
}
";

/// Create a new notebook in `dir`. Refuses if `book.toml` already exists and
/// never overwrites existing files. `program` is the command that `book.toml`
/// registers as the mdBook preprocessor.
pub fn init(dir: &Path, title: Option<&str>, program: &str) -> Result<Notebook> {
    let book_toml = dir.join("book.toml");
    if book_toml.exists() {
        bail!(
            "{} already exists; refusing to overwrite",
            book_toml.display()
        );
    }
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let nb = Notebook::at(dir)?;
    let title = match title {
        Some(t) => t.to_string(),
        None => nb
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Notes".to_string()),
    };
    for kind in [
        Kind::Manual,
        Kind::Summary,
        Kind::Transcript,
        Kind::Reference,
    ] {
        nb.kind_dir(kind)?;
    }
    nb.attachment_dir("png")?;
    let theme_dir = nb.root.join("theme");
    fs::create_dir_all(&theme_dir)
        .with_context(|| format!("cannot create {}", theme_dir.display()))?;

    let title_toml = crate::frontmatter::toml_str(&title);
    let files = [
        (
            book_toml,
            format!(
                "[book]\ntitle = {title_toml}\nsrc = \"src\"\n\n\
                 [preprocessor.{program}]\ncommand = \"{program} preprocess\"\n\n\
                 [output.html]\nadditional-css = [\"{FONT_CSS}\"]\n"
            ),
        ),
        (nb.root.join(FONT_CSS), FONT_CSS_CONTENT.to_string()),
        (nb.root.join(".ignore"), "book/\n".to_string()),
        (
            nb.src().join("SUMMARY.md"),
            format!("# Summary\n\n[Introduction](README.md)\n\n{BEGIN_MARKER}\n{END_MARKER}\n"),
        ),
        (
            nb.src().join("README.md"),
            format!("# {title}\n\nNotes managed by {program}.\n"),
        ),
    ];
    for (path, content) in files {
        if !path.exists() {
            fs::write(&path, content)
                .with_context(|| format!("cannot write {}", path.display()))?;
        }
    }
    Ok(nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_helper_entry(nb: &Notebook, folder: &str, file_name: &str) -> Entry {
        Entry {
            path: nb.src().join(folder).join(file_name),
            name: DenoteName::parse(file_name).unwrap(),
        }
    }

    #[test]
    fn test_kind_of_follows_folder() {
        let nb = Notebook {
            root: PathBuf::from("/nb"),
        };
        let name = "20261012T090000--glossary__acme.md";
        let cases = [
            ("reference", Kind::Reference),
            ("notes", Kind::Manual),
            ("notes/zoom-summaries", Kind::Summary),
            ("notes/zoom-transcripts", Kind::Transcript),
        ];
        for (folder, kind) in cases {
            assert_eq!(nb.kind_of(&test_helper_entry(&nb, folder, name)), kind);
        }
    }
}
