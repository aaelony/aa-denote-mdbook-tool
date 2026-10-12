//! mcp-server spec.

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use common::*;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A running `mcp` server that has completed the initialize handshake.
struct McpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    initialize_result: Value,
}

impl McpSession {
    fn test_helper_send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn test_helper_request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.test_helper_send(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed stdout"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == json!(id) {
                return message["result"].clone();
            }
        }
    }

    /// Call a tool and return its result (content, structuredContent, isError).
    fn test_helper_call(&mut self, tool: &str, arguments: Value) -> Value {
        self.test_helper_request("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    /// Call a tool that must succeed and return its structured content.
    fn test_helper_call_ok(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.test_helper_call(tool, arguments);
        assert_eq!(result["isError"], json!(false), "{result}");
        result["structuredContent"].clone()
    }

    /// Call a tool that must fail and return its error text.
    fn test_helper_call_err(&mut self, tool: &str, arguments: Value) -> String {
        let result = self.test_helper_call(tool, arguments);
        assert_eq!(result["isError"], json!(true), "{result}");
        result["content"][0]["text"].as_str().unwrap().to_string()
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The `mcp` server command for `root`, isolated like the CLI tests, at local
/// time `hms` on 2026-10-10 (UTC-7).
fn test_helper_mcp_command(root: &Path, hms: &str) -> Command {
    let mut c = Command::new(BIN);
    c.arg("--book")
        .arg(root)
        .arg("mcp")
        .env("TZ", "Etc/GMT+7")
        .env(
            "AA_DENOTE_MDBOOK_TOOL_NOW",
            format!("2026-10-10T{hms}-07:00"),
        )
        .env_remove("AA_DENOTE_MDBOOK_TOOL_DIR")
        .env_remove("VISUAL")
        .env_remove("EDITOR");
    c
}

/// Start the server for `root` at 2026-10-10 `hms` and perform the
/// initialize handshake.
fn test_helper_mcp_session(root: &Path, hms: &str) -> McpSession {
    let mut child = test_helper_mcp_command(root, hms)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut session = McpSession {
        stdin: child.stdin.take().unwrap(),
        stdout: BufReader::new(child.stdout.take().unwrap()),
        child,
        next_id: 0,
        initialize_result: Value::Null,
    };
    session.initialize_result = session.test_helper_request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "tests", "version": "0"}
        }),
    );
    session.test_helper_send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    session
}

#[test]
fn test_mcp_listed_in_help() {
    let out = test_helper_run_ok_stdout(test_helper_isolated_cli(Path::new(".")).arg("--help"));
    assert!(
        out.contains("mcp      Run an MCP server over stdio for this notebook"),
        "{out}"
    );
}

#[test]
fn test_mcp_initialize_reports_name_and_version() {
    let (_t, root) = test_helper_init_notebook();
    let session = test_helper_mcp_session(&root, "12:00:00");
    let info = &session.initialize_result;
    assert_eq!(info["serverInfo"]["name"], json!("aa-denote-mdbook-tool"));
    assert_eq!(
        info["serverInfo"]["version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert!(info["capabilities"]["tools"].is_object(), "{info}");
}

#[test]
fn test_mcp_stdout_is_only_json_rpc() {
    let (_t, root) = test_helper_init_notebook();
    let input = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "tests", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "new_note", "arguments": {"title": "Acme Sync"}}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "search_notes", "arguments": {"pattern": "Acme"}}}),
    ]
    .iter()
    .map(|m| format!("{m}\n"))
    .collect::<String>();
    let mut child = test_helper_mcp_command(&root, "12:00:00")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    // Closing stdin ends the session.
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let ids: Vec<Value> = stdout
        .lines()
        .map(|line| {
            let message: Value = serde_json::from_str(line).unwrap();
            assert_eq!(message["jsonrpc"], json!("2.0"), "{line}");
            message["id"].clone()
        })
        .collect();
    assert_eq!(ids, [json!(1), json!(2), json!(3)]);
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("aa-denote-mdbook-tool "));
}

#[test]
fn test_mcp_outside_notebook_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    let out = test_helper_isolated_cli(tmp.path())
        .arg("mcp")
        .write_stdin("")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no notebook found"));
}

#[test]
fn test_mcp_unknown_id_is_tool_error_and_server_continues() {
    let (_t, root) = test_helper_init_notebook();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let message = session.test_helper_call_err("backlinks", json!({"id": "20000101T000000"}));
    assert!(message.contains("20000101T000000"), "{message}");
    let listed = session.test_helper_call_ok("list_notes", json!({}));
    assert_eq!(listed["notes"], json!([]));
}

