// Meetily → Atoll notch bridge
// Pushes meeting state to Atoll's WebSocket RPC (localhost:9020)
// so the macOS notch reflects upcoming/active meetings.
//
// PROTOCOL NOTES (verified end-to-end against live Atoll ExtensionRPCService):
//   * JSON-RPC 2.0 `id` MUST be a STRING — Atoll's RPCRequest decodes id as
//     String; an integer id is rejected with -32700 before method dispatch.
//   * present/update params MUST wrap the payload in a `descriptor` key holding
//     a full AtollNotchExperienceDescriptor (id, bundleIdentifier, priority,
//     accentColor, metadata, + tab|minimalistic). Flat fields → -32602.
//   * The client must complete an `atoll.requestAuthorization` handshake for
//     its bundleIdentifier first, or present/update return -32001 (unauthorized).
//   * dismiss takes `experienceID` (not `id`).

pub mod commands;

use log::{debug, info};
use serde_json::{json, Value};
use tauri::Listener;

const ATOLL_WS_URL: &str = "ws://localhost:9020";
const EXPERIENCE_ID: &str = "meetily-meeting";
const BUNDLE_ID: &str = "com.meetily.ai";

/// Send a single JSON-RPC 2.0 request to Atoll and await its reply.
/// Returns the parsed `result`/`error` envelope, or a transport error string.
async fn rpc_call(method: &str, params: Value) -> Result<Value, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let url = url::Url::parse(ATOLL_WS_URL).map_err(|e| e.to_string())?;
    let (mut ws, _) = connect_async(url)
        .await
        .map_err(|e| format!("Atoll WS connect failed: {}", e))?;

    // Per-request unique id so a stray server notification can't be mistaken
    // for this call's reply. Still a JSON string (Atoll rejects integer ids).
    let req_id = {
        use std::sync::atomic::{AtomicU64, Ordering};
        static RPC_SEQ: AtomicU64 = AtomicU64::new(1);
        RPC_SEQ.fetch_add(1, Ordering::Relaxed).to_string()
    };

    let payload = json!({
        "jsonrpc": "2.0",
        "id": req_id,         // MUST be a string — see protocol notes above
        "method": method,
        "params": params
    });

    ws.send(Message::Text(payload.to_string()))
        .await
        .map_err(|e| format!("Atoll WS send failed: {}", e))?;

    // Read frames until we get the response matching our request id. Any
    // server-initiated notification (no id / mismatched id) is skipped rather
    // than consumed as our reply. A single per-REQUEST deadline (not per-frame)
    // bounds the total wait, so a steady stream of unrelated notifications can't
    // keep resetting the timeout and hang the spawned task indefinitely.
    const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    let deadline = tokio::time::Instant::now() + RPC_TIMEOUT;
    let reply = loop {
        let frame = tokio::time::timeout_at(deadline, ws.next())
            .await
            .map_err(|_| "Atoll RPC timed out waiting for reply".to_string())?;
        match frame {
            Some(Ok(Message::Text(txt))) => {
                let v: Value = serde_json::from_str(&txt)
                    .map_err(|e| format!("Atoll RPC reply was not valid JSON: {}", e))?;
                // Skip notifications / unrelated frames; only accept our id.
                match v.get("id").and_then(|id| id.as_str()) {
                    Some(id) if id == req_id => break v,
                    _ => continue,
                }
            }
            Some(Ok(Message::Close(_))) | None => {
                ws.close(None).await.ok();
                return Err("Atoll WS closed before sending a reply".to_string());
            }
            Some(Ok(_)) => continue, // ping/pong/binary — ignore
            Some(Err(e)) => {
                ws.close(None).await.ok();
                return Err(format!("Atoll WS recv failed: {}", e));
            }
        }
    };

    ws.close(None).await.ok();

    if let Some(err) = reply.get("error") {
        return Err(format!("Atoll RPC error: {}", err));
    }
    Ok(reply)
}

