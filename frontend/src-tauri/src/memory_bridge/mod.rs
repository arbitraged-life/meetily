// Meetily → Memory system bridge (OpenMemory / mem0)
// On meeting save, pushes the transcript into the centralized vector memory
// store so it becomes queryable by Hermes / agents.
//
// Endpoint (OpenMemory REST):  POST {base}/api/v1/memories/
//   body: { user_id, text, metadata, infer, app }
//
// Philosophy: Meetily is a data source, not an AI endpoint. It just feeds
// the transcript downstream; all summarization/intelligence lives in the
// centralized pipeline. This hook is fire-and-forget and NEVER blocks or
// fails the local save.

use log::{debug, info, warn};
use serde_json::json;

/// Base URL of the OpenMemory API. Override with MEETILY_MEMORY_URL.
fn memory_base_url() -> String {
    std::env::var("MEETILY_MEMORY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8765".to_string())
}

/// user_id the memories are filed under. Override with MEETILY_MEMORY_USER.
fn memory_user_id() -> String {
    std::env::var("MEETILY_MEMORY_USER").unwrap_or_else(|_| "bobby45".to_string())
}

/// Whether the bridge is enabled. Disabled by setting MEETILY_MEMORY_ENABLED=0/false.
fn memory_enabled() -> bool {
    match std::env::var("MEETILY_MEMORY_ENABLED") {
        Ok(v) => !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "no" | "off"),
        Err(_) => true,
    }
}

/// Build a plain-text transcript body from the saved segments.
/// `segments` is the raw Vec<serde_json::Value> as received by api_save_transcript.
fn build_transcript_text(title: &str, segments: &[serde_json::Value]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Meeting transcript: {}\n\n", title));
    for seg in segments {
        let text = seg.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let ts = seg
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if ts.is_empty() {
            out.push_str(text);
        } else {
            out.push_str(&format!("[{}] {}", ts, text));
        }
        out.push('\n');
    }
    out
}

/// Push a saved meeting transcript into the centralized memory store.
/// Fire-and-forget: spawns a task, logs on failure, never propagates errors.
pub fn push_meeting_transcript(
    meeting_id: String,
    title: String,
    segments: Vec<serde_json::Value>,
) {
    if !memory_enabled() {
        debug!("Memory bridge disabled (MEETILY_MEMORY_ENABLED) — skipping push");
        return;
    }
    if segments.is_empty() {
        debug!("Memory bridge: empty transcript for '{}' — skipping push", title);
        return;
    }

    tauri::async_runtime::spawn(async move {
        let text = build_transcript_text(&title, &segments);
        if text.trim().is_empty() {
            debug!("Memory bridge: no usable text for '{}' — skipping", title);
            return;
        }

        let base = memory_base_url();
        let url = format!("{}/api/v1/memories/", base.trim_end_matches('/'));
        let user_id = memory_user_id();

        let body = json!({
            "user_id": user_id,
            "text": text,
            // Don't let the LLM rewrite/extract — store the transcript verbatim.
            "infer": false,
            "app": "meetily",
            "metadata": {
                "source": "meetily",
                "type": "meeting_transcript",
                "meeting_id": meeting_id,
                "title": title,
                "segment_count": segments.len(),
            }
        });

        let client = reqwest::Client::new();
        match client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    info!(
                        "Memory bridge: pushed transcript '{}' ({} segments) → {}",
                        title,
                        segments.len(),
                        url
                    );
                } else {
                    let detail = resp.text().await.unwrap_or_default();
                    warn!(
                        "Memory bridge: push failed for '{}' — HTTP {} {}",
                        title, status, detail
                    );
                }
            }
            Err(e) => {
                // Memory store may simply be down — log at debug, don't alarm.
                debug!(
                    "Memory bridge: could not reach memory store at {} ({}). Transcript saved locally regardless.",
                    url, e
                );
            }
        }
    });
}