#[test]
fn test_mcp_tools_list_is_exact() {
    let (_t, root) = test_helper_init_notebook();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let tools = session.test_helper_request("tools/list", json!({}));
    let mut names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "attach_file",
            "backlinks",
            "check_links",
            "fix_links",
            "import_note",
            "list_notes",
            "make_link",
            "new_note",
            "read_note",
            "regenerate_summary",
            "rename_note",
            "search_notes",
        ]
    );
}

#[test]
fn test_mcp_list_notes_filters_by_kind_and_keyword() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "meeting,acme");
    test_helper_add_meeting_note(&root, "09:00:00", "Beta Sync", "meeting,beta");
    test_helper_add_child_note(&root, "summary", "20261010T080000");
    let mut session = test_helper_mcp_session(&root, "12:00:00");

    let all = session.test_helper_call_ok("list_notes", json!({}));
    let ids: Vec<&str> = all["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["20261010T090000", "20261010T080001", "20261010T080000"]
    );

    let summaries = session.test_helper_call_ok("list_notes", json!({"kind": "summary"}));
    assert_eq!(summaries["notes"].as_array().unwrap().len(), 1);
    assert_eq!(summaries["notes"][0]["kind"], json!("summary"));
    assert_eq!(
        summaries["notes"][0]["title"],
        json!("Acme Sync AI Summary")
    );

    let acme_meetings = session.test_helper_call_ok(
        "list_notes",
        json!({"kind": "manual", "keywords": ["acme"]}),
    );
    assert_eq!(
        acme_meetings["notes"],
        json!([{
            "id": "20261010T080000",
            "title": "Acme Sync",
            "kind": "manual",
            "keywords": ["acme", "meeting"],
            "path": "src/notes/20261010T080000--acme-sync__acme_meeting.md",
        }])
    );
}

#[test]
fn test_mcp_read_note_returns_front_matter_and_body() {
    let (_t, root) = test_helper_init_notebook();
    let path = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let note = session.test_helper_call_ok("read_note", json!({"id": "20261010T080000"}));
    assert_eq!(note["path"], json!(path));
    assert_eq!(
        note["content"],
        json!(test_helper_read_file(root.join(&path)))
    );
    assert!(note["content"].as_str().unwrap().starts_with("+++\n"));
}

#[test]
fn test_mcp_read_note_rejects_attachment() {
    let (_t, root) = test_helper_init_notebook();
    fs::write(root.join("src/png/20261010T081000--diagram.png"), b"x").unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let message = session.test_helper_call_err("read_note", json!({"id": "20261010T081000"}));
    assert!(
        message.contains("is an attachment, not a note"),
        "{message}"
    );
}

#[test]
fn test_mcp_search_notes_keyword_filter() {
    let (_t, root) = test_helper_init_notebook();
    let acme = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let beta = test_helper_add_meeting_note(&root, "09:00:00", "Beta Sync", "beta");
    for path in [&acme, &beta] {
        let mut content = test_helper_read_file(root.join(path));
        content.push_str("\nroadmap review\n");
        fs::write(root.join(path), content).unwrap();
    }
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let found = session.test_helper_call_ok(
        "search_notes",
        json!({"pattern": "roadmap", "keywords": ["acme"]}),
    );
    let text = found["text"].as_str().unwrap();
    assert!(text.contains(&format!("{acme}:")), "{text}");
    assert!(text.contains("roadmap review"), "{text}");
    assert!(!text.contains(&beta), "{text}");
}

#[test]
fn test_mcp_search_notes_no_match_is_empty_success() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let found = session.test_helper_call_ok("search_notes", json!({"pattern": "no-such-text"}));
    assert_eq!(found["text"], json!(""));
}

#[test]
fn test_mcp_new_note_matches_cli_and_updates_summary() {
    let (_cli_t, cli_root) = test_helper_init_notebook();
    let (_mcp_t, mcp_root) = test_helper_init_notebook();
    let cli_path = test_helper_add_meeting_note(&cli_root, "08:00:00", "Acme Sync", "acme,meeting");

    let mut session = test_helper_mcp_session(&mcp_root, "08:00:00");
    let created = session.test_helper_call_ok(
        "new_note",
        json!({"title": "Acme Sync", "keywords": ["acme", "meeting"], "kind": "manual"}),
    );
    assert_eq!(created["id"], json!("20261010T080000"));
    assert_eq!(created["path"], json!(cli_path));
    assert_eq!(
        test_helper_read_file(mcp_root.join(&cli_path)),
        test_helper_read_file(cli_root.join(&cli_path))
    );
    let summary = test_helper_read_file(mcp_root.join("src/SUMMARY.md"));
    assert!(
        summary.contains("- [2026-10-10](days/2026-10-10.md)\n"),
        "{summary}"
    );
    assert!(
        summary.contains("  - [Acme Sync](notes/20261010T080000--acme-sync__acme_meeting.md)\n"),
        "{summary}"
    );
}

