use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn fixture_vault() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/context-engine/tests/fixtures/vault")
}

struct McpSession {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl McpSession {
    fn start() -> Self {
        Self::start_at(&fixture_vault())
    }

    fn start_at(vault: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_context"))
            .arg("serve")
            .arg(vault)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start context MCP server");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("server stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("session is open");
        serde_json::to_writer(&mut *stdin, message).expect("serialize MCP message");
        stdin.write_all(b"\n").expect("terminate MCP message");
        stdin.flush().expect("flush MCP message");
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));

        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).expect("read MCP response") > 0,
                "server closed before responding to request {id}"
            );
            let response: Value = serde_json::from_str(&line).expect("valid MCP response JSON");
            if response.get("id").and_then(Value::as_u64) == Some(id) {
                return response;
            }
        }
    }

    fn initialize(&mut self) {
        let initialized = self.request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "context-integration-test",
                    "version": "0.1.0"
                }
            }),
        );
        assert!(initialized["result"]["capabilities"]["tools"].is_object());
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn close(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("wait for MCP server");
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("server stderr")
            .read_to_string(&mut stderr)
            .expect("read server stderr");
        assert!(status.success(), "server failed: {stderr}");
    }
}

#[test]
fn stdio_session_initializes_and_drives_all_tools() {
    let mut session = McpSession::start();

    let initialized = session.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "context-integration-test",
                "version": "0.1.0"
            }
        }),
    );
    assert_eq!(
        initialized["result"]["protocolVersion"],
        json!("2025-11-25")
    );
    assert!(initialized["result"]["capabilities"]["tools"].is_object());

    session.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    let tools = session.request(2, "tools/list", json!({}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .expect("tool list")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names, ["edit_section", "get_section", "outline", "search"]);

    let outline = session.request(
        3,
        "tools/call",
        json!({
            "name": "outline",
            "arguments": {"file": "player.md"}
        }),
    );
    assert_eq!(
        outline["result"]["structuredContent"]["sections"][0]["heading_path"],
        json!("Skills")
    );

    let section = session.request(
        4,
        "tools/call",
        json!({
            "name": "get_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Skills > Gun"
            }
        }),
    );
    assert_eq!(
        section["result"]["structuredContent"]["content"],
        json!("### Gun\nFire the equipped weapon.")
    );
    assert_eq!(
        section["result"]["structuredContent"]["provenance"]["heading_path"],
        json!("Skills > Gun")
    );

    let search = session.request(
        5,
        "tools/call",
        json!({
            "name": "search",
            "arguments": {"query": "gun skill"}
        }),
    );
    assert!(
        search["result"]["structuredContent"]["results"]
            .as_array()
            .expect("search results")
            .iter()
            .any(|result| result["provenance"]["file"] == "player.md"
                && result["provenance"]["heading_path"] == "Skills > Gun")
    );

    let missing = session.request(
        6,
        "tools/call",
        json!({
            "name": "get_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Skills > Cannon"
            }
        }),
    );
    assert_eq!(missing["result"]["isError"], json!(true));
    assert!(
        missing["result"]["structuredContent"]["message"]
            .as_str()
            .expect("error message")
            .contains("Skills > Cannon")
    );
    assert!(
        missing["result"]["structuredContent"]["suggestions"]
            .as_array()
            .expect("error suggestions")
            .iter()
            .any(|suggestion| suggestion == "Skills > Gun")
    );

    session.close();
}

#[test]
fn edit_section_round_trip_over_stdio() {
    let vault = tempfile::TempDir::new().expect("create temp vault");
    let player = "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nFire the equipped weapon.\n\n## Inventory\nitems\n";
    std::fs::write(vault.path().join("player.md"), player).expect("write fixture");

    let mut session = McpSession::start_at(vault.path());
    session.initialize();

    let section = session.request(
        2,
        "tools/call",
        json!({
            "name": "get_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Player > Skills > Gun"
            }
        }),
    );
    let hash = section["result"]["structuredContent"]["provenance"]["content_hash"]
        .as_str()
        .expect("read supplies the edit token")
        .to_owned();
    assert_eq!(hash.len(), 64);

    let edited = session.request(
        3,
        "tools/call",
        json!({
            "name": "edit_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Player > Skills > Gun",
                "body": "Deal 3 damage.",
                "expected_hash": hash
            }
        }),
    );
    assert_ne!(edited["result"]["isError"], json!(true));
    let written = std::fs::read_to_string(vault.path().join("player.md")).expect("read back");
    assert_eq!(
        written,
        "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nDeal 3 damage.\n\n## Inventory\nitems\n"
    );
    let gun = &edited["result"]["structuredContent"]["sections"][0]["children"][0]["children"][0];
    assert_eq!(gun["heading_path"], json!("Player > Skills > Gun"));
    let fresh_hash = gun["content_hash"].as_str().expect("fresh hash").to_owned();
    assert_ne!(fresh_hash, hash);

    let conflict = session.request(
        4,
        "tools/call",
        json!({
            "name": "edit_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Player > Skills > Gun",
                "body": "stale write",
                "expected_hash": hash
            }
        }),
    );
    assert_eq!(conflict["result"]["isError"], json!(true));
    assert_eq!(
        conflict["result"]["structuredContent"]["current_hash"],
        json!(fresh_hash)
    );

    let escape = session.request(
        5,
        "tools/call",
        json!({
            "name": "edit_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Player > Skills > Gun",
                "body": "## Escape Attempt",
                "expected_hash": fresh_hash
            }
        }),
    );
    assert_eq!(escape["result"]["isError"], json!(true));
    assert!(
        escape["result"]["structuredContent"]["message"]
            .as_str()
            .expect("escape message")
            .contains("Escape Attempt")
    );

    let reread = session.request(
        6,
        "tools/call",
        json!({
            "name": "get_section",
            "arguments": {
                "file": "player.md",
                "heading_path": "Player > Skills > Gun"
            }
        }),
    );
    assert_eq!(
        reread["result"]["structuredContent"]["content"],
        json!("### Gun\nDeal 3 damage.")
    );

    session.close();
}

#[test]
fn invalid_vault_fails_before_serving_and_names_the_path() {
    let missing = fixture_vault().join("does-not-exist");
    let output = Command::new(env!("CARGO_BIN_EXE_context"))
        .arg("serve")
        .arg(&missing)
        .output()
        .expect("run context");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
    assert!(stderr.contains(&missing.display().to_string()));
    assert!(stderr.contains("does not exist"));
}
