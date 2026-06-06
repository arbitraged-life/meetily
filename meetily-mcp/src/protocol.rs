// MCP JSON-RPC protocol implementation

use serde::Deserialize;
use serde_json::{json, Value};
use crate::tools;
use crate::resources;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

pub fn handle_request(line: &str) -> anyhow::Result<String> {
    let req: JsonRpcRequest = serde_json::from_str(line)?;

    let result = match req.method.as_str() {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "meetily-mcp",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "listChanged": false }
            }
        }),
        "notifications/initialized" => return Ok(String::new()),
        "tools/list" => tools::list_tools(),
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            tools::call_tool(name, &arguments)?
        }
        "resources/list" => resources::list_resources(),
        "resources/read" => {
            let params = req.params.unwrap_or(json!({}));
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            resources::read_resource(uri)?
        }
        _ => {
            return Ok(error_response(req.id, -32601, &format!("Method not found: {}", req.method)));
        }
    };

    Ok(json!({
        "jsonrpc": "2.0",
        "id": req.id,
        "result": result
    })
    .to_string())
}

pub fn error_response(id: Option<Value>, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}
