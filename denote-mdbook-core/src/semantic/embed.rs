//! Local embeddings with fastembed (ONNX Runtime): `BAAI/bge-small-en-v1.5`.
//!
//! The model loads on the first embed call, so commands that find nothing to
//! embed (for example an `index` run where no note changed) start fast.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use super::Embedder;

pub const MODEL: &str = "BAAI/bge-small-en-v1.5";
pub const DIM: usize = 384;
/// bge models expect this prefix on search questions, not on passages.
const QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";

pub struct FastEmbedder {
    model: Option<TextEmbedding>,
    cache_dir: PathBuf,
}

/// Model files live in the user cache directory, shared by all notebooks.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("aa-denote-mdbook-tool")
        .join("models")
}

impl Default for FastEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl FastEmbedder {
    pub fn new() -> Self {
        Self {
            model: None,
            cache_dir: cache_dir(),
        }
    }

    fn loaded(&mut self) -> Result<&mut TextEmbedding> {
        if self.model.is_none() {
            let options = TextInitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_cache_dir(self.cache_dir.clone())
                .with_show_download_progress(true);
            let model = TextEmbedding::try_new(options)
                .map_err(|e| anyhow!("cannot load embedding model {MODEL}: {e}"))?;
            self.model = Some(model);
        }
        Ok(self.model.as_mut().expect("model loaded above"))
    }
}

impl Embedder for FastEmbedder {
    fn model(&self) -> &str {
        MODEL
    }

    fn dim(&self) -> usize {
        DIM
    }

    fn embed_passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        self.loaded()?
            .embed(texts, None)
            .map_err(|e| anyhow!("embedding failed: {e}"))
    }

    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        let text = [format!("{QUERY_PREFIX}{query}")];
        self.loaded()?
            .embed(&text, None)
            .map_err(|e| anyhow!("embedding failed: {e}"))?
            .pop()
            .ok_or_else(|| anyhow!("embedding returned no vector"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Downloads the model on first run: `cargo test --features semantic -- --ignored`.
    #[test]
    #[ignore = "downloads the embedding model"]
    fn test_embed_returns_384_dims() {
        let mut e = FastEmbedder::new();
        let v = e.embed_query("rollout risk").unwrap();
        assert_eq!(v.len(), DIM);
        let p = e
            .embed_passages(&["one".to_string(), "two".to_string()])
            .unwrap();
        assert_eq!(p.len(), 2);
    }
}
