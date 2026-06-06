// Meetily MCP Server
// Provides MCP tools for external agents to access meeting transcripts,
// calendar events, dictionary, and live recording state.
//
// Transport: stdio (JSON-RPC 2.0)
// Communication with Meetily app: reads from shared filesystem paths
//   - ~/Documents/Meetily/transcripts/*.md (exported transcripts)
//   - ~/.local/share/meetily/last-export.json (latest export notification)
//   - ~/.config/unified-dictionary/dictionary.json (shared dictionary)

mod protocol;
mod tools;
mod resources;

use anyhow::Result;
use std::io::{self, BufRead, Write};

fn main() -> Result<()> {
    env_logger::init();

    // MCP stdio transport: read JSON-RPC from stdin, write to stdout
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        match protocol::handle_request(&line) {
            Ok(response) => {
                writeln!(stdout_lock, "{}", response)?;
                stdout_lock.flush()?;
            }
            Err(e) => {
                let error_response = protocol::error_response(None, -32603, &e.to_string());
                writeln!(stdout_lock, "{}", error_response)?;
                stdout_lock.flush()?;
            }
        }
    }

    Ok(())
}
