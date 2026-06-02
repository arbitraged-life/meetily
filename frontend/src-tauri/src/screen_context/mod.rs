// Screen context module — captures active window/app info during meetings
// Uses macOS Accessibility API to get window titles and app names

pub mod commands;

use chrono::{DateTime, Utc};
use log::info;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A snapshot of what's on screen at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    pub timestamp: DateTime<Utc>,
    pub audio_time: f64, // Recording-relative time
    pub active_app: String,
    pub window_title: String,
    pub url: Option<String>, // Browser URL if detectable
}

/// Configuration for screen context capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContextConfig {
    pub enabled: bool,
    /// Interval between captures (seconds)
    pub capture_interval_secs: u64,
    /// Whether to capture window titles (requires accessibility permission)
    pub capture_titles: bool,
    /// Whether to attempt URL extraction from browsers
    pub capture_urls: bool,
    /// Apps to ignore (e.g. Meetily itself)
    pub ignored_apps: Vec<String>,
}

impl Default for ScreenContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_interval_secs: 30,
            capture_titles: true,
            capture_urls: true,
            ignored_apps: vec![
                "Meetily".to_string(),
                "loginwindow".to_string(),
                "ScreenSaverEngine".to_string(),
            ],
        }
    }
}

/// State for screen context capture during a meeting
pub struct ScreenContextState {
    pub config: ScreenContextConfig,
    pub snapshots: Vec<ScreenSnapshot>,
    pub is_capturing: bool,
}

impl ScreenContextState {
    pub fn new(config: ScreenContextConfig) -> Self {
        Self {
            config,
            snapshots: Vec::new(),
            is_capturing: false,
        }
    }
}

impl Default for ScreenContextState {
    fn default() -> Self {
        Self::new(ScreenContextConfig::default())
    }
}

/// Get the currently focused application and window title using AppleScript
pub fn get_active_window() -> Result<(String, String), String> {
    // Get frontmost app name
    let app_output = Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get name of first application process whose frontmost is true"])
        .output()
        .map_err(|e| format!("osascript failed: {}", e))?;

    let app_name = String::from_utf8_lossy(&app_output.stdout).trim().to_string();

    // Get window title
    let title_script = format!(
        "tell application \"System Events\" to get name of front window of application process \"{}\"",
        app_name
    );
    let title_output = Command::new("osascript")
        .args(["-e", &title_script])
        .output()
        .map_err(|e| format!("osascript title failed: {}", e))?;

    let window_title = if title_output.status.success() {
        String::from_utf8_lossy(&title_output.stdout).trim().to_string()
    } else {
        String::new()
    };

    Ok((app_name, window_title))
}

/// Try to get the current browser URL (Safari, Chrome, Arc, Firefox)
pub fn get_browser_url(app_name: &str) -> Option<String> {
    let script = match app_name {
        "Safari" | "Safari Technology Preview" => {
            "tell application \"Safari\" to get URL of front document"
        }
        "Google Chrome" | "Google Chrome Canary" => {
            "tell application \"Google Chrome\" to get URL of active tab of front window"
        }
        "Arc" => {
            "tell application \"Arc\" to get URL of active tab of front window"
        }
        "Firefox" => return None, // Firefox doesn't support AppleScript URL access
        _ => return None,
    };

    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() && url != "missing value" {
            Some(url)
        } else {
            None
        }
    } else {
        None
    }
}

/// Capture a screen context snapshot
pub fn capture_snapshot(audio_time: f64, config: &ScreenContextConfig) -> Option<ScreenSnapshot> {
    let (app_name, window_title) = get_active_window().ok()?;

    // Skip ignored apps
    if config.ignored_apps.iter().any(|ignored| app_name.contains(ignored)) {
        return None;
    }

    let url = if config.capture_urls {
        get_browser_url(&app_name)
    } else {
        None
    };

    Some(ScreenSnapshot {
        timestamp: Utc::now(),
        audio_time,
        active_app: app_name,
        window_title: if config.capture_titles { window_title } else { String::new() },
        url,
    })
}

/// Start periodic screen context capture
pub async fn start_capture_loop(
    state: Arc<RwLock<ScreenContextState>>,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    info!("🖥️ Starting screen context capture");

    loop {
        let interval = {
            let s = state.read().await;
            if !s.is_capturing {
                break;
            }
            s.config.capture_interval_secs
        };

        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(interval)) => {
                let config = {
                    let s = state.read().await;
                    s.config.clone()
                };

                // audio_time would come from the recording manager — for now use elapsed
                if let Some(snapshot) = capture_snapshot(0.0, &config) {
                    let mut s = state.write().await;
                    s.snapshots.push(snapshot);
                }
            }
            _ = stop_rx.changed() => {
                break;
            }
        }
    }

    info!("🖥️ Screen context capture stopped");
}
