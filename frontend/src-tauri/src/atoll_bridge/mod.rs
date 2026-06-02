// Meetily → Atoll notch bridge
// Pushes meeting state to Atoll's WebSocket RPC (localhost:9020)
// so the macOS notch reflects upcoming/active meetings.

pub mod commands;

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::Listener;
use std::sync::Arc;
use tokio::sync::RwLock;

const ATOLL_WS_URL: &str = "ws://localhost:9020";
const EXPERIENCE_ID: &str = "meetily-meeting";

/// Send a JSON-RPC 2.0 request to Atoll
async fn rpc_call(method: &str, params: serde_json::Value) -> Result<(), String> {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::SinkExt;

    let url = url::Url::parse(ATOLL_WS_URL).map_err(|e| e.to_string())?;
    let (mut ws, _) = connect_async(url)
        .await
        .map_err(|e| format!("Atoll WS connect failed: {}", e))?;

    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });

    ws.send(Message::Text(payload.to_string()))
        .await
        .map_err(|e| format!("Atoll WS send failed: {}", e))?;

    ws.close(None).await.ok();
    Ok(())
}

/// Present a meeting notification in the Atoll notch
pub async fn present_meeting(title: &str, subtitle: &str, style: &str) {
    let params = json!({
        "id": EXPERIENCE_ID,
        "title": format!("📅 {}", title),
        "subtitle": subtitle,
        "style": style,
        "duration": 10.0,
        "source": "meetily"
    });

    match rpc_call("atoll.presentNotchExperience", params).await {
        Ok(_) => info!("Atoll notch: presented meeting — {}", title),
        Err(e) => debug!("Atoll bridge: {} (Atoll may not be running)", e),
    }
}

/// Update an active meeting experience in the notch
pub async fn update_meeting(title: &str, subtitle: &str) {
    let params = json!({
        "id": EXPERIENCE_ID,
        "title": format!("🔴 {}", title),
        "subtitle": subtitle,
    });

    match rpc_call("atoll.updateNotchExperience", params).await {
        Ok(_) => debug!("Atoll notch: updated meeting — {}", title),
        Err(e) => debug!("Atoll bridge: {}", e),
    }
}

/// Dismiss the meeting experience from the notch
pub async fn dismiss_meeting() {
    let params = json!({
        "id": EXPERIENCE_ID
    });

    match rpc_call("atoll.dismissNotchExperience", params).await {
        Ok(_) => info!("Atoll notch: dismissed meeting"),
        Err(e) => debug!("Atoll bridge: {}", e),
    }
}

/// Hook into Meetily's meeting lifecycle events.
/// Call from main setup after detection loop starts.
pub fn setup_atoll_listener(app: &tauri::AppHandle<impl tauri::Runtime + 'static>) {
    let app_handle = app.clone();

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
    let app_handle2 = app.clone();
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
