//! mdBook preprocessor that hides `+++` front matter in rendered chapters and,
//! for the HTML renderer, turns ```` ```mermaid ```` code blocks into diagrams.
//!
//! mdBook calls `aa-denote-mdbook-tool preprocess supports <renderer>` and then
//! `aa-denote-mdbook-tool preprocess` with `[context, book]` JSON on stdin, expecting
//! the book JSON on stdout. Working on `serde_json::Value` keeps any fields
//! this tool does not know about.

use std::io::Read as _;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::frontmatter;

pub fn run(args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) == Some("supports") {
        // Front matter stripping is renderer-independent.
        return Ok(());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let output = process(&input)?;
    println!("{output}");
    Ok(())
}

pub fn process(input: &str) -> Result<String> {
    let value: Value = serde_json::from_str(input).context("invalid preprocessor input")?;
    let Value::Array(mut parts) = value else {
        bail!("preprocessor input must be a [context, book] array");
    };
    if parts.len() != 2 {
        bail!("preprocessor input must be a [context, book] array");
    }
    let mut book = parts.pop().unwrap_or(Value::Null);
    // Other renderers (for example `markdown`) keep the mermaid source as is.
    let html = parts[0].get("renderer").and_then(Value::as_str) == Some("html");
    // mdBook 0.4 calls the list `sections`; 0.5 calls it `items`.
    for key in ["sections", "items"] {
        if let Some(Value::Array(items)) = book.get_mut(key) {
            strip_items(items, html);
        }
    }
    Ok(serde_json::to_string(&book)?)
}

fn strip_items(items: &mut [Value], html: bool) {
    for item in items {
        let Some(chapter) = item.get_mut("Chapter") else {
            continue;
        };
        if let Some(Value::String(content)) = chapter.get_mut("content") {
            let stripped = frontmatter::strip(content).trim_start_matches(['\n', '\r']);
            *content = if html {
                crate::mermaid::render(stripped)
            } else {
                stripped.to_string()
            };
        }
        if let Some(Value::Array(subs)) = chapter.get_mut("sub_items") {
            strip_items(subs, html);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strips_nested_chapters() {
        let fm = "+++\\ntitle = \\\"A\\\"\\nidentifier = \\\"20261010T080000\\\"\\n+++\\n\\n# A\\n";
        let input = format!(
            r#"[{{"root":"/x","config":{{}},"renderer":"html","mdbook_version":"0.4.40"}},
              {{"sections":[
                {{"Chapter":{{"name":"A","content":"{fm}","number":null,
                  "sub_items":[{{"Chapter":{{"name":"B","content":"{fm}","sub_items":[]}}}}],
                  "path":"a.md"}}}},
                {{"PartTitle":"2026-10-10"}},
                "Separator"
              ],"__non_exhaustive":null}}]"#
        );
        let out: Value = serde_json::from_str(&process(&input).unwrap()).unwrap();
        let a = &out["sections"][0]["Chapter"];
        assert_eq!(a["content"], "# A\n");
        assert_eq!(a["sub_items"][0]["Chapter"]["content"], "# A\n");
        assert_eq!(a["path"], "a.md");
        assert_eq!(out["sections"][1]["PartTitle"], "2026-10-10");
        assert!(out.get("__non_exhaustive").is_some());
    }

    #[test]
    fn test_mermaid_only_for_html_renderer() {
        let chapter = r##"{"sections":[{"Chapter":{"content":"```mermaid\ngraph TD\n```\n","sub_items":[]}}]}"##;
        for (renderer, converted) in [("html", true), ("markdown", false)] {
            let input = format!(r#"[{{"renderer":"{renderer}"}}, {chapter}]"#);
            let out: Value = serde_json::from_str(&process(&input).unwrap()).unwrap();
            let content = out["sections"][0]["Chapter"]["content"].as_str().unwrap();
            assert_eq!(
                content.contains("<pre class=\"mermaid\">"),
                converted,
                "{renderer}"
            );
        }
    }

    #[test]
    fn test_content_without_front_matter_unchanged() {
        let input = r##"[{}, {"sections":[{"Chapter":{"content":"# Intro\n","sub_items":[]}}]}]"##;
        let out: Value = serde_json::from_str(&process(input).unwrap()).unwrap();
        assert_eq!(out["sections"][0]["Chapter"]["content"], "# Intro\n");
    }
}
