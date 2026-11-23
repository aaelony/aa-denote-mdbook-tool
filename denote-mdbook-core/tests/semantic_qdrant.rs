//! `index` and semantic `search` against a real Qdrant, with a fake embedder
//! so no model download is needed. Skipped when `qdrant` is not on PATH.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use aa_denote_mdbook_tool_core::frontmatter::Kind;
use aa_denote_mdbook_tool_core::notebook::Notebook;
use aa_denote_mdbook_tool_core::semantic::qdrant::Qdrant;
use aa_denote_mdbook_tool_core::semantic::{self, Embedder, Query, state};
use anyhow::Result;
use chrono::NaiveDate;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("denote-sem-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A Qdrant process on free ports with storage in a temp dir.
struct QdrantProcess {
    child: Child,
    url: String,
    dir: PathBuf,
}

impl QdrantProcess {
    fn start() -> Option<Self> {
        if Command::new("qdrant")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            eprintln!("skipping: qdrant is not on PATH");
            return None;
        }
        let dir = temp_dir("qdrant");
        let http = free_port();
        let grpc = free_port();
        let child = Command::new("qdrant")
            .current_dir(&dir)
            .env("QDRANT__SERVICE__HTTP_PORT", http.to_string())
            .env("QDRANT__SERVICE__GRPC_PORT", grpc.to_string())
            .env("QDRANT__SERVICE__HOST", "127.0.0.1")
            .env("QDRANT__STORAGE__STORAGE_PATH", dir.join("storage"))
            .env("QDRANT__STORAGE__SNAPSHOTS_PATH", dir.join("snapshots"))
            .env("QDRANT__TELEMETRY_DISABLED", "true")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let url = format!("http://127.0.0.1:{http}");
        let q = Qdrant::new(&url);
        let start = Instant::now();
        while q.ping().is_err() {
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "qdrant did not start"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        Some(Self { child, url, dir })
    }
}

impl Drop for QdrantProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Bag of words hashed into 64 dimensions. Shares words = similar vectors.
struct FakeEmbedder {
    calls: usize,
}

impl FakeEmbedder {
    fn vector(text: &str) -> Vec<f32> {
        let mut v = vec![0f32; 64];
        for t in semantic::sparse::tokenize(text) {
            v[(semantic::sparse::token_index(&t) % 64) as usize] += 1.0;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        v.iter().map(|x| x / norm).collect()
    }
}

impl Embedder for FakeEmbedder {
    fn model(&self) -> &str {
        "fake-bow-64"
    }
    fn dim(&self) -> usize {
        64
    }
    fn embed_passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.calls += texts.len();
        Ok(texts.iter().map(|t| Self::vector(t)).collect())
    }
    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        Ok(Self::vector(query))
    }
}

fn write_note(
    root: &Path,
    folder: &str,
    id: &str,
    slug: &str,
    kind: &str,
    keywords: &[&str],
    body: &str,
) -> PathBuf {
    let dir = root.join("src").join(folder);
    fs::create_dir_all(&dir).unwrap();
    let kw = if keywords.is_empty() {
        String::new()
    } else {
        format!("__{}", keywords.join("_"))
    };
    let path = dir.join(format!("{id}--{slug}{kw}.md"));
    let date = format!(
        "{}-{}-{}T{}:{}:00-07:00",
        &id[0..4],
        &id[4..6],
        &id[6..8],
        &id[9..11],
        &id[11..13]
    );
    let tags: Vec<String> = keywords.iter().map(|k| format!("\"{k}\"")).collect();
    let content = format!(
        "+++\ntitle = \"{slug}\"\ndate = {date}\ntags = [{}]\nidentifier = \"{id}\"\nkind = \"{kind}\"\n+++\n\n# {slug}\n\n{body}\n",
        tags.join(", ")
    );
    fs::write(&path, content).unwrap();
    path
}

fn notebook() -> (Notebook, PathBuf) {
    let root = temp_dir("book");
    fs::write(root.join("book.toml"), "[book]\ntitle = \"t\"\n").unwrap();
    write_note(
        &root,
        "notes",
        "20261001T090000",
        "acme-sync",
        "manual",
        &["acme", "meeting"],
        "## Rollout\n\nWe should progressively roll it out to customers.\n\n## Versions\n\nShipped in 4.45.1.0 last week.",
    );
    write_note(
        &root,
        "notes",
        "20261105T100000",
        "budget-review",
        "manual",
        &["finance"],
        "## Budget\n\nThe budget for Q4 is approved.",
    );
    write_note(
        &root,
        "notes/zoom-transcripts",
        "20261105T100002",
        "budget-review-transcript",
        "transcript",
        &["meeting"],
        "```text\n10:00:00 --> 10:00:05\nAlex: the budget looks fine to me.\n```",
    );
    let nb = Notebook::at(&root).unwrap();
    (nb, root)
}

fn query(text: &str) -> Query {
    Query {
        text: text.to_string(),
        limit: 10,
        ..Default::default()
    }
}

