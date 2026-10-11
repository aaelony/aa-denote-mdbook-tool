//! note-capture spec, including timestamp selection.

mod common;

use std::fs;

use common::*;

const WEEKLY: &str = "src/notes/20261010T080000--weekly-sync__acme_meeting.md";

#[test]
fn test_quick_manual_note() {
    let (_t, root) = test_helper_init_notebook();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "08:00:00").args([
            "new",
            "--title",
            "Weekly Sync",
            "--keywords",
            "meeting,acme",
        ]),
    );
    assert_eq!(out, format!("{WEEKLY}\n"));
    assert_eq!(
        test_helper_read_file(root.join(WEEKLY)),
        "+++\ntitle = \"Weekly Sync\"\ndate = 2026-10-10T08:00:00-07:00\n\
         tags = [\"acme\", \"meeting\"]\nidentifier = \"20261010T080000\"\n\
         kind = \"manual\"\n+++\n\n# Weekly Sync\n"
    );
}

#[test]
fn test_note_with_no_title() {
    let (_t, root) = test_helper_init_notebook();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "08:00:00").arg("new"),
    );
    assert_eq!(out.trim(), "src/notes/20261010T080000.md");
    let c = test_helper_read_file(root.join(out.trim()));
    assert!(c.ends_with("+++\n\n# 20261010T080000\n"), "{c}");
}

#[test]
fn test_time_only_date_and_no_imported_field() {
    let (_t, root) = test_helper_init_notebook();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "11:00:00")
            .args(["new", "--title", "Standup", "--date", "09:00"]),
    );
    assert_eq!(out.trim(), "src/notes/20261010T090000--standup.md");
    let c = test_helper_read_file(root.join(out.trim()));
    assert!(c.contains("date = 2026-10-10T09:00:00-07:00"));
    assert!(!c.contains("imported"));
}

#[test]
fn test_invalid_date_rejected() {
    let (_t, root) = test_helper_init_notebook();
    let err = test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
        .args(["new", "--date", "next tuesday"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8_lossy(&err).contains("accepted formats"));
    assert!(test_helper_sorted_file_names(root.join("src/notes")).is_empty());
}

#[test]
fn test_import_markdown_summary() {
    let (_t, root) = test_helper_init_notebook();
    let src = root.parent().unwrap().join("summary.md");
    fs::write(&src, "## Key points\n\n- budget approved\n").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args(["--kind", "summary", "--title", "Weekly Sync Summary"]),
    );
    let path = out.trim();
    assert!(path.starts_with("src/notes/zoom-summaries/20261010T120000--weekly-sync-summary"));
    let c = test_helper_read_file(root.join(path));
    assert!(
        c.ends_with("# Weekly Sync Summary\n\n## Key points\n\n- budget approved\n"),
        "{c}"
    );
    assert!(c.contains("kind = \"summary\""));
    assert!(c.contains("imported = 2026-10-10T12:00:00-07:00"));
    assert_eq!(
        test_helper_read_file(&src),
        "## Key points\n\n- budget approved\n"
    );
}

#[test]
fn test_import_vtt_is_fenced() {
    let (_t, root) = test_helper_init_notebook();
    let src = root.parent().unwrap().join("meeting.vtt");
    fs::write(
        &src,
        "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nAlice: hi\n",
    )
    .unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args(["--kind", "transcript"]),
    );
    let c = test_helper_read_file(root.join(out.trim()));
    assert!(out.starts_with("src/notes/zoom-transcripts/"));
    assert!(
        c.ends_with("```text\nWEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nAlice: hi\n```\n"),
        "{c}"
    );
}

#[test]
fn test_import_stdin_uses_clock() {
    let (_t, root) = test_helper_init_notebook();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .args(["import", "-", "--kind", "summary"])
            .write_stdin("pasted summary\n"),
    );
    assert_eq!(out.trim(), "src/notes/zoom-summaries/20261010T120000.md");
    assert!(test_helper_read_file(root.join(out.trim())).ends_with("\npasted summary\n"));
}

#[test]
fn test_non_utf8_rejected() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
        .args(["import", "-", "--kind", "summary"])
        .write_stdin(vec![0xff, 0xfe, 0x00])
        .assert()
        .failure();
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}

#[test]
fn test_zoom_file_name_timestamp() {
    let (_t, root) = test_helper_init_notebook();
    let src = root
        .parent()
        .unwrap()
        .join("GMT20261010-150012_Recording.transcript.vtt");
    fs::write(&src, "WEBVTT\n").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args(["--kind", "transcript"]),
    );
    assert_eq!(out.trim(), "src/notes/zoom-transcripts/20261010T080012.md");
}

#[test]
fn test_attach_transcript_to_meeting() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    let src = root.parent().unwrap().join("zoom.vtt");
    fs::write(&src, "WEBVTT\n").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args(["--kind", "transcript", "--parent", "20261010T080000"]),
    );
    let path = out.trim();
    let file_name = path.rsplit('/').next().unwrap();
    assert!(file_name.starts_with("20261010T080001"), "{path}");
    assert!(test_helper_read_file(root.join(path)).contains("parent = \"20261010T080000\""));
    let parent = test_helper_read_file(root.join(WEEKLY));
    assert!(
        parent.ends_with(&format!(
            "# Weekly Sync\n\n## Attachments\n\n- [Weekly Sync Transcript](zoom-transcripts/{file_name})\n"
        )),
        "{parent}"
    );
}

