//! `index` and `search --semantic`. Without the `semantic` feature, both
//! commands explain how to get a build that has them.

use std::process::ExitCode;

use anyhow::Result;
use chrono::NaiveDate;

use aa_denote_mdbook_tool_core::frontmatter::Kind;
use aa_denote_mdbook_tool_core::notebook::Notebook;
#[cfg(feature = "semantic")]
use aa_denote_mdbook_tool_core::semantic::{Hit, Query};

/// Options of `search --semantic`.
#[cfg_attr(not(feature = "semantic"), allow(dead_code))]
pub struct SemanticArgs {
    pub keywords: Vec<String>,
    pub kind: Option<Kind>,
    pub all_kinds: bool,
    pub since: Option<NaiveDate>,
    pub until: Option<NaiveDate>,
    pub limit: usize,
    pub json: bool,
}

#[cfg(feature = "semantic")]
impl SemanticArgs {
    fn query(&self, text: &str) -> Query {
        Query {
            text: text.to_string(),
            keywords: self.keywords.clone(),
            kind: self.kind,
            all_kinds: self.all_kinds,
            since: self.since,
            until: self.until,
            limit: self.limit,
        }
    }
}

/// One result block for the terminal.
#[cfg(feature = "semantic")]
pub fn format_hit(h: &Hit) -> String {
    let day = h.date.get(..10).unwrap_or(&h.date);
    let place = if h.heading.is_empty() || h.heading == h.title {
        h.title.clone()
    } else {
        format!("{} > {}", h.title, h.heading)
    };
    format!(
        "{day}  {place}  [{}]  (score {:.3})\n  {}\n  {}\n",
        h.kind, h.score, h.path, h.snippet
    )
}

#[cfg(feature = "semantic")]
mod enabled {
    use super::*;
    use aa_denote_mdbook_tool_core::semantic::embed::FastEmbedder;
    use aa_denote_mdbook_tool_core::semantic::qdrant::{self, Qdrant};
    use aa_denote_mdbook_tool_core::semantic::{self, state};

    pub fn index(nb: &Notebook, rebuild: bool) -> Result<ExitCode> {
        let client = Qdrant::new(&qdrant::url_from_env());
        let report = semantic::index(nb, &client, &mut FastEmbedder::new(), rebuild)?;
        println!(
            "indexed {} notes ({} chunks), {} unchanged, {} removed; collection {}",
            report.indexed,
            report.chunks,
            report.unchanged,
            report.removed,
            semantic::collection_name(nb)
        );
        Ok(ExitCode::SUCCESS)
    }

    pub fn search(nb: &Notebook, text: &str, args: &SemanticArgs) -> Result<ExitCode> {
        let client = Qdrant::new(&qdrant::url_from_env());
        let hits = semantic::search(nb, &client, &mut FastEmbedder::new(), &args.query(text))?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&hits)?);
        } else {
            for h in &hits {
                println!("{}", format_hit(h));
            }
        }
        // Same convention as rg: 1 when nothing matched.
        Ok(if hits.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        })
    }

    /// For the MCP server: one embedder for the life of the process, so the
    /// model loads once instead of on every tool call.
    pub fn hits(nb: &Notebook, query: &Query) -> Result<Vec<Hit>> {
        static EMBEDDER: std::sync::OnceLock<std::sync::Mutex<FastEmbedder>> =
            std::sync::OnceLock::new();
        let client = Qdrant::new(&qdrant::url_from_env());
        let mut embedder = EMBEDDER
            .get_or_init(|| std::sync::Mutex::new(FastEmbedder::new()))
            .lock()
            .map_err(|_| anyhow::anyhow!("embedder lock poisoned by an earlier failure"))?;
        semantic::search(nb, &client, &mut *embedder, query)
    }

    /// After a write command: update the index if this notebook has one.
    /// Problems become a warning; the write itself already succeeded.
    pub fn after_write(nb: &Notebook) {
        if !state::exists(nb) {
            return;
        }
        let client = Qdrant::new(&qdrant::url_from_env());
        if let Err(e) = semantic::index(nb, &client, &mut FastEmbedder::new(), false) {
            eprintln!("warning: semantic index not updated: {e:#}");
        }
    }
}

#[cfg(not(feature = "semantic"))]
mod enabled {
    use super::*;
    use anyhow::bail;

    const HINT: &str = "this build has no semantic search: reinstall with \
                        `cargo install --path . --features semantic`";

    pub fn index(_nb: &Notebook, _rebuild: bool) -> Result<ExitCode> {
        bail!(HINT)
    }

    pub fn search(_nb: &Notebook, _text: &str, _args: &SemanticArgs) -> Result<ExitCode> {
        bail!(HINT)
    }

    pub fn after_write(_nb: &Notebook) {}
}

#[cfg(feature = "semantic")]
pub use enabled::hits;
pub use enabled::{after_write, index, search};
