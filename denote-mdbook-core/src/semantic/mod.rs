//! Semantic search: chunk notes, embed them locally, store them in Qdrant,
//! and run hybrid (meaning + exact term) queries.
//!
//! The embedding model sits behind the [`Embedder`] trait. The real one
//! ([`embed::FastEmbedder`]) needs the `semantic` feature; tests use a fake.

pub mod chunk;
#[cfg(feature = "semantic")]
pub mod embed;
pub mod qdrant;
pub mod sparse;
pub mod state;

use std::collections::{BTreeSet, HashMap};
use std::fs;

use anyhow::{Context, Result, bail};
use chrono::{NaiveDate, TimeDelta};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::frontmatter::{self, FrontMatter, Kind};
use crate::naming;
use crate::notebook::{Entry, Notebook};
use qdrant::Qdrant;
use state::{NoteState, State};

/// Chunks embedded and stored per request.
const BATCH: usize = 32;
/// Results kept per note, so one long note cannot fill the list.
const PER_NOTE: usize = 2;
const SNIPPET_CHARS: usize = 240;

/// A text embedding model.
pub trait Embedder {
    /// Model name, stored in the index state.
    fn model(&self) -> &str;
    /// Vector length.
    fn dim(&self) -> usize;
    /// Vectors for document chunks.
    fn embed_passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    /// Vector for a search question.
    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>>;
}