#[test]
fn test_defaults_from_parent() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .args([
                "import",
                "-",
                "--kind",
                "summary",
                "--parent",
                "20261010T080000",
            ])
            .write_stdin("s\n"),
    );
    assert_eq!(
        out.trim(),
        "src/notes/zoom-summaries/20261010T080001--weekly-sync-ai-summary__acme_meeting_zoomsummary.md"
    );
    let c = test_helper_read_file(root.join(out.trim()));
    assert!(c.contains("title = \"Weekly Sync AI Summary\""));
    assert!(c.contains("tags = [\"acme\", \"meeting\", \"zoomsummary\"]"));
}

#[test]
fn test_unknown_parent() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
        .args([
            "import",
            "-",
            "--kind",
            "summary",
            "--parent",
            "20261010T080000",
        ])
        .write_stdin("s\n")
        .assert()
        .failure();
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}

#[test]
fn test_noon_imports_land_after_their_meeting() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting,acme");
    test_helper_add_meeting_note(&root, "09:00:00", "Roadmap", "meeting");
    let summary = test_helper_add_child_note(&root, "summary", "20261010T080000");
    let transcript = test_helper_add_child_note(&root, "transcript", "20261010T080000");
    assert!(summary.contains("/20261010T080001--"), "{summary}");
    assert!(transcript.contains("/20261010T080002--"), "{transcript}");

    // Late-import front matter: date is the meeting time, imported is now.
    let c = test_helper_read_file(root.join(transcript.trim()));
    assert!(c.contains("date = 2026-10-10T08:00:02-07:00"), "{c}");
    assert!(c.contains("imported = 2026-10-10T12:00:00-07:00"), "{c}");
}

#[test]
fn test_date_overrides_parent() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let src = root.parent().unwrap().join("t.vtt");
    fs::write(&src, "WEBVTT\n").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args([
                "--kind",
                "transcript",
                "--parent",
                "20261010T080000",
                "--date",
                "2026-10-10 08:30",
            ]),
    );
    assert!(out.contains("/20261010T083000--"), "{out}");
}

#[test]
fn test_parent_overrides_zoom_file_name() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let src = root
        .parent()
        .unwrap()
        .join("GMT20261010-170000_Recording.transcript.vtt");
    fs::write(&src, "WEBVTT\n").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("import")
            .arg(&src)
            .args(["--kind", "transcript", "--parent", "20261010T080000"]),
    );
    assert!(out.contains("/20261010T080001--"), "{out}");
}

#[test]
fn test_next_day_import_grouped_under_meeting_date() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "10:00:00", "1:1", "oneonone");
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-11", "09:00:00")
            .args(["import", "-", "--kind", "transcript"])
            .args(["--parent", "20261010T100000"])
            .write_stdin("x\n"),
    );
    assert!(out.contains("/20261010T100001--"), "{out}");
    test_helper_isolated_cli(&root)
        .arg("summary")
        .assert()
        .success();
    let s = test_helper_read_file(root.join("src/SUMMARY.md"));
    assert!(!s.contains("# 2026-10-11"), "{s}");
    assert!(
        s.contains(
            "# 2026-10-10\n\n- [1:1](notes/20261010T100000--1-1__oneonone.md)\n  - [1:1 Transcript]"
        ),
        "{s}"
    );
}

#[test]
fn test_edit_launches_editor() {
    let (_t, root) = test_helper_init_notebook();
    let log = root.parent().unwrap().join("editor.log");
    let script = root.parent().unwrap().join("fake-editor.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\necho \"$@\" > {}\n", log.display()),
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
            .env("EDITOR", &script)
            .args(["new", "--title", "1:1", "--edit"]),
    );
    assert_eq!(
        test_helper_read_file(&log).trim(),
        root.join(out.trim()).display().to_string()
    );

    // VISUAL wins over EDITOR.
    let vlog = root.parent().unwrap().join("visual.log");
    let vscript = root.parent().unwrap().join("fake-visual.sh");
    fs::write(
        &vscript,
        format!("#!/bin/sh\necho \"$@\" > {}\n", vlog.display()),
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&vscript)
        .status()
        .unwrap();
    test_helper_cli_at_time(&root, "2026-10-10", "08:01:00")
        .env("EDITOR", &script)
        .env("VISUAL", &vscript)
        .args(["new", "--edit"])
        .assert()
        .success();
    assert!(vlog.is_file());
}

#[test]
fn test_edit_without_editor_fails_but_keeps_note() {
    let (_t, root) = test_helper_init_notebook();
    let err = test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
        .args(["new", "--title", "X", "--edit"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8_lossy(&err).contains("$VISUAL or $EDITOR"));
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes")),
        vec!["20261010T080000--x.md"]
    );
}

#[test]
fn test_image_for_summary_note() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "09:01:05")
            .args(["new", "--kind", "summary", "--title", "S"]),
    );
    let png = root.parent().unwrap().join("shot.png");
    fs::write(&png, b"\x89PNG").unwrap();
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("image")
            .arg(&png)
            .args(["--title", "Arch Diagram", "--for", "20261010T090105"]),
    );
    assert_eq!(
        out,
        "![Arch Diagram](../../png/20261010T120000--arch-diagram.png)\n"
    );
    assert!(
        root.join("src/png/20261010T120000--arch-diagram.png")
            .is_file()
    );

    // Without --for the reference is relative to src/notes/.
    let out = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
            .arg("image")
            .arg(&png),
    );
    assert_eq!(out, "![shot](../png/20261010T120001--shot.png)\n");
}

#[test]
fn test_non_png_rejected() {
    let (_t, root) = test_helper_init_notebook();
    let jpg = root.parent().unwrap().join("shot.jpg");
    fs::write(&jpg, b"x").unwrap();
    test_helper_cli_at_time(&root, "2026-10-10", "12:00:00")
        .arg("image")
        .arg(&jpg)
        .assert()
        .failure();
    assert!(test_helper_sorted_file_names(root.join("src/png")).is_empty());
}
