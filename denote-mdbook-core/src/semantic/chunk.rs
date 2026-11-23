//! Split a note body into chunks small enough for the embedding model.
//!
//! Notes split at `#`, `##` and `###` headings. A long section splits again
//! at blank-line blocks (paragraphs, or speaker turns in a transcript), with
//! one block of overlap. Each chunk also carries an "embed text" that starts
//! with the note title, date and section, so it makes sense on its own.

/// Upper bound on words per chunk. bge-small reads 512 tokens; about 280
/// words plus the prefix stays under that.
pub const MAX_WORDS: usize = 280;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// The nearest heading above the chunk, or "" before the first heading.
    pub heading: String,
    /// The chunk text as it appears in the note (transcript timestamps removed).
    pub text: String,
    /// `text` with the title, date and section prefix, for the embedder.
    pub embed_text: String,
}

/// Chunks of `body` (the note without front matter).
pub fn chunk_note(title: &str, date: &str, body: &str, is_transcript: bool) -> Vec<Chunk> {
    let mut out = Vec::new();
    for (heading, blocks) in sections(body, is_transcript) {
        for text in group_blocks(&blocks) {
            let embed_text = prefix(title, date, &heading, &text);
            out.push(Chunk {
                heading: heading.clone(),
                text,
                embed_text,
            });
        }
    }
    out
}

fn prefix(title: &str, date: &str, heading: &str, text: &str) -> String {
    let mut p = format!("Title: {title}.");
    if !date.is_empty() {
        p.push_str(&format!(" Date: {date}."));
    }
    if !heading.is_empty() && heading != title {
        p.push_str(&format!(" Section: {heading}."));
    }
    format!("{p}\n{text}")
}

/// Heading level 1 to 3 and its text, for an ATX heading line.
fn heading_text(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some(rest.trim().trim_end_matches('#').trim())
}

/// A transcript cue line such as `07:37:14 --> 07:37:29`.
fn is_cue_line(line: &str) -> bool {
    let t = line.trim();
    t.contains("-->")
        && t.chars()
            .all(|c| c.is_ascii_digit() || " :.,->".contains(c))
}

/// (heading, blank-line blocks) for each section of the body. Headings inside
/// code fences are ignored. Fence lines and transcript cue lines are dropped.
fn sections(body: &str, is_transcript: bool) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut heading = String::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_fence = false;

    let flush_block = |current: &mut String, blocks: &mut Vec<String>| {
        let t = current.trim();
        if !t.is_empty() {
            blocks.push(t.to_string());
        }
        current.clear();
    };

    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            flush_block(&mut current, &mut blocks);
            continue;
        }
        if !in_fence && let Some(h) = heading_text(trimmed) {
            flush_block(&mut current, &mut blocks);
            if !blocks.is_empty() {
                out.push((heading.clone(), std::mem::take(&mut blocks)));
            }
            heading = h.to_string();
            continue;
        }
        if is_transcript && is_cue_line(line) {
            continue;
        }
        if line.trim().is_empty() || line.trim() == "---" {
            flush_block(&mut current, &mut blocks);
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line.trim_end());
        }
    }
    flush_block(&mut current, &mut blocks);
    if !blocks.is_empty() {
        out.push((heading, blocks));
    }
    out
}

fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Join blocks into chunks of at most `MAX_WORDS` words. When a chunk is
/// full, the next one starts with the last block of the previous chunk. A
/// single block longer than the limit is split by words.
fn group_blocks(blocks: &[String]) -> Vec<String> {
    let pieces: Vec<String> = blocks.iter().flat_map(|b| split_long(b)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut count = 0;
    for piece in &pieces {
        let n = words(piece);
        if count + n > MAX_WORDS && !current.is_empty() {
            out.push(current.join("\n\n"));
            let last = *current.last().unwrap_or(&"");
            let overlap = words(last);
            if overlap + n <= MAX_WORDS && current.len() > 1 {
                current = vec![last];
                count = overlap;
            } else {
                current.clear();
                count = 0;
            }
        }
        current.push(piece);
        count += n;
    }
    if !current.is_empty() {
        out.push(current.join("\n\n"));
    }
    out
}

fn split_long(block: &str) -> Vec<String> {
    let w: Vec<&str> = block.split_whitespace().collect();
    if w.len() <= MAX_WORDS {
        return vec![block.to_string()];
    }
    w.chunks(MAX_WORDS).map(|c| c.join(" ")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_splits_at_headings() {
        let body =
            "# Title\n\nIntro line.\n\n## Risks\n\nRoll out slowly.\n\n## Next\n\nCall the CSM.\n";
        let chunks = chunk_note("Title", "2026-11-19", body, false);
        let headings: Vec<&str> = chunks.iter().map(|c| c.heading.as_str()).collect();
        assert_eq!(headings, ["Title", "Risks", "Next"]);
        assert_eq!(chunks[1].text, "Roll out slowly.");
        assert!(
            chunks[1]
                .embed_text
                .starts_with("Title: Title. Date: 2026-11-19. Section: Risks.\n")
        );
        // The section is not repeated when it equals the title.
        assert!(!chunks[0].embed_text.contains("Section:"));
    }

    #[test]
    fn test_chunk_ignores_headings_in_code_fences() {
        let body = "## Code\n\n```sh\n# not a heading\necho hi\n```\n";
        let chunks = chunk_note("T", "", body, false);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "Code");
        assert!(chunks[0].text.contains("# not a heading"));
    }

    #[test]
    fn test_chunk_long_section_overlaps_one_paragraph() {
        let para = |tag: &str| format!("{tag} {}", "word ".repeat(120).trim());
        let body = format!(
            "## S\n\n{}\n\n{}\n\n{}\n",
            para("one"),
            para("two"),
            para("three")
        );
        let chunks = chunk_note("T", "", &body, false);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.starts_with("one") && chunks[0].text.contains("two"));
        // The second chunk repeats the last paragraph of the first.
        assert!(chunks[1].text.starts_with("two") && chunks[1].text.contains("three"));
        assert!(chunks.iter().all(|c| words(&c.text) <= MAX_WORDS));
    }

    #[test]
    fn test_chunk_splits_one_huge_block_by_words() {
        let body = "word ".repeat(MAX_WORDS * 2 + 10);
        let chunks = chunk_note("T", "", &body, false);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| words(&c.text) <= MAX_WORDS));
    }

    #[test]
    fn test_chunk_transcript_groups_turns() {
        let mut body = String::from("# Sync Transcript\n\n```text\n");
        for i in 0..80 {
            body.push_str(&format!(
                "07:{:02}:00 --> 07:{:02}:05\nSpeaker {i}: {}\n\n",
                i % 60,
                i % 60,
                "talk ".repeat(10).trim()
            ));
        }
        body.push_str("```\n");
        let chunks = chunk_note("Sync Transcript", "2026-08-28", &body, true);
        assert!(chunks.len() >= 3, "got {} chunks", chunks.len());
        assert!(chunks.iter().all(|c| !c.text.contains("-->")));
        assert!(chunks[0].text.starts_with("Speaker 0:"));
        assert!(chunks.iter().all(|c| words(&c.text) <= MAX_WORDS));
    }

    #[test]
    fn test_chunk_empty_body_has_no_chunks() {
        assert!(chunk_note("T", "", "\n\n# Only a heading\n\n", false).is_empty());
    }
}
