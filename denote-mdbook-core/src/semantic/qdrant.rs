//! A small Qdrant client over the REST API, on the `ureq` agent the crate
//! already uses. Only the calls `index` and `search --semantic` need.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use ureq::Agent;

use super::sparse::SparseVector;

/// Environment variable that overrides the Qdrant URL.
pub const URL_ENV: &str = "AA_DENOTE_MDBOOK_TOOL_QDRANT_URL";
pub const DEFAULT_URL: &str = "http://127.0.0.1:6333";

pub const DENSE: &str = "dense";
pub const SPARSE: &str = "terms";

/// The Qdrant URL from the environment, or the local default.
pub fn url_from_env() -> String {
    std::env::var(URL_ENV)
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string())
}

pub struct Qdrant {
    base: String,
    agent: Agent,
}

/// One result of a query.
#[derive(Debug, Clone)]
pub struct ScoredPoint {
    pub score: f64,
    pub payload: Value,
}

type Response = ureq::http::Response<ureq::Body>;

impl Qdrant {
    pub fn new(url: &str) -> Self {
        let agent: Agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        Self {
            base: url.trim_end_matches('/').to_string(),
            agent,
        }
    }

    pub fn url(&self) -> &str {
        &self.base
    }

    fn unreachable(&self) -> String {
        format!(
            "cannot reach Qdrant at {}: start it (see the README) or set {URL_ENV}; \
             plain `search` works without it",
            self.base
        )
    }

    fn check(&self, result: Result<Response, ureq::Error>, what: &str) -> Result<Value> {
        let mut response = result.with_context(|| self.unreachable())?;
        let status = response.status();
        let body = response.body_mut().read_to_string().unwrap_or_default();
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.pointer("/status/error")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or(body);
            bail!(
                "Qdrant {what} failed: HTTP {}: {}",
                status.as_u16(),
                message.trim()
            );
        }
        if body.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body)
            .with_context(|| format!("Qdrant {what} returned unexpected JSON"))
    }

    fn get(&self, path: &str, what: &str) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        self.check(self.agent.get(&url).call(), what)
    }

    fn send(&self, method: &str, path: &str, body: &Value, what: &str) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let result = match method {
            "PUT" => self.agent.put(&url).send_json(body),
            "POST" => self.agent.post(&url).send_json(body),
            _ => unreachable!("unsupported method {method}"),
        };
        self.check(result, what)
    }

    /// Fails with a clear message when Qdrant does not answer.
    pub fn ping(&self) -> Result<()> {
        self.get("/", "health check").map(|_| ())
    }

    pub fn collection_exists(&self, name: &str) -> Result<bool> {
        let v = self.get(&format!("/collections/{name}/exists"), "collection check")?;
        Ok(v.pointer("/result/exists")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Dense size of an existing collection, if it has the `dense` vector.
    pub fn dense_size(&self, name: &str) -> Result<Option<u64>> {
        let v = self.get(&format!("/collections/{name}"), "collection info")?;
        Ok(
            v.pointer(&format!("/result/config/params/vectors/{DENSE}/size"))
                .and_then(Value::as_u64),
        )
    }

    /// Create the collection with a dense and a sparse vector and the payload
    /// indexes the filters use. Returns false if it already existed.
    pub fn ensure_collection(&self, name: &str, dim: usize) -> Result<bool> {
        if self.collection_exists(name)? {
            if let Some(size) = self.dense_size(name)?
                && usize::try_from(size).ok() != Some(dim)
            {
                bail!(
                    "Qdrant collection {name} has {size}-dimension vectors but the model makes \
                     {dim}: run `index --rebuild`"
                );
            }
            return Ok(false);
        }
        let body = json!({
            "vectors": { DENSE: { "size": dim, "distance": "Cosine" } },
            "sparse_vectors": { SPARSE: { "modifier": "idf" } }
        });
        self.send(
            "PUT",
            &format!("/collections/{name}"),
            &body,
            "create collection",
        )?;
        for (field, schema) in [
            ("identifier", "keyword"),
            ("kind", "keyword"),
            ("keywords", "keyword"),
            ("parent", "keyword"),
            ("date", "datetime"),
        ] {
            let body = json!({ "field_name": field, "field_schema": schema });
            self.send(
                "PUT",
                &format!("/collections/{name}/index?wait=true"),
                &body,
                "create payload index",
            )?;
        }
        Ok(true)
    }

    pub fn delete_collection(&self, name: &str) -> Result<()> {
        let url = format!("{}/collections/{name}", self.base);
        self.check(self.agent.delete(&url).call(), "delete collection")
            .map(|_| ())
    }

    /// Insert or replace points: `(id, dense, sparse, payload)`.
    pub fn upsert(
        &self,
        name: &str,
        points: &[(u64, Vec<f32>, SparseVector, Value)],
    ) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        let points: Vec<Value> = points
            .iter()
            .map(|(id, dense, sparse, payload)| {
                let mut vector = json!({ DENSE: dense });
                if !sparse.is_empty() {
                    vector[SPARSE] = json!({ "indices": sparse.indices, "values": sparse.values });
                }
                json!({ "id": id, "vector": vector, "payload": payload })
            })
            .collect();
        self.send(
            "PUT",
            &format!("/collections/{name}/points?wait=true"),
            &json!({ "points": points }),
            "upsert",
        )
        .map(|_| ())
    }

    /// Delete every point of one note.
    pub fn delete_note(&self, name: &str, identifier: &str) -> Result<()> {
        let body = json!({
            "filter": { "must": [ { "key": "identifier", "match": { "value": identifier } } ] }
        });
        self.send(
            "POST",
            &format!("/collections/{name}/points/delete?wait=true"),
            &body,
            "delete",
        )
        .map(|_| ())
    }

    /// Hybrid query: dense and sparse prefetch fused with RRF. With an empty
    /// sparse vector, a plain dense query.
    pub fn hybrid_query(
        &self,
        name: &str,
        dense: &[f32],
        sparse: &SparseVector,
        filter: &Value,
        limit: usize,
    ) -> Result<Vec<ScoredPoint>> {
        let prefetch_limit = (limit * 4).max(40);
        let mut prefetch = vec![
            json!({ "query": dense, "using": DENSE, "limit": prefetch_limit, "filter": filter }),
        ];
        if !sparse.is_empty() {
            prefetch.push(json!({
                "query": { "indices": sparse.indices, "values": sparse.values },
                "using": SPARSE,
                "limit": prefetch_limit,
                "filter": filter
            }));
        }
        let body = json!({
            "prefetch": prefetch,
            "query": { "fusion": "rrf" },
            "limit": limit,
            "with_payload": true
        });
        let v = self.send(
            "POST",
            &format!("/collections/{name}/points/query"),
            &body,
            "query",
        )?;
        let points = v
            .pointer("/result/points")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(points
            .into_iter()
            .map(|p| ScoredPoint {
                score: p.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                payload: p.get("payload").cloned().unwrap_or(Value::Null),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unreachable_qdrant_explains() {
        // Port 9 (discard) is closed on a normal machine, so the connection fails fast.
        let q = Qdrant::new("http://127.0.0.1:9/");
        let err = format!("{:#}", q.ping().unwrap_err());
        assert!(
            err.contains("cannot reach Qdrant at http://127.0.0.1:9"),
            "{err}"
        );
        assert!(err.contains("plain `search`"), "{err}");
    }
}
