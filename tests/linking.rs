//! note-linking spec, including rename.

mod common;

use std::fs;

use common::*;

const WEEKLY: &str = "src/notes/20261010T080000--weekly-sync__acme_meeting.md";

/// A meeting plus an attached summary at 09:01:05 in zoom-summaries.
fn test_helper_meeting_and_summary(root: &std::path::Path) -> String {
    test_helper_add_meeting_note(root, "08:00:00", "Weekly Sync", "meeting,acme");
    test_helper_run_ok_stdout(
        test_helper_cli_at_time(root, "2026-10-10", "09:01:05").args([
            "new", "--kind", "summary", "--title", "Notes", "--date", "09:01:05",
        ]),
    )
    .trim()
    .to_string()
}

fn test_helper_append_to_file(path: impl AsRef<std::path::Path>, text: &str) {
    let mut c = test_helper_read_file(&path);
    c.push_str(text);
    fs::write(path, c).unwrap();
}

#[test]
fn test_link_relative_to_from_note() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_meeting_and_summary(&root);
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args([
        "link",
        "20261010T080000",
        "--from",
        "20261010T090105",
    ]));
    assert_eq!(
        out,
        "[Weekly Sync](../20261010T080000--weekly-sync__acme_meeting.md)\n"
    );

    // Default is relative to src/notes/.
    let out = test_helper_run_ok_stdout(
        test_helper_isolated_cli(&root).args(["link", "20261010T080000"]),
    );
    assert_eq!(
        out,
        "[Weekly Sync](20261010T080000--weekly-sync__acme_meeting.md)\n"
    );
}

#[test]
fn test_link_append() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_isolated_cli(&root)
        .args([
            "link",
            "20261010T080000",
            "--from",
            "20261010T090105",
            "--append",
        ])
        .assert()
        .success();
    assert!(
        test_helper_read_file(root.join(summary))
            .ends_with("\n\n[Weekly Sync](../20261010T080000--weekly-sync__acme_meeting.md)\n")
    );
}

#[test]
fn test_check_reports_broken_and_passes_when_valid() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_append_to_file(
        root.join(&summary),
        "\nSee [sync](../20261010T080000--weekly-sync__acme_meeting.md).\n",
    );
    let out = test_helper_isolated_cli(&root)
        .args(["links", "check"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(out.is_empty());

    test_helper_append_to_file(
        root.join(&summary),
        "Old [sync](../20261010T080000--weekly-sync.md).\n",
    );
    let out = test_helper_isolated_cli(&root)
        .args(["links", "check"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    let line = test_helper_read_file(root.join(&summary)).lines().count();
    assert_eq!(
        out,
        format!(
            "{summary}:{line}: broken link ../20261010T080000--weekly-sync.md (fixable: 20261010T080000)\n"
        )
    );
}

#[test]
fn test_fix_repairs_moves_and_denote_links() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_append_to_file(
        root.join(&summary),
        "\n[old](../20261010T080000--weekly-sync.md#agenda)\n\
         [typed](denote:20261010T080000)\n\
         [gone](../nothing-here.md)\n\
         [ghost](../20251231T235959--ghost.md)\n\
         ```\n[code](../20261010T080000--weekly-sync.md)\n```\n",
    );
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args(["links", "fix"]));
    assert!(out.contains("cannot resolve ../nothing-here.md"), "{out}");
    assert!(
        out.contains("cannot resolve ../20251231T235959--ghost.md"),
        "{out}"
    );

    let c = test_helper_read_file(root.join(&summary));
    assert!(
        c.contains("[old](../20261010T080000--weekly-sync__acme_meeting.md#agenda)"),
        "{c}"
    );
    assert!(
        c.contains("[typed](../20261010T080000--weekly-sync__acme_meeting.md)"),
        "{c}"
    );
    assert!(c.contains("[gone](../nothing-here.md)"));
    assert!(c.contains("[ghost](../20251231T235959--ghost.md)"));
    assert!(
        c.contains("[code](../20261010T080000--weekly-sync.md)"),
        "code block must be untouched"
    );
}

#[test]
fn test_fix_after_move() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_append_to_file(
        root.join(WEEKLY),
        "\n[s](zoom-summaries/20261010T090105--notes.md)\n",
    );
    // Move the summary note into the transcripts folder.
    let moved = root.join("src/notes/zoom-transcripts/20261010T090105--notes.md");
    fs::rename(root.join(&summary), &moved).unwrap();
    test_helper_isolated_cli(&root)
        .args(["links", "fix"])
        .assert()
        .success();
    assert!(
        test_helper_read_file(root.join(WEEKLY))
            .contains("[s](zoom-transcripts/20261010T090105--notes.md)")
    );
    test_helper_isolated_cli(&root)
        .args(["links", "check"])
        .assert()
        .success();
}

