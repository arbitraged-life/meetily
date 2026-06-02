// Meeting auto-detection — monitors system audio + calendar to auto-start/stop recording

pub mod commands;

use crate::calendar::{CalendarConfig, CalendarState};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter};
use tokio::sync::RwLock;

static AUTO_DETECT_RUNNING: AtomicBool = AtomicBool::new(false);

/// Meeting detection state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionState {
    pub is_monitoring: bool,
    pub meeting_detected: bool,
    pub detection_reason: Option<String>,
    pub auto_recording_active: bool,
}

/// Detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub enabled: bool,
    /// Seconds of silence before auto-stopping
    pub silence_timeout_seconds: u64,
    /// Bundle IDs of meeting apps to monitor
    pub meeting_app_bundles: Vec<String>,
    /// Use calendar events for detection
    pub use_calendar: bool,
    /// Use audio activity detection
    pub use_audio_detection: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            silence_timeout_seconds: 30,
            meeting_app_bundles: vec![
                "us.zoom.xos".to_string(),
                "com.microsoft.teams".to_string(),
                "com.microsoft.teams2".to_string(),
                "com.google.Chrome".to_string(), // Meet via Chrome
                "com.brave.Browser".to_string(),
                "com.cisco.webexmeetingsapp".to_string(),
                "com.slack.Slack".to_string(),
            ],
            use_calendar: true,
            use_audio_detection: true,
        }
    }
}

/// Meeting detection reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionReason {
    CalendarEvent { event_title: String },
    MeetingAppActive { app_name: String },
    AudioActivity,
    Manual,
}

/// Start the meeting auto-detection loop
pub fn start_detection_loop(
    detection_config: Arc<RwLock<DetectionConfig>>,
    calendar_config: Arc<RwLock<CalendarConfig>>,
    calendar_state: CalendarState,
    app: tauri::AppHandle<impl tauri::Runtime + 'static>,
) -> Option<tokio::task::JoinHandle<()>> {
    if AUTO_DETECT_RUNNING.load(Ordering::SeqCst) {
        warn!("Meeting detection loop already running");
        return None;
    }

    AUTO_DETECT_RUNNING.store(true, Ordering::SeqCst);

    Some(tokio::spawn(async move {
        let mut meeting_in_progress = false;
        let mut silence_counter: u64 = 0;

        loop {
            let det_cfg = detection_config.read().await.clone();
            if !det_cfg.enabled {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            let mut should_record = false;
            let mut reason = None;

            // Check 1: Calendar event happening now
            if det_cfg.use_calendar {
                let cal_cfg = calendar_config.read().await;
                let active_events = crate::calendar::scheduler::get_active_events(
                    &calendar_state,
                    &*cal_cfg,
                )
                .await;

                if let Some(event) = active_events.first() {
                    should_record = true;
                    reason = Some(format!("Calendar: {}", event.summary));
                }
            }

            // Check 2: Meeting app is active with audio
            if !should_record && det_cfg.use_audio_detection {
                // Check if system audio is active (meeting apps producing sound)
                if is_meeting_app_using_audio(&det_cfg.meeting_app_bundles) {
                    should_record = true;
                    reason = Some("Meeting app audio detected".to_string());
                }
            }

            // State machine: start/stop recording
            if should_record && !meeting_in_progress {
                // START recording
                info!("🎙️ Auto-detect: Meeting detected — {:?}", reason);
                meeting_in_progress = true;
                silence_counter = 0;
                let _ = app.emit(
                    "meeting-auto-detected",
                    serde_json::json!({
                        "action": "start",
                        "reason": reason
                    }),
                );
            } else if !should_record && meeting_in_progress {
                // Increment silence counter
                silence_counter += 5; // 5 second poll interval
                if silence_counter >= det_cfg.silence_timeout_seconds {
                    // STOP recording
                    info!("🛑 Auto-detect: Meeting ended ({}s silence)", silence_counter);
                    meeting_in_progress = false;
                    silence_counter = 0;
                    let _ = app.emit(
                        "meeting-auto-detected",
                        serde_json::json!({
                            "action": "stop",
                            "reason": "silence_timeout"
                        }),
                    );
                }
            } else if should_record {
                silence_counter = 0;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }))
}

/// Check if any meeting app is currently using audio (macOS-specific)
fn is_meeting_app_using_audio(bundle_ids: &[String]) -> bool {
    // Use macOS CoreAudio to check if any process from our bundle list is using audio
    // Simplified: check running processes matching meeting app bundles
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Check audio tap processes — if meeting apps have active audio sessions
        if let Ok(output) = Command::new("lsof")
            .args(["-i", "UDP", "-n", "-P"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for bundle_id in bundle_ids {
                // Extract app name from bundle ID for process matching
                let app_name = bundle_id.split('.').last().unwrap_or("");
                if stdout.to_lowercase().contains(&app_name.to_lowercase()) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn stop_detection() {
    AUTO_DETECT_RUNNING.store(false, Ordering::SeqCst);
}
