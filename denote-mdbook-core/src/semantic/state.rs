//! `.semantic-index.json`: what the index holds, so `index` can skip notes
//! that did not change.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::notebook::Notebook;

pub const STATE_FILE: &str = ".semantic-index.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteState {
    /// SHA-256 of the note file, hex.
    pub hash: String,
    pub chunks: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Embedding model the vectors came from.
    pub model: String,
    /// Qdrant collection name.
    pub collection: String,
    /// By note identifier.
    pub notes: BTreeMap<String, NoteState>,
}

pub fn path(nb: &Notebook) -> PathBuf {
    nb.root.join(STATE_FILE)
}

pub fn exists(nb: &Notebook) -> bool {
    path(nb).is_file()
}

impl State {
    /// The saved state, or an empty one if there is none yet.
    pub fn load(nb: &Notebook) -> Result<Self> {
        let p = path(nb);
        if !p.is_file() {
            return Ok(Self::default());
        }
        let text =
            fs::read_to_string(&p).with_context(|| format!("cannot read {}", p.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", p.display()))
    }

    /// Write atomically: a temporary file, then rename.
    pub fn save(&self, nb: &Notebook) -> Result<()> {
        let p = path(nb);
        let tmp = p.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)? + "\n")
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        fs::rename(&tmp, &p).with_context(|| format!("cannot write {}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_state_roundtrip() {
        let dir = std::env::temp_dir().join(format!("denote-state-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let nb = Notebook::at(&dir).unwrap();
        assert_eq!(State::load(&nb).unwrap(), State::default());
        let mut s = State {
            model: "m".into(),
            collection: "c".into(),
            ..Default::default()
        };
        s.notes.insert(
            "20261119T000000".into(),
            NoteState {
                hash: "abc".into(),
                chunks: 3,
            },
        );
        s.save(&nb).unwrap();
        assert!(exists(&nb));
        assert_eq!(State::load(&nb).unwrap(), s);
        fs::remove_dir_all(&dir).unwrap();
    }
}
