//! book-rendering spec.

mod common;

use std::fs;

use common::*;
use tempfile::TempDir;

#[test]
fn test_summary_groups_nests_and_preserves_hand_edits() {
    let (_t, root) = test_helper_init_notebook();
    // Hand edit above the begin marker.
    let summary_path = root.join("src/SUMMARY.md");
    let s = test_helper_read_file(&summary_path).replace(
        "[Introduction](README.md)\n",
        "[Introduction](README.md)\n[About](about.md)\n",
    );
    fs::write(&summary_path, s).unwrap();
    fs::write(root.join("src/about.md"), "# About\n").unwrap();

    test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-09", "16:00:00").args(["new", "--title", "Retro"]),
    );
    test_helper_add_meeting_note(&root, "09:00:00", "Roadmap", "meeting");
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    // Transcript imported before the summary; summaries still list first.
    test_helper_add_child_note(&root, "transcript", "20261010T080000");
    test_helper_add_child_note(&root, "summary", "20261010T080000");
    // Orphans: no parent, and a parent that does not exist.
    test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "10:30:00").args([
            "new",
            "--kind",
            "transcript",
            "--title",
            "Loose",
        ]),
    );
    let dangling = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "11:00:00").args([
            "new",
            "--kind",
            "transcript",
            "--title",
            "Dangling",
        ]),
    );
    let p = root.join(dangling.trim());
    let c = test_helper_read_file(&p).replace(
        "kind = \"transcript\"\n",
        "kind = \"transcript\"\nparent = \"20200101T000000\"\n",
    );
    fs::write(&p, c).unwrap();

    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    let got = test_helper_read_file(&summary_path);
    let expected = concat!(
        "# Summary\n\n[Introduction](README.md)\n[About](about.md)\n\n",
        "<!-- aa-denote-mdbook-tool:begin -->\n",
        "\n- [2026-10-10](days/2026-10-10.md)\n",
        "  - [Weekly Sync](notes/20261010T080000--weekly-sync__meeting.md)\n",
        "    - [Weekly Sync AI Summary](notes/zoom-summaries/20261010T080002--weekly-sync-ai-summary__meeting_zoomsummary.md)\n",
        "    - [Weekly Sync Transcript](notes/zoom-transcripts/20261010T080001--weekly-sync-transcript__meeting_zoomtranscript.md)\n",
        "  - [Roadmap](notes/20261010T090000--roadmap__meeting.md)\n",
        "  - [Loose](notes/zoom-transcripts/20261010T103000--loose.md)\n",
        "  - [Dangling](notes/zoom-transcripts/20261010T110000--dangling.md)\n",
        "- [2026-10-09](days/2026-10-09.md)\n",
        "  - [Retro](notes/20261009T160000--retro.md)\n",
        "\n<!-- aa-denote-mdbook-tool:end -->\n",
    );
    assert_eq!(got, expected);

    // Regenerating is stable.
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    assert_eq!(test_helper_read_file(&summary_path), expected);
}

#[test]
fn test_summary_writes_day_pages_and_removes_stale_ones() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    test_helper_add_child_note(&root, "summary", "20261010T080000");
    let retro = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-09", "16:00:00").args(["new", "--title", "Retro"]),
    );
    fs::create_dir_all(root.join("src/days")).unwrap();
    fs::write(root.join("src/days/notes-about-days.md"), "# Mine\n").unwrap();
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    assert_eq!(
        test_helper_read_file(root.join("src/days/2026-10-10.md")),
        concat!(
            "# 2026-10-10\n\n",
            "- [Weekly Sync](../notes/20261010T080000--weekly-sync__meeting.md)\n",
            "  - [Weekly Sync AI Summary](../notes/zoom-summaries/20261010T080001--weekly-sync-ai-summary__meeting_zoomsummary.md)\n",
        )
    );
    assert!(root.join("src/days/2026-10-09.md").is_file());

    // Removing the only note of a day removes its page, but not other files.
    fs::remove_file(root.join(retro.trim())).unwrap();
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    assert!(!root.join("src/days/2026-10-09.md").exists());
    assert!(root.join("src/days/notes-about-days.md").is_file());
    test_helper_isolated_cli(&root)
        .args(["links", "check"])
        .assert()
        .success();
}

