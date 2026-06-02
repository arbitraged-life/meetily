use log::info;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use super::google_calendar::{
    CalendarAutoRecordConfig, CalendarEvent, CalendarIntegrationStatus, GoogleCalendarClient, load_token_from_disk, save_token_to_disk,
};

static CALENDAR_CLIENT: Lazy<Arc<AsyncMutex<Option<GoogleCalendarClient>>>> =
    Lazy::new(|| Arc::new(AsyncMutex::new(None)));

/// Initialize calendar with OAuth credentials (from app config)
#[tauri::command]
pub async fn google_calendar_init(client_id: String, client_secret: String) -> Result<(), String> {
    let client = GoogleCalendarClient::new(client_id, client_secret);

    // Try to load existing token from disk
    if let Some(token) = load_token_from_disk() {
        client.set_token(token).await;
        info!("📅 Google Calendar: restored saved token");
    }

    *CALENDAR_CLIENT.lock().await = Some(client);
    Ok(())
}

/// Get OAuth URL to open in browser
#[tauri::command]
pub async fn google_calendar_get_auth_url() -> Result<String, String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    Ok(client.get_auth_url(17249))
}

/// Complete OAuth flow with authorization code
#[tauri::command]
pub async fn google_calendar_auth_callback(code: String) -> Result<(), String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    let token = client.exchange_code(&code, 17249).await?;
    save_token_to_disk(&token)?;
    Ok(())
}

/// Disconnect Google Calendar
#[tauri::command]
pub async fn google_calendar_disconnect() -> Result<(), String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    client.disconnect().await;
    let path = super::google_calendar::get_token_path();
    let _ = std::fs::remove_file(path);
    Ok(())
}

/// Get calendar integration status
#[tauri::command]
pub async fn google_calendar_get_status() -> Result<CalendarIntegrationStatus, String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;

    let is_connected = client.is_authenticated().await;
    let auto_record_enabled = client.is_enabled();

    let upcoming_events = if is_connected {
        client.get_upcoming_events(60).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let next_event = upcoming_events.first().cloned();

    Ok(CalendarIntegrationStatus {
        is_connected,
        account_email: None,
        auto_record_enabled,
        upcoming_events,
        next_event,
    })
}

/// Get upcoming events
#[tauri::command]
pub async fn google_calendar_get_events(minutes_ahead: Option<i64>) -> Result<Vec<CalendarEvent>, String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    client.get_upcoming_events(minutes_ahead.unwrap_or(60)).await
}

/// Enable/disable auto-recording from calendar
#[tauri::command]
pub async fn google_calendar_set_auto_record(enabled: bool) -> Result<(), String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    client.set_enabled(enabled);
    Ok(())
}

/// Update auto-record configuration
#[tauri::command]
pub async fn google_calendar_set_config(config: CalendarAutoRecordConfig) -> Result<(), String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    client.set_config(config).await;
    Ok(())
}

/// Get current auto-record configuration
#[tauri::command]
pub async fn google_calendar_get_config() -> Result<CalendarAutoRecordConfig, String> {
    let guard = CALENDAR_CLIENT.lock().await;
    let client = guard.as_ref().ok_or("Calendar not initialized")?;
    Ok(client.get_config().await)
}