#[test]
fn test_index_and_search_roundtrip() {
    let Some(q) = QdrantProcess::start() else {
        return;
    };
    let client = Qdrant::new(&q.url);
    let (nb, root) = notebook();
    let mut emb = FakeEmbedder { calls: 0 };

    let report = semantic::index(&nb, &client, &mut emb, false).unwrap();
    assert_eq!(
        (report.indexed, report.unchanged, report.removed),
        (3, 0, 0)
    );
    assert!(state::exists(&nb));

    // Exact version string finds the note that has it.
    let hits = semantic::search(&nb, &client, &mut emb, &query("4.45.1.0")).unwrap();
    assert_eq!(hits[0].identifier, "20261001T090000");
    assert_eq!(hits[0].heading, "Versions");
    assert!(hits[0].snippet.contains("4.45.1.0"));
    assert_eq!(
        hits[0].path,
        "src/notes/20261001T090000--acme-sync__acme_meeting.md"
    );

    // Transcripts are left out unless asked for.
    let hits = semantic::search(&nb, &client, &mut emb, &query("budget")).unwrap();
    assert!(hits.iter().all(|h| h.kind != "transcript"), "{hits:?}");
    let mut all = query("budget");
    all.all_kinds = true;
    let hits = semantic::search(&nb, &client, &mut emb, &all).unwrap();
    assert!(hits.iter().any(|h| h.kind == "transcript"));
    let mut only = query("budget");
    only.kind = Some(Kind::Transcript);
    let hits = semantic::search(&nb, &client, &mut emb, &only).unwrap();
    assert!(!hits.is_empty() && hits.iter().all(|h| h.kind == "transcript"));

    // Keyword and date filters.
    let mut kw = query("budget rollout");
    kw.keywords = vec!["acme".into()];
    let hits = semantic::search(&nb, &client, &mut emb, &kw).unwrap();
    assert!(!hits.is_empty() && hits.iter().all(|h| h.identifier == "20261001T090000"));
    let mut since = query("budget rollout");
    since.since = NaiveDate::from_ymd_opt(2026, 11, 1);
    let hits = semantic::search(&nb, &client, &mut emb, &since).unwrap();
    assert!(
        !hits.is_empty() && hits.iter().all(|h| h.date.starts_with("2026-11")),
        "{hits:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_index_skips_unchanged_and_removes_deleted() {
    let Some(q) = QdrantProcess::start() else {
        return;
    };
    let client = Qdrant::new(&q.url);
    let (nb, root) = notebook();
    let mut emb = FakeEmbedder { calls: 0 };
    semantic::index(&nb, &client, &mut emb, false).unwrap();

    // Nothing changed: nothing embedded.
    emb.calls = 0;
    let report = semantic::index(&nb, &client, &mut emb, false).unwrap();
    assert_eq!((report.indexed, report.unchanged), (0, 3));
    assert_eq!(emb.calls, 0);

    // One note edited, one removed.
    let edited = write_note(
        &root,
        "notes",
        "20261105T100000",
        "budget-review",
        "manual",
        &["finance"],
        "## Budget\n\nThe budget moved to Snowflake credits.",
    );
    assert!(edited.is_file());
    fs::remove_file(root.join("src/notes/20261001T090000--acme-sync__acme_meeting.md")).unwrap();
    let report = semantic::index(&nb, &client, &mut emb, false).unwrap();
    assert_eq!(
        (report.indexed, report.unchanged, report.removed),
        (1, 1, 1)
    );

    let hits = semantic::search(&nb, &client, &mut emb, &query("4.45.1.0 rollout")).unwrap();
    assert!(
        hits.iter().all(|h| h.identifier != "20261001T090000"),
        "{hits:?}"
    );
    let hits = semantic::search(&nb, &client, &mut emb, &query("snowflake")).unwrap();
    assert_eq!(hits[0].identifier, "20261105T100000");

    // Rebuild embeds everything again.
    emb.calls = 0;
    let report = semantic::index(&nb, &client, &mut emb, true).unwrap();
    assert_eq!(report.indexed, 2);
    assert!(emb.calls > 0);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_index_refuses_a_different_model_without_rebuild() {
    let Some(q) = QdrantProcess::start() else {
        return;
    };
    let client = Qdrant::new(&q.url);
    let (nb, root) = notebook();
    semantic::index(&nb, &client, &mut FakeEmbedder { calls: 0 }, false).unwrap();

    struct Other(FakeEmbedder);
    impl Embedder for Other {
        fn model(&self) -> &str {
            "other"
        }
        fn dim(&self) -> usize {
            64
        }
        fn embed_passages(&mut self, t: &[String]) -> Result<Vec<Vec<f32>>> {
            self.0.embed_passages(t)
        }
        fn embed_query(&mut self, q: &str) -> Result<Vec<f32>> {
            self.0.embed_query(q)
        }
    }
    let err =
        semantic::index(&nb, &client, &mut Other(FakeEmbedder { calls: 0 }), false).unwrap_err();
    assert!(format!("{err:#}").contains("index --rebuild"), "{err:#}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_search_without_index_says_run_index() {
    let Some(q) = QdrantProcess::start() else {
        return;
    };
    let client = Qdrant::new(&q.url);
    let (nb, root) = notebook();
    let err =
        semantic::search(&nb, &client, &mut FakeEmbedder { calls: 0 }, &query("x")).unwrap_err();
    assert!(format!("{err:#}").contains("run `index` first"), "{err:#}");
    let _ = fs::remove_dir_all(&root);
}
