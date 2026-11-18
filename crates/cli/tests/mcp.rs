#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]
use mermaidcraft_cli::{info_json, mcp, render_bytes, Format};
use serde_json::json;

#[test]
fn initialize_and_list_tools() {
    let r = mcp::handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
    assert_eq!(r["result"]["serverInfo"]["name"], "mermaidcraft");
    let l = mcp::handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
    assert!(l["result"]["tools"].as_array().unwrap().len() >= 8);
    assert!(mcp::handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    let bad = mcp::handle(&json!({"jsonrpc":"2.0","id":3,"method":"nope"})).unwrap();
    assert_eq!(bad["error"]["code"], -32601);
}

#[test]
fn tool_validate_and_edit() {
    let call = |name: &str, args: serde_json::Value| mcp::handle(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
    let r = call("diagram_validate", json!({"source": "flowchart TD\nA-->"}));
    assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("\"valid\": false"));
    let r = call("flowchart_edit", json!({"source": "flowchart TD\nA-->B", "ops": [{"op":"add_node","id":"C","label":"Hi"},{"op":"add_edge","from":"B","to":"C"}]}));
    assert_eq!(r["result"]["isError"], false);
    assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("B --> C"));
    let r = call("flowchart_edit", json!({"source": "sequenceDiagram\nA->>B: x", "ops": [{"op":"add_node"}]}));
    assert_eq!(r["result"]["isError"], true);
    let r = call("diagram_write", json!({"path": "/etc/passwd", "source": "flowchart TD\nA"}));
    assert_eq!(r["result"]["isError"], true);
}

#[test]
fn info_and_render() {
    let v = info_json("erDiagram\nA ||--o{ B : x", false);
    assert_eq!(v["type"], "erDiagram");
    assert_eq!(v["edges"], 1);
    assert!(render_bytes("flowchart TD\nA-->B", Format::Png, 2.0).unwrap().starts_with(&[0x89]));
    assert!(render_bytes("flowchart TD\nA-->", Format::Svg, 1.0).is_err());
}
