//! `mcp`: a Model Context Protocol server over stdio, exposing the notebook
//! as tools that call the same core functions as the CLI commands.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use aa_denote_mdbook_tool_core::capture::{self, NoteOptions, Source};
use aa_denote_mdbook_tool_core::frontmatter::{FrontMatter, Kind};
use aa_denote_mdbook_tool_core::notebook::Notebook;
use aa_denote_mdbook_tool_core::{links, naming, search, summary};

/// Serve `nb` on standard input and output until the client disconnects.
pub fn serve(nb: Notebook) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let service = NotebookServer::new(nb)
            .serve(rmcp::transport::stdio())
            .await?;
        service.waiting().await?;
        Ok(())
    })
}

/// Note kind as accepted by the tools.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum KindParam {
    Manual,
    Summary,
    Transcript,
    Reference,
}

impl From<KindParam> for Kind {
    fn from(k: KindParam) -> Self {
        match k {
            KindParam::Manual => Kind::Manual,
            KindParam::Summary => Kind::Summary,
            KindParam::Transcript => Kind::Transcript,
            KindParam::Reference => Kind::Reference,
        }
    }
}

/// Kinds `import_note` accepts.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ImportKindParam {
    Summary,
    Transcript,
    Reference,
}

impl From<ImportKindParam> for Kind {
    fn from(k: ImportKindParam) -> Self {
        match k {
            ImportKindParam::Summary => Kind::Summary,
            ImportKindParam::Transcript => Kind::Transcript,
            ImportKindParam::Reference => Kind::Reference,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NewNoteParams {
    /// Note title
    title: Option<String>,
    /// Keywords, for example ["acme", "meeting"]
    keywords: Option<Vec<String>>,
    /// Denote signature
    signature: Option<String>,
    /// Identifier of the parent (meeting) note
    parent: Option<String>,
    /// When the content happened: "YYYY-MM-DD HH:MM[:SS]", "YYYY-MM-DD", "HH:MM", or an identifier
    date: Option<String>,
    /// Kind of note (default: manual)
    kind: Option<KindParam>,
}

impl NewNoteParams {
    fn options(&self) -> NoteOptions {
        NoteOptions {
            title: self.title.clone(),
            keywords: self.keywords.clone(),
            signature: self.signature.clone(),
            parent: self.parent.clone(),
            date: self.date.clone(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ImportNoteParams {
    /// Kind of imported note
    kind: ImportKindParam,
    /// File to import; prefer an absolute path (relative paths resolve against the server's directory)
    path: Option<String>,
    /// Text to import instead of a file
    content: Option<String>,
    /// File name the content came from; .txt and .vtt are kept verbatim, and a Zoom timestamp in it sets the date
    file_name: Option<String>,
    /// Note title
    title: Option<String>,
    /// Keywords
    keywords: Option<Vec<String>>,
    /// Denote signature
    signature: Option<String>,
    /// Identifier of the parent (meeting) note
    parent: Option<String>,
    /// When the content happened
    date: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AttachFileParams {
    /// File to copy (png, jpg, svg, pdf, sql, or json); prefer an absolute path
    path: String,
    /// Title (default: the file's name without extension)
    title: Option<String>,
    /// Keywords
    keywords: Option<Vec<String>>,
    /// When the file is from
    date: Option<String>,
    /// Make the reference relative to this note
    for_note: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MakeLinkParams {
    /// Identifier of the note or attachment to link to
    target: String,
    /// Make the link relative to this note
    from: Option<String>,
    /// Append the link to the `from` note instead of only returning it
    #[serde(default)]
    append: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct IdParams {
    /// Note identifier, for example 20261010T080000
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RenameNoteParams {
    /// Identifier of the note to rename
    id: String,
    /// New title
    title: Option<String>,
    /// New keywords
    keywords: Option<Vec<String>>,
    /// New signature
    signature: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchNotesParams {
    /// Regular expression to search for (ripgrep syntax)
    pattern: String,
    /// Only notes with all of these keywords
    keywords: Option<Vec<String>>,
    /// Only notes of this kind
    kind: Option<KindParam>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListNotesParams {
    /// Only notes of this kind
    kind: Option<KindParam>,
    /// Only notes with all of these keywords
    keywords: Option<Vec<String>>,
}

/// A created or renamed note.
#[derive(Debug, Serialize, JsonSchema)]
struct NoteRef {
    id: String,
    /// Path relative to the notebook root
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AttachResult {
    /// Stored path relative to the notebook root
    path: String,
    /// Markdown image or link referencing the file
    reference: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct LinkResult {
    link: String,
    /// Path of the note the link was appended to, if `append` was set
    appended_to: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct BrokenLink {
    file: String,
    line: usize,
    dest: String,
    /// Identifier the link can be repaired to by fix_links
    fixable_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CheckLinksResult {
    broken: Vec<BrokenLink>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FixedLink {
    file: String,
    line: usize,
    old: String,
    new: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct LinkAt {
    file: String,
    line: usize,
    dest: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct FixLinksResult {
    fixed: Vec<FixedLink>,
    unresolved: Vec<LinkAt>,
    /// Bare URLs turned into links
    linkified: Vec<LinkAt>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct TitledPath {
    path: String,
    title: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct BacklinksResult {
    notes: Vec<TitledPath>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SummaryResult {
    /// Path of the regenerated summary relative to the notebook root
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SearchResult {
    /// ripgrep's matching lines as `path:line:text`; empty when nothing matches
    text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct NoteInfo {
    id: String,
    title: String,
    kind: String,
    keywords: Vec<String>,
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ListNotesResult {
    notes: Vec<NoteInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadNoteResult {
    path: String,
    /// Full Markdown, front matter included
    content: String,
}

type ToolResult<T> = std::result::Result<Json<T>, String>;

#[derive(Clone)]
struct NotebookServer {
    notebook: Arc<Mutex<Notebook>>,
    tool_router: ToolRouter<Self>,
}

impl NotebookServer {
    fn new(nb: Notebook) -> Self {
        Self {
            notebook: Arc::new(Mutex::new(nb)),
            tool_router: Self::tool_router(),
        }
    }

    /// Run `f` with the notebook locked, so tool calls never interleave, and
    /// turn an error into the message the CLI prints after `error: `.
    fn with_notebook<T>(&self, f: impl FnOnce(&Notebook) -> Result<T>) -> ToolResult<T> {
        let nb = self
            .notebook
            .lock()
            .map_err(|_| "notebook lock poisoned by an earlier failure".to_string())?;
        f(&nb).map(Json).map_err(|e| format!("{e:#}"))
    }
}

#[tool_router]
impl NotebookServer {
    #[tool(description = "Create a new note and return its identifier and path")]
    async fn new_note(&self, Parameters(p): Parameters<NewNoteParams>) -> ToolResult<NoteRef> {
        self.with_notebook(|nb| {
            let kind = p.kind.map_or(Kind::Manual, Kind::from);
            let path = capture::create_note(nb, kind, &p.options(), "", None, false)?;
            summary::write(nb)?;
            Ok(note_ref(nb, &path))
        })
    }

    #[tool(
        description = "Import a Zoom AI summary, transcript, or reference note from a file `path` or from `content`; pass exactly one"
    )]
    async fn import_note(
        &self,
        Parameters(p): Parameters<ImportNoteParams>,
    ) -> ToolResult<NoteRef> {
        self.with_notebook(|nb| {
            let source = match (&p.path, &p.content) {
                (Some(path), None) => Source::File(Path::new(path)),
                (None, Some(text)) => Source::Content {
                    text,
                    file_name: p.file_name.as_deref(),
                },
                _ => bail!("pass exactly one of path or content"),
            };
            let body = capture::import_body(&source)?;
            let opts = NoteOptions {
                title: p.title.clone(),
                keywords: p.keywords.clone(),
                signature: p.signature.clone(),
                parent: p.parent.clone(),
                date: p.date.clone(),
                ..Default::default()
            };
            let path =
                capture::create_note(nb, p.kind.into(), &opts, &body, source.file_name(), true)?;
            summary::write(nb)?;
            Ok(note_ref(nb, &path))
        })
    }

    #[tool(
        description = "Copy a png, jpg, svg, pdf, sql, or json file into src/<ext>/ and return a Markdown reference (image syntax for png, jpg, svg; link syntax otherwise)"
    )]
    async fn attach_file(
        &self,
        Parameters(p): Parameters<AttachFileParams>,
    ) -> ToolResult<AttachResult> {
        self.with_notebook(|nb| {
            let keywords = p.keywords.clone().unwrap_or_default();
            let (path, reference) = capture::add_attachment(
                nb,
                Path::new(&p.path),
                p.title.as_deref(),
                &keywords,
                p.date.as_deref(),
                p.for_note.as_deref(),
            )?;
            summary::write(nb)?;
            Ok(AttachResult {
                path: nb.display(&path),
                reference,
            })
        })
    }

    #[tool(
        description = "Return a relative Markdown link to a note or attachment; with append and from, also append it to the from note"
    )]
    async fn make_link(&self, Parameters(p): Parameters<MakeLinkParams>) -> ToolResult<LinkResult> {
        self.with_notebook(|nb| {
            let link = links::make_link(nb, &p.target, p.from.as_deref())?;
            let appended_to = match (p.append, p.from.as_deref()) {
                (true, Some(id)) => {
                    let path = nb.find_note(id)?.path;
                    links::append_line(&path, &link)?;
                    summary::write(nb)?;
                    Some(nb.display(&path))
                }
                (true, None) => bail!("append needs from: the note to append the link to"),
                (false, _) => None,
            };
            Ok(LinkResult { link, appended_to })
        })
    }

    #[tool(description = "Report broken links in the notebook")]
    async fn check_links(&self) -> ToolResult<CheckLinksResult> {
        self.with_notebook(|nb| {
            let broken = links::check(nb)?
                .into_iter()
                .map(|p| BrokenLink {
                    file: nb.display(&p.file),
                    line: p.line,
                    dest: p.dest,
                    fixable_id: p.fixable_id,
                })
                .collect();
            Ok(CheckLinksResult { broken })
        })
    }

    #[tool(
        description = "Repair links by identifier, convert denote: links, and turn bare http(s) URLs into links"
    )]
    async fn fix_links(&self) -> ToolResult<FixLinksResult> {
        self.with_notebook(|nb| {
            let report = links::fix(nb)?;
            summary::write(nb)?;
            let at = |(file, line, dest): (PathBuf, usize, String)| LinkAt {
                file: nb.display(&file),
                line,
                dest,
            };
            Ok(FixLinksResult {
                fixed: report
                    .fixed
                    .into_iter()
                    .map(|(file, line, old, new)| FixedLink {
                        file: nb.display(&file),
                        line,
                        old,
                        new,
                    })
                    .collect(),
                unresolved: report.unresolved.into_iter().map(at).collect(),
                linkified: report.linkified.into_iter().map(at).collect(),
            })
        })
    }

    #[tool(description = "List notes that link to a note or attachment")]
    async fn backlinks(&self, Parameters(p): Parameters<IdParams>) -> ToolResult<BacklinksResult> {
        self.with_notebook(|nb| {
            let notes = links::backlinks(nb, &p.id)?
                .into_iter()
                .map(|file| TitledPath {
                    title: std::fs::read_to_string(&file)
                        .ok()
                        .and_then(|c| FrontMatter::parse(&c))
                        .map(|f| f.title)
                        .unwrap_or_default(),
                    path: nb.display(&file),
                })
                .collect();
            Ok(BacklinksResult { notes })
        })
    }

    #[tool(
        description = "Change a note's title, keywords, or signature, keeping its identifier and fixing inbound links"
    )]
    async fn rename_note(
        &self,
        Parameters(p): Parameters<RenameNoteParams>,
    ) -> ToolResult<NoteRef> {
        self.with_notebook(|nb| {
            let path = links::rename(
                nb,
                &p.id,
                p.title.as_deref(),
                p.keywords.as_deref(),
                p.signature.as_deref(),
            )?;
            summary::write(nb)?;
            Ok(note_ref(nb, &path))
        })
    }

    #[tool(description = "Regenerate src/SUMMARY.md and the day pages")]
    async fn regenerate_summary(&self) -> ToolResult<SummaryResult> {
        self.with_notebook(|nb| {
            summary::write(nb)?;
            Ok(SummaryResult {
                path: nb.display(&nb.src().join("SUMMARY.md")),
            })
        })
    }

    #[tool(
        description = "Search notes with ripgrep, optionally only notes with given keywords or kind"
    )]
    async fn search_notes(
        &self,
        Parameters(p): Parameters<SearchNotesParams>,
    ) -> ToolResult<SearchResult> {
        self.with_notebook(|nb| {
            let keywords = p.keywords.clone().unwrap_or_default();
            let found = search::search_captured(nb, &p.pattern, &keywords, p.kind.map(Kind::from))?;
            if found.status > 1 {
                bail!("rg failed: {}", found.stderr.trim());
            }
            Ok(SearchResult { text: found.stdout })
        })
    }

    #[tool(
        description = "List notes (identifier, title, kind, keywords, path), newest first, optionally filtered by kind and keywords"
    )]
    async fn list_notes(
        &self,
        Parameters(p): Parameters<ListNotesParams>,
    ) -> ToolResult<ListNotesResult> {
        self.with_notebook(|nb| {
            let kind = p.kind.map(Kind::from);
            let wanted = naming::normalize_keywords(p.keywords.clone().unwrap_or_default());
            let mut notes: Vec<NoteInfo> = nb
                .scan()
                .into_iter()
                .filter(|e| e.is_note())
                .filter(|e| kind.is_none_or(|k| nb.kind_of(e) == k))
                .filter(|e| wanted.iter().all(|k| e.name.keywords.contains(k)))
                .map(|e| NoteInfo {
                    id: e.name.id.clone(),
                    title: e.title(),
                    kind: nb.kind_of(&e).as_str().to_string(),
                    keywords: e.name.keywords.clone(),
                    path: nb.display(&e.path),
                })
                .collect();
            notes.sort_by(|a, b| b.id.cmp(&a.id));
            Ok(ListNotesResult { notes })
        })
    }

    #[tool(description = "Return a note's full Markdown, front matter included")]
    async fn read_note(&self, Parameters(p): Parameters<IdParams>) -> ToolResult<ReadNoteResult> {
        self.with_notebook(|nb| {
            let entry = nb.find_note(&p.id)?;
            let content = std::fs::read_to_string(&entry.path)?;
            Ok(ReadNoteResult {
                path: nb.display(&entry.path),
                content,
            })
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for NotebookServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Tools for a notebook of Denote-named Markdown notes in an mdBook layout. \
                 Use list_notes and search_notes to find notes, read_note to read one.",
            )
    }
}

fn note_ref(nb: &Notebook, path: &Path) -> NoteRef {
    let id = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(naming::leading_identifier)
        .unwrap_or_default()
        .to_string();
    NoteRef {
        id,
        path: nb.display(path),
    }
}