#[test]
fn test_backlinks_lists_linking_notes() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    let other = test_helper_add_meeting_note(&root, "10:00:00", "Roadmap", "meeting");
    test_helper_append_to_file(
        root.join(&summary),
        "\n[w](../20261010T080000--weekly-sync__acme_meeting.md)\n",
    );
    test_helper_append_to_file(root.join(&other), "\n[w](denote:20261010T080000)\n");
    // SUMMARY.md links to every note but is not a note itself.
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args([
        "links",
        "backlinks",
        "20261010T080000",
    ]));
    // Listed in identifier (chronological) order.
    assert_eq!(out, format!("{summary}\tNotes\n{other}\tRoadmap\n"));
}

#[test]
fn test_backlinks_ignore_denote_links_to_other_notes() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_add_meeting_note(&root, "10:00:00", "Roadmap", "meeting");
    test_helper_append_to_file(root.join(&summary), "\n[r](denote:20261010T100000)\n");
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args([
        "links",
        "backlinks",
        "20261010T080000",
    ]));
    assert!(!out.contains(&summary), "{out}");
}

#[test]
fn test_rename_image_is_rejected() {
    let (_t, root) = test_helper_init_notebook();
    let png = root.parent().unwrap().join("shot.png");
    fs::write(&png, b"\x89PNG").unwrap();
    test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("image")
            .arg(&png),
    );
    let out = test_helper_isolated_cli(&root)
        .args(["rename", "20261010T120000", "--title", "New"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&out);
    assert!(msg.contains("is an image, not a note"), "{msg}");
}

#[test]
fn test_rename_to_current_title_succeeds() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    test_helper_isolated_cli(&root)
        .args(["rename", "20261010T080000", "--title", "Weekly Sync"])
        .assert()
        .success();
}

#[test]
fn test_rename_title_fixes_inbound_links() {
    let (_t, root) = test_helper_init_notebook();
    let summary = test_helper_meeting_and_summary(&root);
    test_helper_append_to_file(
        root.join(&summary),
        "\n[w](../20261010T080000--weekly-sync__acme_meeting.md)\n",
    );
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args([
        "rename",
        "20261010T080000",
        "--title",
        "Acme Weekly",
    ]));
    let new = "src/notes/20261010T080000--acme-weekly__acme_meeting.md";
    assert_eq!(out.trim(), new);
    assert!(!root.join(WEEKLY).exists());
    let c = test_helper_read_file(root.join(new));
    assert!(c.contains("title = \"Acme Weekly\""));
    assert!(c.contains("identifier = \"20261010T080000\""));
    assert!(c.contains("\n# Acme Weekly\n"));
    assert!(
        test_helper_read_file(root.join(&summary))
            .contains("[w](../20261010T080000--acme-weekly__acme_meeting.md)")
    );
}

#[test]
fn test_rename_keywords_updates_tags() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(&root).args([
        "rename",
        "20261010T080000",
        "--keywords",
        "beta,meeting",
    ]));
    assert_eq!(
        out.trim(),
        "src/notes/20261010T080000--weekly-sync__beta_meeting.md"
    );
    let c = test_helper_read_file(root.join(out.trim()));
    assert!(c.contains("tags = [\"beta\", \"meeting\"]"));
    assert!(c.contains("\n# Weekly Sync\n"));
}

#[test]
fn test_rename_unknown_identifier() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    test_helper_isolated_cli(&root)
        .args(["rename", "20991231T000000", "--title", "X"])
        .assert()
        .failure();
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes")),
        vec![WEEKLY.rsplit('/').next().unwrap()]
    );
}