#[test]
fn test_summary_skips_images_in_notes_folder() {
    let (_t, root) = test_helper_init_notebook();
    fs::write(
        root.join("src/notes/20261010T130000--stray.png"),
        b"\x89PNG",
    )
    .unwrap();
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    let summary = test_helper_read_file(root.join("src/SUMMARY.md"));
    assert!(!summary.contains("stray"), "{summary}");
}

/// A note that names itself as parent is treated as top-level, so its own
/// children still nest under it.
#[test]
fn test_summary_self_parent_still_nests_children() {
    let (_t, root) = test_helper_init_notebook();
    let meeting = root.join(test_helper_add_meeting_note(
        &root,
        "08:00:00",
        "Weekly Sync",
        "meeting",
    ));
    let content = test_helper_read_file(&meeting).replace(
        "kind = \"manual\"\n",
        "kind = \"manual\"\nparent = \"20261010T080000\"\n",
    );
    fs::write(&meeting, content).unwrap();
    let child = test_helper_add_child_note(&root, "summary", "20261010T080000");
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    let summary = test_helper_read_file(root.join("src/SUMMARY.md"));
    let child_link = child.strip_prefix("src/").unwrap();
    assert!(
        summary
            .lines()
            .any(|l| l.starts_with("    - ") && l.contains(child_link)),
        "{summary}"
    );
}

#[test]
fn test_preprocessor_supports_any_renderer() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_isolated_cli(&root)
        .args(["preprocess", "supports", "html"])
        .assert()
        .success();
}

/// `serve` must locate the notebook before starting mdBook; outside one it
/// fails instead of starting a server.
#[test]
fn test_serve_without_notebook_fails() {
    let tmp = TempDir::new().unwrap();
    let out = test_helper_isolated_cli(tmp.path())
        .arg("serve")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&out);
    assert!(msg.contains("no notebook found"), "{msg}");
}

#[test]
fn test_build_hides_front_matter() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    test_helper_add_child_note(&root, "summary", "20261010T080000");
    test_helper_isolated_cli(&root)
        .arg("build")
        .assert()
        .success();

    let html = test_helper_read_file(
        root.join("book/notes/20261010T080000--weekly-sync__acme_meeting.html"),
    );
    assert!(
        html.contains("Weekly Sync</h1>") || html.contains("Weekly Sync</a></h1>"),
        "{html}"
    );
    assert!(!html.contains("identifier ="));
    assert!(!html.contains("title ="));
    // Sources keep their front matter.
    assert!(
        test_helper_read_file(root.join("src/notes/20261010T080000--weekly-sync__acme_meeting.md"))
            .starts_with("+++\n")
    );
    // Rendered summary is a subchapter in the sidebar.
    assert!(root.join("book/notes/zoom-summaries").is_dir());
    // Each day is a linked page in the sidebar.
    let day_html = test_helper_read_file(root.join("book/days/2026-10-10.html"));
    assert!(day_html.contains("2026-10-10</h1>") || day_html.contains("2026-10-10</a></h1>"));
    assert!(day_html.contains("../notes/20261010T080000--weekly-sync__acme_meeting.html"));
}

#[test]
fn test_build_without_mdbook() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let err = test_helper_isolated_cli(&root)
        .env("PATH", "")
        .arg("build")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&err);
    assert!(
        msg.contains("mdbook") && msg.contains("cargo install mdbook"),
        "{msg}"
    );
    // SUMMARY.md was regenerated before mdbook was needed.
    assert!(test_helper_read_file(root.join("src/SUMMARY.md")).contains("Weekly Sync"));
}