#[test]
fn test_mcp_reference_notes() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "meeting,acme");
    let mut session = test_helper_mcp_session(&root, "09:00:00");

    let created = session.test_helper_call_ok(
        "new_note",
        json!({"title": "Glossary", "kind": "reference"}),
    );
    assert_eq!(
        created["path"],
        json!("src/reference/20261010T090000--glossary.md")
    );
    let summary = test_helper_read_file(root.join("src/SUMMARY.md"));
    assert!(
        summary.contains(
            "- [Reference](reference.md)\n  - [Glossary](reference/20261010T090000--glossary.md)\n"
        ),
        "{summary}"
    );

    let references = session.test_helper_call_ok("list_notes", json!({"kind": "reference"}));
    assert_eq!(
        references["notes"],
        json!([{
            "id": "20261010T090000",
            "title": "Glossary",
            "kind": "reference",
            "keywords": [],
            "path": "src/reference/20261010T090000--glossary.md",
        }])
    );

    let err = session.test_helper_call_err(
        "new_note",
        json!({"title": "Terms", "kind": "reference", "parent": "20261010T080000"}),
    );
    assert!(
        err.contains("reference notes cannot have a parent"),
        "{err}"
    );
    assert_eq!(
        test_helper_sorted_file_names(root.join("src/reference")),
        ["20261010T090000--glossary.md"]
    );
}

#[test]
fn test_mcp_import_note_from_content_links_parent() {
    let (_t, root) = test_helper_init_notebook();
    let parent = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nhttps://example.com said hi\n";
    let created = session.test_helper_call_ok(
        "import_note",
        json!({
            "kind": "transcript",
            "content": vtt,
            "file_name": "meeting.vtt",
            "parent": "20261010T080000",
        }),
    );
    let path = created["path"].as_str().unwrap();
    assert!(path.starts_with("src/notes/zoom-transcripts/"), "{path}");
    let note = test_helper_read_file(root.join(path));
    // Verbatim: fenced, and the URL is not turned into a link.
    assert!(note.contains("```text\nWEBVTT\n"), "{note}");
    assert!(note.contains("\nhttps://example.com said hi\n"), "{note}");
    let file_name = Path::new(path).file_name().unwrap().to_str().unwrap();
    assert!(
        test_helper_read_file(root.join(&parent)).contains(file_name),
        "parent does not link the transcript"
    );
    assert!(test_helper_read_file(root.join("src/SUMMARY.md")).contains(file_name));
}

#[test]
fn test_mcp_import_note_rejects_path_and_content_together() {
    let (t, root) = test_helper_init_notebook();
    let file = t.path().join("summary.md");
    fs::write(&file, "# Summary\n").unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let message = session.test_helper_call_err(
        "import_note",
        json!({"kind": "summary", "path": file, "content": "text"}),
    );
    assert!(
        message.contains("exactly one of path or content"),
        "{message}"
    );
    assert!(test_helper_sorted_file_names(root.join("src/notes/zoom-summaries")).is_empty());
}

#[test]
fn test_mcp_attach_pdf_returns_link_syntax() {
    let (t, root) = test_helper_init_notebook();
    let file = t.path().join("plan.pdf");
    fs::write(&file, b"%PDF").unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let attached =
        session.test_helper_call_ok("attach_file", json!({"path": file, "title": "Plan"}));
    assert_eq!(attached["path"], json!("src/pdf/20261010T120000--plan.pdf"));
    assert_eq!(
        attached["reference"],
        json!("[Plan](../pdf/20261010T120000--plan.pdf)")
    );
    assert!(root.join("src/pdf/20261010T120000--plan.pdf").is_file());
}

#[test]
fn test_mcp_attach_unsupported_extension_is_tool_error() {
    let (t, root) = test_helper_init_notebook();
    let file = t.path().join("plan.docx");
    fs::write(&file, b"x").unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let message = session.test_helper_call_err("attach_file", json!({"path": file}));
    assert!(
        message.contains("is not a supported attachment"),
        "{message}"
    );
}