/// Ensure Atoll has authorized this bundle (idempotent, persists server-side).
async fn ensure_authorized() -> Result<(), String> {
    static AUTHORIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if AUTHORIZED.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    rpc_call(
        "atoll.requestAuthorization",
        json!({ "bundleIdentifier": BUNDLE_ID }),
    )
    .await
    .map(|_| {
        AUTHORIZED.store(true, std::sync::atomic::Ordering::Relaxed);
        ()
    })
}

/// Build a minimal-but-valid AtollNotchExperienceDescriptor for a meeting,
/// rendered via the minimalistic (music-replacement) layout.
fn meeting_descriptor(headline: &str, subtitle: &str) -> Value {
    json!({
        "id": EXPERIENCE_ID,
        "bundleIdentifier": BUNDLE_ID,
        "priority": "normal",
        "accentColor": { "red": -1, "green": -1, "blue": -1, "alpha": 1.0 }, // system accent
        "metadata": { "source": "meetily" },
        "minimalistic": {
            "headline": headline,
            "subtitle": subtitle,
            "sections": [],
            "layout": "stack",
            "hidesMusicControls": true
        },
        "durationHint": 10.0
    })
}

/// Present a meeting notification in the Atoll notch
pub async fn present_meeting(title: &str, subtitle: &str, _style: &str) {
    if let Err(e) = ensure_authorized().await {
        debug!("Atoll bridge: auth failed: {} (Atoll may not be running)", e);
        return;
    }
    let descriptor = meeting_descriptor(&format!("📅 {}", title), subtitle);
    match rpc_call("atoll.presentNotchExperience", json!({ "descriptor": descriptor })).await {
        Ok(_) => info!("Atoll notch: presented meeting — {}", title),
        Err(e) => debug!("Atoll bridge: {} (Atoll may not be running)", e),
    }
}

/// Update an active meeting experience in the notch
pub async fn update_meeting(title: &str, subtitle: &str) {
    if let Err(e) = ensure_authorized().await {
        debug!("Atoll bridge: auth failed: {} (Atoll may not be running)", e);
        return;
    }
    let descriptor = meeting_descriptor(&format!("🔴 {}", title), subtitle);
    match rpc_call("atoll.updateNotchExperience", json!({ "descriptor": descriptor })).await {
        Ok(_) => debug!("Atoll notch: updated meeting — {}", title),
        Err(e) => debug!("Atoll bridge: {}", e),
    }
}

/// Dismiss the meeting experience from the notch
pub async fn dismiss_meeting() {
    let params = json!({
        "experienceID": EXPERIENCE_ID,
        "bundleIdentifier": BUNDLE_ID
    });

    match rpc_call("atoll.dismissNotchExperience", params).await {
        Ok(_) => info!("Atoll notch: dismissed meeting"),
        Err(e) => debug!("Atoll bridge: {}", e),
    }
}

/// Hook into Meetily's meeting lifecycle events.
/// Call from main setup after detection loop starts.
pub fn setup_atoll_listener(app: &tauri::AppHandle<impl tauri::Runtime + 'static>) {
    // Listen for meeting-auto-detected events from meeting_detect module
    app.listen("meeting-auto-detected", move |event| {
        let payload: serde_json::Value =
            serde_json::from_str(event.payload()).unwrap_or_default();

        let action = payload["action"].as_str().unwrap_or("").to_string();
        let reason = payload["reason"].as_str().unwrap_or("Meeting");

        let reason_owned = reason.to_string();
        tauri::async_runtime::spawn(async move {
            match action.as_str() {
                "start" => {
                    present_meeting(&reason_owned, "Recording starting…", "compact").await;
                }
                "stop" => {
                    dismiss_meeting().await;
                }
                _ => {}
            }
        });
    });

    // Listen for recording state changes
    app.listen("recording-state-changed", move |event| {
        let payload: serde_json::Value =
            serde_json::from_str(event.payload()).unwrap_or_default();

        let is_recording = payload["recording"].as_bool().unwrap_or(false);
        let meeting_title = payload["title"]
            .as_str()
            .unwrap_or("Meeting")
            .to_string();

        tauri::async_runtime::spawn(async move {
            if is_recording {
                present_meeting(&meeting_title, "Recording…", "persistent").await;
            } else {
                dismiss_meeting().await;
            }
        });
    });

    info!("Atoll bridge: listener setup complete");
}
