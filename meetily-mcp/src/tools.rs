// MCP Tools — operations agents can invoke

use serde_json::{json, Value};
use std::path::PathBuf;

pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "meetily_list_meetings",
                "description": "List recent meeting transcripts with dates and titles",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Max results (default 20)", "default": 20 },
                        "search": { "type": "string", "description": "Optional keyword search filter" }
                    }
                }
            },
            {
                "name": "meetily_get_transcript",
                "description": "Get the full transcript of a meeting by filename",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string", "description": "Transcript filename (from list_meetings)" }
                    },
                    "required": ["filename"]
                }
            },
            {
                "name": "meetily_search_transcripts",
                "description": "Full-text search across all meeting transcripts",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "default": 10 }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "meetily_get_latest",
                "description": "Get the most recently exported meeting transcript",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "meetily_dictionary_list",
                "description": "List all entries in the shared dictionary",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "meetily_dictionary_add",
                "description": "Add a word/phrase to the shared dictionary",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "display": { "type": "string", "description": "Correct display form" },
                        "aliases": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Alternative spellings/pronunciations"
                        }
                    },
                    "required": ["display", "aliases"]
                }
            },
            {
                "name": "meetily_dictionary_remove",
                "description": "Remove a dictionary entry by ID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }
            }
        ]
    })
}

pub fn call_tool(name: &str, arguments: &Value) -> anyhow::Result<Value> {
    match name {
        "meetily_list_meetings" => list_meetings(arguments),
        "meetily_get_transcript" => get_transcript(arguments),
        "meetily_search_transcripts" => search_transcripts(arguments),
        "meetily_get_latest" => get_latest(arguments),
        "meetily_dictionary_list" => dictionary_list(),
        "meetily_dictionary_add" => dictionary_add(arguments),
        "meetily_dictionary_remove" => dictionary_remove(arguments),
        _ => Ok(json!({
            "content": [{ "type": "text", "text": format!("Unknown tool: {}", name) }],
            "isError": true
        })),
    }
}

fn transcripts_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Documents")
        .join("Meetily")
        .join("transcripts")
}

fn dictionary_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("unified-dictionary")
        .join("dictionary.json")
}

fn list_meetings(args: &Value) -> anyhow::Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let search = args.get("search").and_then(|v| v.as_str()).unwrap_or("");
    let dir = transcripts_dir();

    if !dir.exists() {
        return Ok(tool_text("No transcripts directory found. Record a meeting first."));
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();

    // Sort by modification time, newest first
    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    let mut results = Vec::new();
    for entry in entries.into_iter().take(limit) {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !search.is_empty() && !filename.to_lowercase().contains(&search.to_lowercase()) {
            continue;
        }
        results.push(filename);
    }

    Ok(tool_text(&results.join("\n")))
}

fn get_transcript(args: &Value) -> anyhow::Result<Value> {
    let filename = args
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("filename required"))?;

    let path = transcripts_dir().join(filename);
    if !path.exists() {
        return Ok(tool_text(&format!("Transcript not found: {}", filename)));
    }

    let content = std::fs::read_to_string(&path)?;
    Ok(tool_text(&content))
}

fn search_transcripts(args: &Value) -> anyhow::Result<Value> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("query required"))?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let dir = transcripts_dir();

    if !dir.exists() {
        return Ok(tool_text("No transcripts directory found."));
    }

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
        if results.len() >= limit {
            break;
        }
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "md") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.to_lowercase().contains(&query_lower) {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                // Extract context around match
                if let Some(pos) = content.to_lowercase().find(&query_lower) {
                    let start = pos.saturating_sub(100);
                    let end = (pos + query.len() + 100).min(content.len());
                    let snippet = &content[start..end];
                    results.push(format!("## {}\n...{}...\n", filename, snippet.trim()));
                }
            }
        }
    }

    if results.is_empty() {
        Ok(tool_text(&format!("No results for '{}'", query)))
    } else {
        Ok(tool_text(&results.join("\n---\n")))
    }
}

fn get_latest(_args: &Value) -> anyhow::Result<Value> {
    let notify_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local")
        .join("share")
        .join("meetily")
        .join("last-export.json");

    if !notify_path.exists() {
        return Ok(tool_text("No meetings exported yet."));
    }

    let meta = std::fs::read_to_string(&notify_path)?;
    let info: Value = serde_json::from_str(&meta)?;

    // Read the actual transcript
    if let Some(path) = info.get("transcript_path").and_then(|v| v.as_str()) {
        if let Ok(content) = std::fs::read_to_string(path) {
            return Ok(tool_text(&content));
        }
    }

    Ok(tool_text(&format!("Latest export metadata:\n{}", meta)))
}

fn dictionary_list() -> anyhow::Result<Value> {
    let path = dictionary_path();
    if !path.exists() {
        return Ok(tool_text("Dictionary is empty."));
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(tool_text(&content))
}

fn dictionary_add(args: &Value) -> anyhow::Result<Value> {
    let display = args
        .get("display")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("display required"))?;
    let aliases: Vec<String> = args
        .get("aliases")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let path = dictionary_path();
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;

    let mut entries: Vec<Value> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    let entry = json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "display": display,
        "aliases": aliases,
        "source": "mcp",
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    entries.push(entry.clone());

    let json_str = serde_json::to_string_pretty(&entries)?;
    std::fs::write(&path, json_str)?;

    Ok(tool_text(&format!("Added: {} (aliases: {:?})", display, aliases)))
}

fn dictionary_remove(args: &Value) -> anyhow::Result<Value> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("id required"))?;

    let path = dictionary_path();
    if !path.exists() {
        return Ok(tool_text("Dictionary is empty."));
    }

    let content = std::fs::read_to_string(&path)?;
    let mut entries: Vec<Value> = serde_json::from_str(&content)?;
    let before = entries.len();
    entries.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(id));
    let removed = before - entries.len();

    let json_str = serde_json::to_string_pretty(&entries)?;
    std::fs::write(&path, json_str)?;

    Ok(tool_text(&format!("Removed {} entries", removed)))
}

fn tool_text(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }]
    })
}
