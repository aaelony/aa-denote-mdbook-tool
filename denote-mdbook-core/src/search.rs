//! `search`: ripgrep over the notebook with keyword and kind filters.

use std::process::Command;

use anyhow::Result;

use crate::external;
use crate::frontmatter::Kind;
use crate::naming;
use crate::notebook::Notebook;

/// Keep each `rg` command line well under OS argument limits.
const BATCH: usize = 500;

const RG_HINT: &str = "ripgrep is required for search: install it with `cargo install ripgrep` \
                       or your package manager.";

pub fn search(
    nb: &Notebook,
    pattern: &str,
    keywords: &[String],
    kind: Option<Kind>,
    rg_args: &[String],
) -> Result<u8> {
    let rg = |paths: &[String]| -> Result<u8> {
        let mut cmd = Command::new("rg");
        // -H: show file names even when a batch holds a single file.
        cmd.current_dir(&nb.root)
            .arg("-H")
            .args(rg_args)
            .arg("-e")
            .arg(pattern)
            .args(paths);
        external::run(cmd, RG_HINT)
    };

    if keywords.is_empty() && kind.is_none() {
        return rg(&["src".to_string()]);
    }

    let wanted = naming::normalize_keywords(keywords);
    let paths: Vec<String> = nb
        .scan()
        .into_iter()
        .filter(|e| e.is_note())
        .filter(|e| kind.is_none_or(|k| nb.kind_of(e) == k))
        .filter(|e| wanted.iter().all(|k| e.name.keywords.contains(k)))
        .map(|e| nb.display(&e.path))
        .collect();

    if paths.is_empty() {
        // Same status rg uses for "no matches".
        return Ok(1);
    }

    let mut status = 1u8;
    for chunk in paths.chunks(BATCH) {
        status = combine_batch_status(status, rg(chunk)?);
    }
    Ok(status)
}

/// Fold one batch's `rg` exit code into the status so far: 0 if any batch
/// matched, else the latest error code, else 1 (no match).
fn combine_batch_status(status_so_far: u8, batch_status: u8) -> u8 {
    if status_so_far == 0 || batch_status == 0 {
        0
    } else if batch_status == 1 {
        status_so_far
    } else {
        batch_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combine_batch_status_match_wins() {
        assert_eq!(combine_batch_status(1, 0), 0);
        assert_eq!(combine_batch_status(2, 0), 0);
        // An error after a match does not override it.
        assert_eq!(combine_batch_status(0, 2), 0);
        assert_eq!(combine_batch_status(0, 1), 0);
    }

    #[test]
    fn test_combine_batch_status_no_match_keeps_status() {
        assert_eq!(combine_batch_status(1, 1), 1);
        // An earlier error survives a later batch with no match.
        assert_eq!(combine_batch_status(2, 1), 2);
    }

    #[test]
    fn test_combine_batch_status_error_is_reported() {
        assert_eq!(combine_batch_status(1, 2), 2);
    }
}
