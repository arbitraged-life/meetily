// Tauri commands for Atoll bridge (manual triggers from frontend)

use super::{dismiss_meeting, present_meeting};

/// Manually push a meeting notification to Atoll notch
#[tauri::command]
pub async fn atoll_notify_meeting(title: String, subtitle: String) -> Result<(), String> {
    present_meeting(&title, &subtitle, "compact").await;
    Ok(())
}

/// Dismiss the Atoll notch meeting display
#[tauri::command]
pub async fn atoll_dismiss_meeting() -> Result<(), String> {
    dismiss_meeting().await;
    Ok(())
}
