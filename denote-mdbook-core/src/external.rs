//! Running `mdbook`, `rg`, and the user's editor.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};

/// Run `program` and return its exit status, or a helpful error when it is
/// not installed.
pub fn run(mut cmd: Command, install_hint: &str) -> Result<u8> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    match cmd.status() {
        Ok(status) => Ok(status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .unwrap_or(1)),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            bail!("`{program}` was not found on PATH. {install_hint}")
        }
        Err(e) => Err(e).with_context(|| format!("cannot run `{program}`")),
    }
}

pub fn exit(code: u8) -> ExitCode {
    ExitCode::from(code)
}

/// `PATH` with this executable's directory first, so mdBook can always find
/// the `aa-denote-mdbook-tool preprocess` command it is configured with.
pub fn path_with_self() -> Option<OsString> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut dirs = vec![exe_dir];
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs).ok()
}

/// Open `file` in `$VISUAL`, else `$EDITOR`.
pub fn edit(file: &Path) -> Result<()> {
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|v| v.to_string_lossy().into_owned())
        .find(|v| !v.trim().is_empty());
    let Some(editor) = editor else {
        bail!(
            "cannot open an editor: set $VISUAL or $EDITOR (the note was created at {})",
            file.display()
        );
    };
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or_default();
    let mut cmd = Command::new(program);
    cmd.args(parts).arg(file);
    let code = run(cmd, "Check $VISUAL or $EDITOR.")?;
    if code != 0 {
        bail!("editor `{editor}` exited with status {code}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program that exists but cannot be executed (a directory) is a
    /// different failure from "not installed" and must not get the install hint.
    #[cfg(unix)]
    #[test]
    fn test_run_unexecutable_program_is_not_reported_missing() {
        let err = run(Command::new(std::env::temp_dir()), "INSTALL HINT").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("cannot run"), "{msg}");
        assert!(!msg.contains("INSTALL HINT"), "{msg}");
    }
}
