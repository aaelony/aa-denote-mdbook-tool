//! Command-line interface: clap definitions and command dispatch.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use aa_denote_mdbook_tool_core::capture::{self, NoteOptions, Source};
use aa_denote_mdbook_tool_core::frontmatter::Kind;
use aa_denote_mdbook_tool_core::notebook::{self, Notebook};
use aa_denote_mdbook_tool_core::{external, frontmatter, links, preprocess, search, summary};

/// The installed command name, from `[[bin]] name` in Cargo.toml.
const BIN: &str = env!("CARGO_BIN_NAME");

#[derive(Parser)]
#[command(version, about = "Denote-style note names in an mdBook layout")]
struct Cli {
    /// Notebook root (default: $AA_DENOTE_MDBOOK_TOOL_DIR, then the nearest ancestor with book.toml)
    #[arg(long, global = true, value_name = "DIR")]
    book: Option<PathBuf>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Args)]
struct CommonNoteArgs {
    /// Note title
    #[arg(long)]
    title: Option<String>,
    /// Comma-separated keywords
    #[arg(long, value_delimiter = ',')]
    keywords: Option<Vec<String>>,
    /// Denote signature
    #[arg(long)]
    signature: Option<String>,
    /// Identifier of the parent (meeting) note
    #[arg(long, value_name = "ID")]
    parent: Option<String>,
    /// When the content happened: "YYYY-MM-DD HH:MM[:SS]", "YYYY-MM-DD", "HH:MM", or an identifier
    #[arg(long, value_name = "WHEN")]
    date: Option<String>,
    /// Open the note in $VISUAL or $EDITOR after creating it
    #[arg(long)]
    edit: bool,
}

impl CommonNoteArgs {
    fn options(&self) -> NoteOptions {
        NoteOptions {
            title: self.title.clone(),
            keywords: self.keywords.clone(),
            signature: self.signature.clone(),
            parent: self.parent.clone(),
            date: self.date.clone(),
        }
    }
}

/// Note kind as accepted on the command line.
#[derive(Clone, Copy, clap::ValueEnum)]
enum KindArg {
    Manual,
    Summary,
    Transcript,
}

impl From<KindArg> for Kind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Manual => Kind::Manual,
            KindArg::Summary => Kind::Summary,
            KindArg::Transcript => Kind::Transcript,
        }
    }
}

/// Kinds `import` accepts.
#[derive(Clone, Copy, clap::ValueEnum)]
enum ImportKind {
    Summary,
    Transcript,
}

