use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[test]
fn cli_reports_invalid_arguments_as_json_with_stable_exit_code() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_chipdock-cli"))
        .args(["--json", "connect", "1", "VIRTUAL-A", "--stop-bits", "3"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "invalid_request");
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn mcp_stdio_initializes_and_lists_all_serial_tools() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chipdock-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    tokio::time::timeout(Duration::from_secs(5), async {
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"chipdock-test","version":"1"}
        }});
        input.write_all(format!("{init}\n").as_bytes()).await.unwrap();
        let initialized: Value = serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(initialized["result"]["serverInfo"]["name"], "chipdock-mcp");
        input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await.unwrap();
        input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n").await.unwrap();
        let reply: Value = serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 10);
        for tool in ["serial_records", "serial_send", "serial_select_protocol", "serial_open_plot"] {
            assert!(tools.iter().any(|item| item["name"] == tool));
        }
    }).await.unwrap();
    drop(input);
    tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .unwrap()
        .unwrap();
}
