//! Turns ```` ```mermaid ```` code blocks into `<pre class="mermaid">` blocks
//! that Mermaid draws in the browser.
//!
//! Each chapter that has a diagram gets one `<script type="module">` that
//! loads Mermaid from a CDN, so pages without diagrams load nothing extra.
//! Code blocks inside other code blocks (for example a ```` ````markdown ````
//! example that shows mermaid source) stay as code.

/// Mermaid release that the rendered pages load.
pub const MERMAID_VERSION: &str = "11.4.1";

/// Return `content` with every mermaid code block replaced by an HTML block,
/// plus the loader script when at least one block was replaced. Content
/// without mermaid blocks comes back unchanged.
pub fn render(content: &str) -> String {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let mut out = String::with_capacity(content.len());
    let mut found = false;
    let mut i = 0;
    while i < lines.len() {
        let Some(open) = Fence::open(lines[i]) else {
            out.push_str(lines[i]);
            i += 1;
            continue;
        };
        let close = lines[i + 1..]
            .iter()
            .position(|l| open.closes(l))
            .map(|p| i + 1 + p);
        let Some(close) = close else {
            // An unclosed fence runs to the end of the chapter: leave it as is.
            lines[i..].iter().for_each(|l| out.push_str(l));
            break;
        };
        if open.is_mermaid {
            found = true;
            let indent = &lines[i][..open.indent];
            out.push_str(indent);
            out.push_str("<pre class=\"mermaid\">\n");
            for line in &lines[i + 1..close] {
                out.push_str(&escape(strip_indent(line, open.indent)));
                if !line.ends_with('\n') {
                    out.push('\n');
                }
            }
            out.push_str(indent);
            out.push_str("</pre>\n");
        } else {
            lines[i..=close].iter().for_each(|l| out.push_str(l));
        }
        i = close + 1;
    }
    if found {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&loader_script());
    }
    out
}

/// The script that loads Mermaid and draws every `pre.mermaid` on the page.
/// It picks Mermaid's dark theme when mdBook shows a dark theme.
fn loader_script() -> String {
    format!(
        "<script type=\"module\">\n\
         import mermaid from \"https://cdn.jsdelivr.net/npm/mermaid@{MERMAID_VERSION}/dist/mermaid.esm.min.mjs\";\n\
         const dark = [\"coal\", \"navy\", \"ayu\"].some((c) => document.documentElement.classList.contains(c));\n\
         mermaid.initialize({{ startOnLoad: false, theme: dark ? \"dark\" : \"default\" }});\n\
         await mermaid.run({{ querySelector: \"pre.mermaid\" }});\n\
         </script>\n"
    )
}

/// An opening code fence: ```` ``` ```` or `~~~`, three or more long.
struct Fence {
    indent: usize,
    ch: char,
    len: usize,
    is_mermaid: bool,
}

impl Fence {
    fn open(line: &str) -> Option<Fence> {
        let body = line.trim_end_matches(['\n', '\r']);
        let rest = body.trim_start_matches(' ');
        let indent = body.len() - rest.len();
        let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
        let len = rest.chars().take_while(|c| *c == ch).count();
        if len < 3 {
            return None;
        }
        let info = rest[len..].trim();
        // A backtick fence cannot have a backtick in its info string.
        if ch == '`' && info.contains('`') {
            return None;
        }
        let lang = info.split_whitespace().next().unwrap_or("");
        Some(Fence {
            indent,
            ch,
            len,
            is_mermaid: lang == "mermaid",
        })
    }

    fn closes(&self, line: &str) -> bool {
        let t = line.trim();
        let run = t.chars().take_while(|c| *c == self.ch).count();
        run >= self.len && run == t.chars().count()
    }
}

/// Remove up to `n` leading spaces, as CommonMark does for fenced content.
fn strip_indent(line: &str, n: usize) -> &str {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    &line[spaces.min(n)..]
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_converts_mermaid_block_and_adds_script_once() {
        let md =
            "# T\n\n```mermaid\ngraph TD\n  A-->B\n```\n\ntext\n\n~~~ mermaid\nflowchart LR\n~~~\n";
        let out = render(md);
        assert!(
            out.starts_with(
                "# T\n\n<pre class=\"mermaid\">\ngraph TD\n  A--&gt;B\n</pre>\n\ntext\n"
            )
        );
        assert!(out.contains("<pre class=\"mermaid\">\nflowchart LR\n</pre>\n"));
        assert_eq!(out.matches("<script type=\"module\">").count(), 1);
        assert!(out.contains(&format!("mermaid@{MERMAID_VERSION}/")));
        assert!(!out.contains("```"));
    }

    #[test]
    fn test_escapes_html() {
        let out = render("```mermaid\nA[\"<b>x</b> & y\"]\n```\n");
        assert!(out.contains("A[\"&lt;b&gt;x&lt;/b&gt; &amp; y\"]\n"));
    }

    #[test]
    fn test_content_without_mermaid_unchanged() {
        for md in [
            "# T\n\n```rust\nfn main() {}\n```\n",
            "no newline at end",
            "````markdown\n```mermaid\ngraph TD\n```\n````\n",
            "```mermaid\ngraph TD\n",
            "``mermaid``\n",
        ] {
            assert_eq!(render(md), md);
        }
    }

    #[test]
    fn test_indented_block_in_list() {
        let out = render("- item\n\n  ```mermaid\n  graph TD\n    A-->B\n  ```\n");
        assert!(
            out.starts_with(
                "- item\n\n  <pre class=\"mermaid\">\ngraph TD\n  A--&gt;B\n  </pre>\n"
            )
        );
    }

    #[test]
    fn test_longer_closing_fence_and_no_trailing_newline() {
        let out = render("```mermaid\ngraph TD\n`````");
        assert!(out.starts_with("<pre class=\"mermaid\">\ngraph TD\n</pre>\n\n<script"));
    }
}