#[test]
fn test_mcp_make_link_and_backlinks() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let retro = test_helper_add_meeting_note(&root, "09:00:00", "Retro", "team");
    let mut session = test_helper_mcp_session(&root, "12:00:00");

    let printed = session.test_helper_call_ok("make_link", json!({"target": "20261010T080000"}));
    assert_eq!(
        printed,
        json!({"link": "[Acme Sync](20261010T080000--acme-sync__acme.md)", "appended_to": null})
    );

    let appended = session.test_helper_call_ok(
        "make_link",
        json!({"target": "20261010T080000", "from": "20261010T090000", "append": true}),
    );
    assert_eq!(appended["appended_to"], json!(retro));
    assert!(
        test_helper_read_file(root.join(&retro))
            .ends_with("\n[Acme Sync](20261010T080000--acme-sync__acme.md)\n")
    );

    let back = session.test_helper_call_ok("backlinks", json!({"id": "20261010T080000"}));
    assert_eq!(back["notes"], json!([{"path": retro, "title": "Retro"}]));
}

#[test]
fn test_mcp_check_links_reports_broken_link_as_success() {
    let (_t, root) = test_helper_init_notebook();
    let note = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let mut content = test_helper_read_file(root.join(&note));
    content.push_str("\n[gone](missing.md)\n");
    fs::write(root.join(&note), content).unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let checked = session.test_helper_call_ok("check_links", json!({}));
    let broken = checked["broken"].as_array().unwrap();
    assert_eq!(broken.len(), 1, "{checked}");
    assert_eq!(broken[0]["file"], json!(note));
    assert_eq!(broken[0]["dest"], json!("missing.md"));
    assert_eq!(broken[0]["fixable_id"], Value::Null);
}

#[test]
fn test_mcp_fix_links_repairs_renamed_target() {
    let (_t, root) = test_helper_init_notebook();
    let target = test_helper_add_meeting_note(&root, "08:00:00", "Acme Sync", "acme");
    let retro = test_helper_add_meeting_note(&root, "09:00:00", "Retro", "team");
    let mut content = test_helper_read_file(root.join(&retro));
    content.push_str("\n[Acme](20261010T080000--old-title__acme.md)\n");
    fs::write(root.join(&retro), content).unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let fixed = session.test_helper_call_ok("fix_links", json!({}));
    assert_eq!(
        fixed["fixed"],
        json!([{
            "file": retro,
            "line": fixed["fixed"][0]["line"],
            "old": "20261010T080000--old-title__acme.md",
            "new": "20261010T080000--acme-sync__acme.md",
        }])
    );
    assert!(
        test_helper_read_file(root.join(&retro))
            .contains("[Acme](20261010T080000--acme-sync__acme.md)")
    );
    assert!(root.join(&target).is_file());
}

#[test]
fn test_mcp_rename_note_updates_summary() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let renamed = session.test_helper_call_ok(
        "rename_note",
        json!({"id": "20261010T080000", "title": "Acme Kickoff"}),
    );
    assert_eq!(
        renamed,
        json!({"id": "20261010T080000", "path": "src/notes/20261010T080000--acme-kickoff__meeting.md"})
    );
    let summary = test_helper_read_file(root.join("src/SUMMARY.md"));
    assert!(summary.contains("[Acme Kickoff](notes/20261010T080000--acme-kickoff__meeting.md)"));
    assert!(!summary.contains("weekly-sync"));
}

#[test]
fn test_mcp_rename_note_with_nothing_to_change_is_tool_error() {
    let (_t, root) = test_helper_init_notebook();
    let note = test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let message = session.test_helper_call_err("rename_note", json!({"id": "20261010T080000"}));
    assert!(message.contains("nothing to rename"), "{message}");
    assert!(root.join(note).is_file());
}

#[test]
fn test_mcp_regenerate_summary() {
    let (_t, root) = test_helper_init_notebook();
    test_helper_add_meeting_note(&root, "08:00:00", "Weekly Sync", "meeting");
    let summary_path = root.join("src/SUMMARY.md");
    let regenerated = test_helper_read_file(&summary_path);
    fs::write(&summary_path, "# Summary\n").unwrap();
    let mut session = test_helper_mcp_session(&root, "12:00:00");
    let result = session.test_helper_call_ok("regenerate_summary", json!({}));
    assert_eq!(result["path"], json!("src/SUMMARY.md"));
    assert!(test_helper_read_file(&summary_path).contains("Weekly Sync"));
    assert!(regenerated.contains("Weekly Sync"));
}