impl From<ImportKind> for Kind {
    fn from(k: ImportKind) -> Self {
        match k {
            ImportKind::Summary => Kind::Summary,
            ImportKind::Transcript => Kind::Transcript,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new notebook
    Init {
        /// Directory to create (default: current directory)
        dir: Option<PathBuf>,
        /// Book title (default: the directory name)
        #[arg(long)]
        title: Option<String>,
    },
    /// Create a new note
    New {
        #[command(flatten)]
        common: CommonNoteArgs,
        /// Kind of note
        #[arg(long, value_enum, default_value = "manual")]
        kind: KindArg,
    },
    /// Import a Zoom AI summary or transcript ("-" reads standard input)
    Import {
        file: PathBuf,
        #[arg(long, value_enum)]
        kind: ImportKind,
        #[command(flatten)]
        common: CommonNoteArgs,
    },
    /// Copy a png into src/png and print a Markdown image reference
    Image {
        file: PathBuf,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, value_delimiter = ',')]
        keywords: Option<Vec<String>>,
        #[arg(long, value_name = "WHEN")]
        date: Option<String>,
        /// Print the reference relative to this note
        #[arg(long = "for", value_name = "ID")]
        for_note: Option<String>,
    },
    /// Print a Markdown link to a note or image
    Link {
        target: String,
        /// Make the path relative to this note
        #[arg(long, value_name = "ID")]
        from: Option<String>,
        /// Append the link to the --from note instead of printing it
        #[arg(long, requires = "from")]
        append: bool,
    },
    /// Check, fix, or list links
    Links {
        #[command(subcommand)]
        command: LinksCmd,
    },
    /// Change a note's title, keywords, or signature, keeping its identifier
    Rename {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, value_delimiter = ',')]
        keywords: Option<Vec<String>>,
        #[arg(long)]
        signature: Option<String>,
    },
    /// Regenerate src/SUMMARY.md
    Summary,
    /// Regenerate SUMMARY.md and run `mdbook build`
    Build,
    /// Regenerate SUMMARY.md and run `mdbook serve`
    Serve {
        #[arg(long)]
        open: bool,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Search notes with ripgrep
    Search {
        pattern: String,
        /// Only notes with this keyword (repeatable; all must match)
        #[arg(long = "keyword", value_name = "K")]
        keywords: Vec<String>,
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        /// Extra arguments for rg, after `--`
        #[arg(last = true)]
        rg_args: Vec<String>,
    },
    /// mdBook preprocessor entry point
    #[command(hide = true)]
    Preprocess {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum LinksCmd {
    /// Report broken links; exits non-zero if any
    Check,
    /// Repair links by identifier and convert denote: links
    Fix,
    /// List notes that link to a note or image
    Backlinks { id: String },
}

pub fn main() -> ExitCode {
    let cli = Cli::parse();
    // On stderr so stdout stays clean for paths, links, and search output.
    // Skipped for `preprocess`, which mdBook runs on every build.
    if !matches!(cli.command, Cmd::Preprocess { .. }) {
        eprintln!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    }
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let book = cli.book.as_deref();
    match cli.command {
        Cmd::Init { dir, title } => cmd_init(dir, title.as_deref()),
        Cmd::New { common, kind } => cmd_new(book, &common, kind),
        Cmd::Import { file, kind, common } => cmd_import(book, &file, kind, &common),
        Cmd::Image {
            file,
            title,
            keywords,
            date,
            for_note,
        } => cmd_image(
            book,
            &file,
            title.as_deref(),
            keywords,
            date.as_deref(),
            for_note.as_deref(),
        ),
        Cmd::Link {
            target,
            from,
            append,
        } => cmd_link(book, &target, from.as_deref(), append),
        Cmd::Links { command } => links_command(&Notebook::find(book, BIN)?, command),
        Cmd::Rename {
            id,
            title,
            keywords,
            signature,
        } => cmd_rename(
            book,
            &id,
            title.as_deref(),
            keywords.as_deref(),
            signature.as_deref(),
        ),
        Cmd::Summary => cmd_summary(book),
        Cmd::Build => mdbook(&Notebook::find(book, BIN)?, &["build".to_string()]),
        Cmd::Serve { open, port } => cmd_serve(book, open, port),
        Cmd::Search {
            pattern,
            keywords,
            kind,
            rg_args,
        } => cmd_search(book, &pattern, &keywords, kind, &rg_args),
        Cmd::Preprocess { args } => preprocess::run(&args).map(|()| ExitCode::SUCCESS),
    }
}

fn cmd_init(dir: Option<PathBuf>, title: Option<&str>) -> Result<ExitCode> {
    let dir = dir.unwrap_or_else(|| PathBuf::from("."));
    let nb = notebook::init(&dir, title, BIN)?;
    println!("Created notebook at {}", nb.root.display());
    println!(
        "Note: book.toml runs `{BIN} preprocess`; install with \
         `cargo install --path .` so plain `mdbook` can find it."
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_new(book: Option<&Path>, common: &CommonNoteArgs, kind: KindArg) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let path = capture::create_note(&nb, kind.into(), &common.options(), "", None, false)?;
    finish_note(&nb, &path, common.edit)
}

fn cmd_import(
    book: Option<&Path>,
    file: &Path,
    kind: ImportKind,
    common: &CommonNoteArgs,
) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let source = if file.as_os_str() == "-" {
        Source::Stdin
    } else {
        Source::File(file)
    };
    let body = capture::import_body(&source)?;
    let file_name = match source {
        Source::File(p) => p.file_name().and_then(|n| n.to_str()),
        Source::Stdin => None,
    };
    let path = capture::create_note(&nb, kind.into(), &common.options(), &body, file_name, true)?;
    finish_note(&nb, &path, common.edit)
}

fn cmd_image(
    book: Option<&Path>,
    file: &Path,
    title: Option<&str>,
    keywords: Option<Vec<String>>,
    date: Option<&str>,
    for_note: Option<&str>,
) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let keywords = keywords.unwrap_or_default();
    let (_, reference) = capture::add_image(&nb, file, title, &keywords, date, for_note)?;
    println!("{reference}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_link(
    book: Option<&Path>,
    target: &str,
    from: Option<&str>,
    append: bool,
) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let link = links::make_link(&nb, target, from)?;
    match (append, from) {
        (true, Some(id)) => links::append_line(&nb.find_note(id)?.path, &link)?,
        _ => println!("{link}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_rename(
    book: Option<&Path>,
    id: &str,
    title: Option<&str>,
    keywords: Option<&[String]>,
    signature: Option<&str>,
) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let path = links::rename(&nb, id, title, keywords, signature)?;
    println!("{}", nb.display(&path));
    Ok(ExitCode::SUCCESS)
}

fn cmd_summary(book: Option<&Path>) -> Result<ExitCode> {
    summary::write(&Notebook::find(book, BIN)?)?;
    Ok(ExitCode::SUCCESS)
}

fn cmd_serve(book: Option<&Path>, open: bool, port: Option<u16>) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let mut args = vec!["serve".to_string()];
    if open {
        args.push("--open".to_string());
    }
    if let Some(p) = port {
        args.push("--port".to_string());
        args.push(p.to_string());
    }
    mdbook(&nb, &args)
}

fn cmd_search(
    book: Option<&Path>,
    pattern: &str,
    keywords: &[String],
    kind: Option<KindArg>,
    rg_args: &[String],
) -> Result<ExitCode> {
    let nb = Notebook::find(book, BIN)?;
    let code = search::search(&nb, pattern, keywords, kind.map(Kind::from), rg_args)?;
    Ok(external::exit(code))
}

fn finish_note(nb: &Notebook, path: &Path, edit: bool) -> Result<ExitCode> {
    println!("{}", nb.display(path));
    if edit {
        external::edit(path)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn links_command(nb: &Notebook, command: LinksCmd) -> Result<ExitCode> {
    match command {
        LinksCmd::Check => {
            let problems = links::check(nb)?;
            for p in &problems {
                let fixable = p
                    .fixable_id
                    .as_ref()
                    .map(|id| format!(" (fixable: {id})"))
                    .unwrap_or_default();
                println!(
                    "{}:{}: broken link {}{fixable}",
                    nb.display(&p.file),
                    p.line,
                    p.dest
                );
            }
            if !problems.is_empty() {
                return Ok(ExitCode::FAILURE);
            }
        }
        LinksCmd::Fix => {
            let report = links::fix(nb)?;
            for (file, line, old, new) in &report.fixed {
                println!("{}:{line}: fixed {old} -> {new}", nb.display(file));
            }
            for (file, line, dest) in &report.unresolved {
                println!("{}:{line}: cannot resolve {dest}", nb.display(file));
            }
        }
        LinksCmd::Backlinks { id } => {
            for file in links::backlinks(nb, &id)? {
                let title = std::fs::read_to_string(&file)
                    .ok()
                    .and_then(|c| frontmatter::FrontMatter::parse(&c))
                    .map(|f| f.title)
                    .unwrap_or_default();
                println!("{}\t{title}", nb.display(&file));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Regenerate SUMMARY.md, then run mdbook.
fn mdbook(nb: &Notebook, args: &[String]) -> Result<ExitCode> {
    Ok(external::exit(summary::run_mdbook(nb, args)?))
}
