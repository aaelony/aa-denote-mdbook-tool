#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

pub const BIN: &str = env!("CARGO_BIN_EXE_aa-denote-mdbook-tool");

/// A command with a fixed time zone (UTC-7) and no inherited notebook or
/// editor settings.
pub fn test_helper_isolated_cli(cwd: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.current_dir(cwd)
        .env("TZ", "Etc/GMT+7")
        .env_remove("AA_DENOTE_MDBOOK_TOOL_DIR")
        .env_remove("AA_DENOTE_MDBOOK_TOOL_NOW")
        .env_remove("VISUAL")
        .env_remove("EDITOR");
    c
}

/// A command running at local time `hms` on `date` (UTC-7).
pub fn test_helper_cli_at_time(cwd: &Path, date: &str, hms: &str) -> Command {
    let mut c = test_helper_isolated_cli(cwd);
    c.env("AA_DENOTE_MDBOOK_TOOL_NOW", format!("{date}T{hms}-07:00"));
    c
}

pub fn test_helper_run_ok_stdout(c: &mut Command) -> String {
    let out = c.assert().success().get_output().stdout.clone();
    String::from_utf8(out).unwrap()
}

/// A fresh notebook in a temp dir; returns (guard, root).
pub fn test_helper_init_notebook() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("work-notes");
    test_helper_isolated_cli(tmp.path())
        .arg("init")
        .arg(&root)
        .assert()
        .success();
    let root = root.canonicalize().unwrap();
    (tmp, root)
}

pub fn test_helper_read_file(p: impl AsRef<Path>) -> String {
    fs::read_to_string(p).unwrap()
}

/// Add a `kind` (summary or transcript) note under `parent` via `import -` at
/// 2026-10-10 12:00:00; returns its relative path.
pub fn test_helper_add_child_note(root: &Path, kind: &str, parent: &str) -> String {
    test_helper_run_ok_stdout(
        test_helper_cli_at_time(root, "2026-10-10", "12:00:00")
            .args(["import", "-", "--kind", kind, "--parent", parent])
            .write_stdin("x\n"),
    )
    .trim()
    .to_string()
}

/// Create a manual note at 2026-10-10 `hms`; returns its relative path.
pub fn test_helper_add_meeting_note(root: &Path, hms: &str, title: &str, keywords: &str) -> String {
    test_helper_run_ok_stdout(test_helper_cli_at_time(root, "2026-10-10", hms).args([
        "new",
        "--title",
        title,
        "--keywords",
        keywords,
    ]))
    .trim()
    .to_string()
}

/// Names of files in a directory, sorted.
pub fn test_helper_sorted_file_names(dir: impl AsRef<Path>) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// PATH with the binary's directory first, as `build` does.
pub fn test_helper_path_with_bin_dir() -> String {
    let dir = Path::new(BIN)
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    format!("{dir}:{}", std::env::var("PATH").unwrap_or_default())
}