/// Qdrant collection for this notebook: `denote_` plus the directory name,
/// so two copies of a notebook never share a collection.
pub fn collection_name(nb: &Notebook) -> String {
    let dir = nb
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut name: String = dir
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    name = name.trim_matches('_').to_string();
    if name.is_empty() {
        name = "notebook".to_string();
    }
    format!("denote_{name}")
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IndexReport {
    pub indexed: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub chunks: usize,
}

/// Bring the collection in step with the notebook. Only notes whose content
/// changed are embedded again; notes that no longer exist are removed.
pub fn index(
    nb: &Notebook,
    client: &Qdrant,
    embedder: &mut dyn Embedder,
    rebuild: bool,
) -> Result<IndexReport> {
    client.ping()?;
    let collection = collection_name(nb);
    let mut st = State::load(nb)?;
    if !rebuild && !st.notes.is_empty() && st.model != embedder.model() {
        bail!(
            "the index was built with model {} but this build uses {}: run `index --rebuild`",
            st.model,
            embedder.model()
        );
    }
    if rebuild && client.collection_exists(&collection)? {
        client.delete_collection(&collection)?;
    }
    if rebuild || client.ensure_collection(&collection, embedder.dim())? {
        // A new collection holds nothing, whatever the old state says.
        st.notes.clear();
        client.ensure_collection(&collection, embedder.dim())?;
    }
    st.model = embedder.model().to_string();
    st.collection.clone_from(&collection);

    let mut report = IndexReport::default();
    let notes: Vec<Entry> = nb.scan().into_iter().filter(Entry::is_note).collect();
    let present: BTreeSet<String> = notes.iter().map(|e| e.name.id.clone()).collect();

    for entry in &notes {
        let content = fs::read_to_string(&entry.path)
            .with_context(|| format!("cannot read {}", entry.path.display()))?;
        let hash = hex(&Sha256::digest(content.as_bytes()));
        if st.notes.get(&entry.name.id).is_some_and(|n| n.hash == hash) {
            report.unchanged += 1;
            continue;
        }
        let chunks = index_note(nb, client, embedder, &collection, entry, &content)?;
        report.indexed += 1;
        report.chunks += chunks;
        st.notes
            .insert(entry.name.id.clone(), NoteState { hash, chunks });
        st.save(nb)?;
    }

    let gone: Vec<String> = st
        .notes
        .keys()
        .filter(|id| !present.contains(*id))
        .cloned()
        .collect();
    for id in gone {
        client.delete_note(&collection, &id)?;
        st.notes.remove(&id);
        report.removed += 1;
    }
    st.save(nb)?;
    Ok(report)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Stable point id for chunk `n` of a note.
fn point_id(identifier: &str, n: usize) -> u64 {
    let d = Sha256::digest(format!("{identifier}#{n}").as_bytes());
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[..8]);
    // Qdrant ids are unsigned; keep them under 2^63 for JSON tools that use i64.
    u64::from_be_bytes(b) >> 1
}

/// RFC 3339 date of a note: front matter, else its identifier.
fn note_date(front: Option<&FrontMatter>, id: &str) -> String {
    front
        .map(|f| f.date.clone())
        .filter(|d| !d.is_empty())
        .or_else(|| naming::parse_id(id).map(|t| naming::to_rfc3339_local(&t)))
        .unwrap_or_default()
}

/// Replace one note's points. Returns the number of chunks stored.
fn index_note(
    nb: &Notebook,
    client: &Qdrant,
    embedder: &mut dyn Embedder,
    collection: &str,
    entry: &Entry,
    content: &str,
) -> Result<usize> {
    let front = FrontMatter::parse(content);
    let id = &entry.name.id;
    let title = front
        .as_ref()
        .map(|f| f.title.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| id.clone());
    let date = note_date(front.as_ref(), id);
    let day = date.get(..10).unwrap_or("");
    let kind = nb.kind_of(entry);
    let body = frontmatter::strip(content);
    let chunks = chunk::chunk_note(&title, day, body, kind == Kind::Transcript);

    client.delete_note(collection, id)?;
    for (b, batch) in chunks.chunks(BATCH).enumerate() {
        let texts: Vec<String> = batch.iter().map(|c| c.embed_text.clone()).collect();
        let vectors = embedder.embed_passages(&texts)?;
        let points: Vec<_> = batch
            .iter()
            .zip(vectors)
            .enumerate()
            .map(|(i, (c, dense))| {
                let n = b * BATCH + i;
                let payload = json!({
                    "identifier": id,
                    "title": title,
                    "heading": c.heading,
                    "kind": kind.as_str(),
                    "keywords": entry.name.keywords,
                    "date": date,
                    "parent": front.as_ref().and_then(|f| f.parent.clone()),
                    "path": nb.display(&entry.path),
                    "chunk": n,
                    "text": c.text,
                });
                (
                    point_id(id, n),
                    dense,
                    sparse::document_vector(&c.embed_text),
                    payload,
                )
            })
            .collect();
        client.upsert(collection, &points)?;
    }
    Ok(chunks.len())
}

/// A semantic search request.
#[derive(Debug, Clone, Default)]
pub struct Query {
    pub text: String,
    /// All must match.
    pub keywords: Vec<String>,
    pub kind: Option<Kind>,
    /// Include transcripts when no kind is given.
    pub all_kinds: bool,
    pub since: Option<NaiveDate>,
    /// Inclusive.
    pub until: Option<NaiveDate>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Hit {
    pub identifier: String,
    pub title: String,
    pub date: String,
    pub kind: String,
    pub heading: String,
    pub path: String,
    pub snippet: String,
    pub score: f64,
}

fn local_midnight(d: NaiveDate) -> String {
    naming::to_rfc3339_local(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
}

/// The Qdrant filter for a query's keyword, kind and date options.
pub fn filter(q: &Query) -> Value {
    let mut must: Vec<Value> = Vec::new();
    let mut must_not: Vec<Value> = Vec::new();
    for k in naming::normalize_keywords(&q.keywords) {
        must.push(json!({ "key": "keywords", "match": { "value": k } }));
    }
    match q.kind {
        Some(k) => must.push(json!({ "key": "kind", "match": { "value": k.as_str() } })),
        None if !q.all_kinds => must_not.push(json!({
            "key": "kind", "match": { "value": Kind::Transcript.as_str() }
        })),
        None => {}
    }
    let mut range = serde_json::Map::new();
    if let Some(d) = q.since {
        range.insert("gte".into(), json!(local_midnight(d)));
    }
    if let Some(d) = q.until {
        range.insert("lt".into(), json!(local_midnight(d + TimeDelta::days(1))));
    }
    if !range.is_empty() {
        must.push(json!({ "key": "date", "range": range }));
    }
    json!({ "must": must, "must_not": must_not })
}

/// Run a hybrid query and return at most `q.limit` hits, no more than two
/// per note.
pub fn search(
    nb: &Notebook,
    client: &Qdrant,
    embedder: &mut dyn Embedder,
    q: &Query,
) -> Result<Vec<Hit>> {
    let collection = collection_name(nb);
    if !client.collection_exists(&collection)? {
        bail!("this notebook has no semantic index yet: run `index` first");
    }
    let limit = q.limit.max(1);
    let dense = embedder.embed_query(&q.text)?;
    let sparse = sparse::query_vector(&q.text);
    let points = client.hybrid_query(&collection, &dense, &sparse, &filter(q), limit * 4)?;

    let terms = sparse::tokenize(&q.text);
    let mut points = points;
    prefer_exact_terms(&mut points, &terms);
    let mut per_note: HashMap<String, usize> = HashMap::new();
    let mut hits = Vec::new();
    for p in points {
        let text_of = |k: &str| {
            p.payload
                .get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let identifier = text_of("identifier");
        let seen = per_note.entry(identifier.clone()).or_insert(0);
        if *seen >= PER_NOTE {
            continue;
        }
        *seen += 1;
        hits.push(Hit {
            identifier,
            title: text_of("title"),
            date: text_of("date"),
            kind: text_of("kind"),
            heading: text_of("heading"),
            path: text_of("path"),
            snippet: snippet(&text_of("text"), &terms),
            score: p.score,
        });
        if hits.len() == limit {
            break;
        }
    }
    Ok(hits)
}

/// Query terms that only make sense as exact matches: versions, ticket
/// numbers, identifiers (a digit, `_` or `.` inside the token).
fn exact_terms(terms: &[String]) -> Vec<&str> {
    terms
        .iter()
        .map(String::as_str)
        .filter(|t| {
            t.chars()
                .any(|c| c.is_ascii_digit() || c == '_' || c == '.')
        })
        .collect()
}

/// Move points whose text contains every exact-match term ahead of the
/// rest, keeping the fused order within each group. Embedding models treat
/// `4.45.1.0` and `4.46.2.0` as near neighbours; a reader does not.
fn prefer_exact_terms(points: &mut [qdrant::ScoredPoint], terms: &[String]) {
    let exact = exact_terms(terms);
    if exact.is_empty() {
        return;
    }
    let has_all = |p: &qdrant::ScoredPoint| {
        let text = p
            .payload
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        exact.iter().all(|t| text.contains(t))
    };
    // Stable sort: true sorts first via `!has_all`.
    points.sort_by_key(|p| !has_all(p));
}

/// About `SNIPPET_CHARS` characters around the first query term found, on
/// one line.
pub fn snippet(text: &str, terms: &[String]) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = flat.to_lowercase();
    let start = terms
        .iter()
        .filter_map(|t| lower.find(t.as_str()))
        .min()
        .unwrap_or(0);
    let chars: Vec<char> = flat.chars().collect();
    // Byte offset to char offset.
    let start_char = flat[..start.min(flat.len())].chars().count();
    let from = start_char.saturating_sub(SNIPPET_CHARS / 3);
    let to = (from + SNIPPET_CHARS).min(chars.len());
    let mut s: String = chars[from..to].iter().collect();
    if from > 0 {
        s = format!("...{s}");
    }
    if to < chars.len() {
        s.push_str("...");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_name_from_directory() {
        let dir = std::env::temp_dir().join(format!("Meetings Book-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let nb = Notebook::at(&dir).unwrap();
        assert_eq!(
            collection_name(&nb),
            format!("denote_meetings_book_{}", std::process::id())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_point_id_is_stable_and_distinct() {
        assert_eq!(
            point_id("20261119T000000", 0),
            point_id("20261119T000000", 0)
        );
        assert_ne!(
            point_id("20261119T000000", 0),
            point_id("20261119T000000", 1)
        );
        assert!(point_id("x", 3) < 1 << 63);
    }

    #[test]
    fn test_filter_excludes_transcripts_by_default() {
        let f = filter(&Query::default());
        assert_eq!(f["must_not"][0]["match"]["value"], "transcript");
        let all = filter(&Query {
            all_kinds: true,
            ..Default::default()
        });
        assert!(all["must_not"].as_array().unwrap().is_empty());
        let kind = filter(&Query {
            kind: Some(Kind::Transcript),
            ..Default::default()
        });
        assert_eq!(kind["must"][0]["match"]["value"], "transcript");
        assert!(kind["must_not"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_filter_dates_and_keywords() {
        let f = filter(&Query {
            keywords: vec!["Acme".into(), "meeting".into()],
            since: NaiveDate::from_ymd_opt(2026, 10, 1),
            until: NaiveDate::from_ymd_opt(2026, 10, 31),
            ..Default::default()
        });
        let must = f["must"].as_array().unwrap();
        assert_eq!(must[0]["match"]["value"], "acme");
        assert_eq!(must[1]["match"]["value"], "meeting");
        let range = &must[2]["range"];
        assert!(
            range["gte"]
                .as_str()
                .unwrap()
                .starts_with("2026-10-01T00:00:00")
        );
        assert!(
            range["lt"]
                .as_str()
                .unwrap()
                .starts_with("2026-11-01T00:00:00")
        );
    }

    #[test]
    fn test_prefer_exact_terms_moves_exact_matches_first() {
        let point = |text: &str| qdrant::ScoredPoint {
            score: 0.0,
            payload: json!({ "text": text }),
        };
        let mut points = vec![
            point("fixversion IN (4.46.2.0)"),
            point("shipped in 4.45.1.0"),
            point("unrelated"),
            point("also 4.45.1.0 here"),
        ];
        prefer_exact_terms(&mut points, &sparse::tokenize("release 4.45.1.0"));
        let texts: Vec<&str> = points
            .iter()
            .map(|p| p.payload["text"].as_str().unwrap())
            .collect();
        assert_eq!(
            texts,
            [
                "shipped in 4.45.1.0",
                "also 4.45.1.0 here",
                "fixversion IN (4.46.2.0)",
                "unrelated"
            ]
        );

        // Plain words leave the fused order alone.
        let mut points = vec![point("b"), point("a")];
        prefer_exact_terms(&mut points, &sparse::tokenize("rollout risk"));
        assert_eq!(points[0].payload["text"], "b");
    }

    #[test]
    fn test_snippet_centers_on_first_term() {
        let text = format!(
            "{} we should progressively roll it out {}",
            "x ".repeat(200),
            "y ".repeat(200)
        );
        let s = snippet(&text, &["roll".to_string()]);
        assert!(s.starts_with("...") && s.ends_with("..."));
        assert!(s.contains("progressively roll it out"));
        assert_eq!(snippet("short text", &[]), "short text");
    }
}
