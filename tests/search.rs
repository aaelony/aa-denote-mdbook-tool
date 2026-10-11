//! note-search spec.

mod common;

use std::fs;
use std::process::Command as StdCommand;

use common::*;

/// Notebook with matching text in an acme transcript, a beta transcript, and
/// an acme manual note.
fn test_helper_search_fixture_notebook() -> (tempfile::TempDir, std::path::PathBuf) {
    let (t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "meeting,acme");
    test_helper_add_meeting_note(&root, "09:00:00", "Beta Sync", "meeting,beta");
    for (parent, text) in [
        ("20261010T080000", "Budget is approved\n"),
        ("20261010T090000", "budget later\n"),
    ] {
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .args(["import", "-", "--kind", "transcript", "--parent", parent])
            .write_stdin(text)
            .assert()
            .success();
    }
    let acme = root.join("src/notes/20261010T080000--acme-sync__acme_meeting.md");
    let mut c = test_helper_read_file(&acme);
    c.push_str("\nbudget owner: Dana\n");
    fs::write(&acme, c).unwrap();
    (t, root)
}

fn test_helper_exit_code_and_stdout(c: &mut assert_cmd::Command) -> (i32, String) {
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
    )
}

#[test]
fn test_keyword_and_kind_filters() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let (code, text) = test_helper_exit_code_and_stdout(test_helper_isolated_cli(&root).args([
        "search",
        "Budget",
        "--keyword",
        "acme",
        "--kind",
        "transcript",
    ]));
    assert_eq!(code, 0, "{text}");
    let files: Vec<&str> = text.lines().map(|l| l.split(':').next().unwrap()).collect();
    assert!(!files.is_empty());
    for f in &files {
        assert!(
            f.starts_with("src/notes/zoom-transcripts/20261010T080001--"),
            "{text}"
        );
    }
}

#[test]
fn test_pass_through_arguments() {
    let (_t, root) = test_helper_search_fixture_notebook();
    // Case-sensitive by default: "budget" misses "Budget is approved".
    let (_, text) = test_helper_exit_code_and_stdout(test_helper_isolated_cli(&root).args([
        "search",
        "budget",
        "--keyword",
        "acme",
        "--kind",
        "transcript",
    ]));
    assert!(!text.contains("Budget is approved"));
    let (code, text) = test_helper_exit_code_and_stdout(test_helper_isolated_cli(&root).args([
        "search",
        "budget",
        "--keyword",
        "acme",
        "--kind",
        "transcript",
        "--",
        "-i",
        "-C",
        "2",
    ]));
    assert_eq!(code, 0);
    assert!(text.contains("Budget is approved"), "{text}");
}

/// An rg error (exit 2) on the filtered path is reported, not turned into
/// "no match" (1).
#[test]
fn test_filtered_search_reports_ripgrep_error() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let (code, _) = test_helper_exit_code_and_stdout(test_helper_isolated_cli(&root).args([
        "search",
        "budget",
        "--keyword",
        "acme",
        "--",
        "--no-such-ripgrep-flag",
    ]));
    assert_eq!(code, 2);
}

#[test]
fn test_unfiltered_search_covers_src() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let (code, text) = test_helper_exit_code_and_stdout(
        test_helper_isolated_cli(&root).args(["search", "budget"]),
    );
    assert_eq!(code, 0);
    assert!(
        text.contains("src/notes/20261010T080000--acme-sync__acme_meeting.md"),
        "{text}"
    );
    assert!(text.contains("src/notes/zoom-transcripts/"), "{text}");
}

#[test]
fn test_no_matching_files_is_status_1() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let (code, text) = test_helper_exit_code_and_stdout(test_helper_isolated_cli(&root).args([
        "search",
        "budget",
        "--keyword",
        "nosuchtag",
    ]));
    assert_eq!(code, 1);
    assert!(text.is_empty());
}

#[test]
fn test_ripgrep_missing() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let o = test_helper_isolated_cli(&root)
        .env("PATH", "")
        .args(["search", "budget"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("ripgrep is required"));
}

#[test]
fn test_plain_ripgrep_scenarios() {
    let (_t, root) = test_helper_search_fixture_notebook();
    let rg = |args: &[&str]| {
        let o = StdCommand::new("rg")
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };

    // By tag in front matter.
    let tagged = rg(&["-l", r#"tags = .*"acme""#, "src/notes"]);
    let mut tagged: Vec<&str> = tagged.lines().collect();
    tagged.sort();
    assert_eq!(
        tagged,
        vec![
            "src/notes/20261010T080000--acme-sync__acme_meeting.md",
            "src/notes/zoom-transcripts/20261010T080001--acme-sync-transcript__acme_meeting_zoomtranscript.md",
        ]
    );

    // By file name.
    let files = rg(&["--files", "src"]);
    let meetings: Vec<&str> = files
        .lines()
        .filter(|l| l.contains("__") && l.contains("meeting"))
        .collect();
    assert_eq!(meetings.len(), 4);

    // book/ is ignored after a build.
    test_helper_isolated_cli(&root)
        .arg("build")
        .assert()
        .success();
    assert!(root.join("book").is_dir());
    let hits = rg(&["-l", "budget"]);
    assert!(!hits.is_empty());
    assert!(hits.lines().all(|l| !l.starts_with("book/")), "{hits}");
}
