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

/// Run `rg` with inherited output and return its exit status.
pub fn search(
    nb: &Notebook,
    pattern: &str,
    keywords: &[String],
    kind: Option<Kind>,
    rg_args: &[String],
) -> Result<u8> {
    run_filtered(nb, keywords, kind, |paths| {
        external::run(rg_command(nb, pattern, rg_args, paths), RG_HINT)
    })
}

/// What `search_captured` returns: `rg`'s combined exit status and output.
#[derive(Debug, Default)]
pub struct CapturedSearch {
    pub status: u8,
    pub stdout: String,
    pub stderr: String,
}

/// Like `search` without extra `rg` arguments, but collecting `rg`'s output
/// instead of letting it write to this process's standard output.
pub fn search_captured(
    nb: &Notebook,
    pattern: &str,
    keywords: &[String],
    kind: Option<Kind>,
) -> Result<CapturedSearch> {
    let mut stdout = String::new();
    let mut stderr = String::new();
    let status = run_filtered(nb, keywords, kind, |paths| {
        let out = external::output(rg_command(nb, pattern, &[], paths), RG_HINT)?;
        stdout.push_str(&String::from_utf8_lossy(&out.stdout));
        stderr.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok(out
            .status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .unwrap_or(1))
    })?;
    Ok(CapturedSearch {
        status,
        stdout,
        stderr,
    })
}

fn rg_command(nb: &Notebook, pattern: &str, rg_args: &[String], paths: &[String]) -> Command {
    let mut cmd = Command::new("rg");
    // -H: show file names even when a batch holds a single file.
    cmd.current_dir(&nb.root)
        .arg("-H")
        .args(rg_args)
        .arg("-e")
        .arg(pattern)
        .args(paths);
    cmd
}

/// Call `rg` on `src/`, or on the notes matching the keyword and kind
/// filters in batches, and combine the statuses.
fn run_filtered(
    nb: &Notebook,
    keywords: &[String],
    kind: Option<Kind>,
    mut rg: impl FnMut(&[String]) -> Result<u8>,
) -> Result<u8> {
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
