// Calendar scheduler — background task that polls ICS and maintains event cache

use super::{ics::fetch_and_parse_ics, CalendarConfig, CalendarState};
use log::{error, info};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Start the calendar polling background task
pub fn start_calendar_poller(
    config: Arc<RwLock<CalendarConfig>>,
    state: CalendarState,
) -> tauri::async_runtime::JoinHandle<()> {
    // Use Tauri's managed async runtime rather than a bare `tokio::spawn`.
    // This callback runs inside Tauri's setup/event-loop context where no
    // Tokio reactor is entered on the current thread, so `tokio::spawn`
    // panics with "there is no reactor running". `tauri::async_runtime::spawn`
    // always targets Tauri's global runtime and is safe here.
    tauri::async_runtime::spawn(async move {
        loop {
            let cfg = config.read().await.clone();

            if let Some(ref url) = cfg.ics_url {
                match fetch_and_parse_ics(url).await {
                    Ok(events) => {
                        info!("📅 Calendar sync: {} events loaded", events.len());
                        let mut state_guard = state.write().await;
                        *state_guard = events;
                    }
                    Err(e) => {
                        error!("📅 Calendar sync failed: {}", e);
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(
                cfg.poll_interval_minutes * 60,
            ))
            .await;
        }
    })
}

/// Get events that are active right now
pub async fn get_active_events(state: &CalendarState, config: &CalendarConfig) -> Vec<super::CalendarEvent> {
    let events = state.read().await;
    events
        .iter()
        .filter(|e| {
            e.is_active_now(
                config.pre_meeting_buffer_seconds,
                config.post_meeting_buffer_seconds,
            )
        })
        .cloned()
        .collect()
}
