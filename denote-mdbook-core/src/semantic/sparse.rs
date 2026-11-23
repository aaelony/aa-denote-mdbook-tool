//! Sparse term vectors for exact-term matching.
//!
//! Each token maps to a stable 32-bit index (FNV-1a hash) with a BM25
//! term-frequency weight. The Qdrant collection applies inverse document
//! frequency (`modifier: idf`) at query time, so rare terms such as a version
//! number or a customer name count for more than common words.

use std::collections::BTreeMap;

const K1: f32 = 1.2;
const B: f32 = 0.75;
/// Typical chunk length in tokens, for BM25 length normalization.
const AVG_LEN: f32 = 180.0;

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from", "has", "have", "he",
    "i", "if", "in", "is", "it", "its", "of", "on", "or", "our", "she", "so", "that", "the",
    "their", "them", "then", "there", "they", "this", "to", "uh", "um", "was", "we", "were",
    "what", "when", "which", "who", "will", "with", "you", "your",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl SparseVector {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

/// Lowercase tokens. `.`, `_` and `-` inside a token are kept, so `4.45.1.0`,
/// `lrs_pygen` and `snap-code` stay whole. Stopwords and one-letter tokens
/// are dropped.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let push = |current: &mut String, out: &mut Vec<String>| {
        let t = current.trim_matches(|c| c == '.' || c == '_' || c == '-');
        if t.chars().count() > 1 && !STOPWORDS.contains(&t) {
            out.push(t.to_string());
        }
        current.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
            current.extend(c.to_lowercase());
        } else {
            push(&mut current, &mut out);
        }
    }
    push(&mut current, &mut out);
    out
}

/// FNV-1a: stable across Rust versions and platforms, unlike `DefaultHasher`.
pub fn token_index(token: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in token.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn counts(tokens: &[String]) -> BTreeMap<u32, u32> {
    let mut m = BTreeMap::new();
    for t in tokens {
        *m.entry(token_index(t)).or_insert(0) += 1;
    }
    m
}

/// BM25 term weights for a document (chunk).
pub fn document_vector(text: &str) -> SparseVector {
    let tokens = tokenize(text);
    #[allow(clippy::cast_precision_loss)]
    let len = tokens.len() as f32;
    let norm = K1 * (1.0 - B + B * len / AVG_LEN);
    let mut v = SparseVector::default();
    for (index, tf) in counts(&tokens) {
        #[allow(clippy::cast_precision_loss)]
        let tf = tf as f32;
        v.indices.push(index);
        v.values.push(tf * (K1 + 1.0) / (tf + norm));
    }
    v
}

/// Query terms, each with weight 1.
pub fn query_vector(text: &str) -> SparseVector {
    let mut v = SparseVector::default();
    for index in counts(&tokenize(text)).into_keys() {
        v.indices.push(index);
        v.values.push(1.0);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_keeps_versions_and_identifiers() {
        let t = tokenize("Shipped 4.45.1.0 via lrs_pygen; see SnapCode-Plugin. Done.");
        assert_eq!(
            t,
            [
                "shipped",
                "4.45.1.0",
                "via",
                "lrs_pygen",
                "see",
                "snapcode-plugin",
                "done"
            ]
        );
    }

    #[test]
    fn test_tokenize_drops_stopwords_and_single_letters() {
        assert_eq!(
            tokenize("The plan is a go, I think"),
            ["plan", "go", "think"]
        );
    }

    #[test]
    fn test_sparse_vector_is_stable_across_runs() {
        // Fixed values: a change here would silently break existing indexes.
        assert_eq!(token_index("emirates"), 833_831_233);
        assert_eq!(
            document_vector("Emirates emirates"),
            document_vector("emirates EMIRATES")
        );
    }

    #[test]
    fn test_document_vector_weights_repeated_terms_higher() {
        let v = document_vector("rollout rollout rollout risk");
        let w = |t: &str| v.values[v.indices.iter().position(|i| *i == token_index(t)).unwrap()];
        assert!(w("rollout") > w("risk"));
        assert_eq!(v.indices.len(), 2);
    }

    #[test]
    fn test_query_vector_has_unit_weights() {
        let q = query_vector("the 4.45.1.0 release release");
        assert_eq!(q.values, [1.0, 1.0]);
        assert!(query_vector("the a of").is_empty());
    }
}
