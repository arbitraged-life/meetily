// Tauri commands for screen context

use super::{ScreenContextConfig, ScreenContextState, ScreenSnapshot};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::RwLock;

/// Tauri command: get all screen snapshots from current meeting
#[tauri::command]
pub async fn get_screen_context<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ScreenSnapshot>, String> {
    let state = app.state::<Arc<RwLock<ScreenContextState>>>();
    let s = state.read().await;
    Ok(s.snapshots.clone())
}

/// Tauri command: get current active window info (one-shot)
#[tauri::command]
pub async fn get_active_window_info<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<ScreenSnapshot, String> {
    let (app_name, window_title) = super::get_active_window()?;
    let url = super::get_browser_url(&app_name);
    Ok(ScreenSnapshot {
        timestamp: chrono::Utc::now(),
        audio_time: 0.0,
        active_app: app_name,
        window_title,
        url,
    })
}

/// Tauri command: update screen context config
#[tauri::command]
pub async fn set_screen_context_config<R: Runtime>(
    app: AppHandle<R>,
    config: ScreenContextConfig,
) -> Result<(), String> {
    let state = app.state::<Arc<RwLock<ScreenContextState>>>();
    let mut s = state.write().await;
    s.config = config;
    Ok(())
}

/// Tauri command: enable/disable screen context capture
#[tauri::command]
pub async fn toggle_screen_capture<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    let state = app.state::<Arc<RwLock<ScreenContextState>>>();
    let mut s = state.write().await;
    s.is_capturing = enabled;
    Ok(())
}
