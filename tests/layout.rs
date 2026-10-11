//! notebook-layout spec and identifier uniqueness.

mod common;

use std::fs;
use std::process::Command as StdCommand;

use common::*;
use tempfile::TempDir;

#[test]
fn test_init_creates_layout_and_builds() {
    let (_t, root) = test_helper_init_notebook();
    for f in ["book.toml", ".ignore", "src/SUMMARY.md", "src/README.md"] {
        assert!(root.join(f).is_file(), "missing {f}");
    }
    for d in [
        "src/notes",
        "src/notes/zoom-summaries",
        "src/notes/zoom-transcripts",
        "src/png",
    ] {
        assert!(root.join(d).is_dir(), "missing {d}");
    }
    assert_eq!(test_helper_read_file(root.join(".ignore")), "book/\n");
    let toml = test_helper_read_file(root.join("book.toml"));
    assert!(toml.contains("title = \"work-notes\""));
    assert!(toml.contains("[preprocessor.aa-denote-mdbook-tool]"));
    assert!(toml.contains("command = \"aa-denote-mdbook-tool preprocess\""));

    // Plain `mdbook build` works on a fresh notebook, given the binary on PATH.
    let status = StdCommand::new("mdbook")
        .arg("build")
        .arg(&root)
        .env("PATH", test_helper_path_with_bin_dir())
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(root.join("book/index.html").is_file());
}

#[test]
fn test_init_title_option() {
    let tmp = TempDir::new().unwrap();
    test_helper_isolated_cli(tmp.path())
        .args(["init", "nb", "--title", "Work Notes"])
        .assert()
        .success();
    assert!(
        test_helper_read_file(tmp.path().join("nb/book.toml"))
            .contains("[book]\ntitle = \"Work Notes\"")
    );
}

#[test]
fn test_init_refuses_existing_notebook() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("book.toml"), "mine\n").unwrap();
    let out = test_helper_isolated_cli(tmp.path())
        .arg("init")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8_lossy(&out).contains("book.toml"));
    assert_eq!(
        test_helper_read_file(tmp.path().join("book.toml")),
        "mine\n"
    );
    assert!(!tmp.path().join("src").exists());
}

#[test]
fn test_root_found_from_subdirectory() {
    let (_t, root) = test_helper_init_notebook();
    let sub = root.join("src/notes/zoom-summaries");
    let path = test_helper_run_ok_stdout(
        test_helper_cli_at_time(&sub, "2026-10-10", "08:00:00").args(["new", "--title", "Standup"]),
    );
    assert_eq!(path.trim(), "src/notes/20261010T080000--standup.md");
    assert!(root.join(path.trim()).is_file());
}

#[test]
fn test_book_option_beats_env() {
    let (_ta, a) = test_helper_init_notebook();
    let (_tb, b) = test_helper_init_notebook();
    let elsewhere = TempDir::new().unwrap();
    test_helper_cli_at_time(elsewhere.path(), "2026-10-10", "08:00:00")
        .env("AA_DENOTE_MDBOOK_TOOL_DIR", &a)
        .arg("--book")
        .arg(&b)
        .args(["new", "--title", "X"])
        .assert()
        .success();
    assert_eq!(test_helper_sorted_file_names(b.join("src/notes")).len(), 1);
    assert!(test_helper_sorted_file_names(a.join("src/notes")).is_empty());

    // The environment variable alone is used when --book is absent.
    test_helper_cli_at_time(elsewhere.path(), "2026-10-10", "08:00:00")
        .env("AA_DENOTE_MDBOOK_TOOL_DIR", &a)
        .args(["new", "--title", "Y"])
        .assert()
        .success();
    assert_eq!(test_helper_sorted_file_names(a.join("src/notes")).len(), 1);
}

#[test]
fn test_no_notebook_found() {
    let tmp = TempDir::new().unwrap();
    let out = test_helper_isolated_cli(tmp.path())
        .arg("new")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&out);
    assert!(
        msg.contains("aa-denote-mdbook-tool init") && msg.contains("--book"),
        "{msg}"
    );
}

#[test]
fn test_deleted_folders_are_recreated() {
    let (_t, root) = test_helper_init_notebook();
    fs::remove_dir_all(root.join("src/png")).unwrap();
    fs::remove_dir_all(root.join("src/notes/zoom-transcripts")).unwrap();
    let png = root.join("shot.png");
    fs::write(&png, b"\x89PNG").unwrap();
    test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
        .arg("image")
        .arg(&png)
        .assert()
        .success();
    assert_eq!(test_helper_sorted_file_names(root.join("src/png")).len(), 1);
    test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
        .args(["new", "--kind", "transcript", "--title", "T"])
        .assert()
        .success();
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/notes/zoom-transcripts")).len(),
        1
    );
}

#[test]
fn test_two_notes_in_the_same_second() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "A", "x");
    let second = test_helper_add_meeting_note(&root, "08:00:00", "B", "x");
    assert_eq!(second, "src/notes/20261010T080001--b__x.md");
}

#[test]
fn test_banner_on_stderr_only() {
    let (_t, root) = test_helper_init_notebook();
    let out = test_helper_cli_at_time(&root, "2026-10-10", "08:00:00")
        .args(["new", "--title", "X"])
        .assert()
        .success()
        .get_output()
        .clone();
    let banner = format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with(&banner));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "src/notes/20261010T080000--x.md\n"
    );

    // The preprocessor prints nothing extra.
    let out = test_helper_isolated_cli(&root)
        .args(["preprocess", "supports", "html"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(out.stderr.is_empty());
}
