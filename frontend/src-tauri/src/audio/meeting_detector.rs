use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Known meeting application process names (macOS)
const MEETING_PROCESSES: &[(&str, &str)] = &[
    ("zoom.us", "Zoom"),
    ("Microsoft Teams", "Microsoft Teams"),
    ("Google Chrome Helper", "Google Meet"),  // detected via window title
    ("Webex", "Webex"),
    ("Slack", "Slack Huddle"),
    ("Discord", "Discord"),
    ("FaceTime", "FaceTime"),
];

/// More specific detection: window titles that indicate active call
#[allow(dead_code)] // reserved for stricter window-title detection
const MEETING_WINDOW_INDICATORS: &[&str] = &[
    "Zoom Meeting",
    "Meeting in progress",
    "Google Meet",
    "meet.google.com",
    "Teams call",
    "Huddle",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedMeeting {
    pub app_name: String,
    pub process_name: String,
    pub detected_at: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeetingDetectionEvent {
    MeetingStarted(DetectedMeeting),
    MeetingEnded(DetectedMeeting),
}

pub struct MeetingDetector {
    enabled: Arc<AtomicBool>,
    event_sender: mpsc::UnboundedSender<MeetingDetectionEvent>,
    active_meetings: HashSet<String>,
}

impl MeetingDetector {
    pub fn new(event_sender: mpsc::UnboundedSender<MeetingDetectionEvent>) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            event_sender,
            active_meetings: HashSet::new(),
        }
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
        info!("🔍 Meeting auto-detection enabled");
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
        info!("🔍 Meeting auto-detection disabled");
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Poll running processes for meeting apps (macOS-specific)
    pub fn detect_meetings(&mut self) -> Vec<MeetingDetectionEvent> {
        if !self.is_enabled() {
            return Vec::new();
        }

        let mut events = Vec::new();
        let running = self.get_running_meeting_apps();

        // Detect new meetings
        for (process_name, app_name) in &running {
            if !self.active_meetings.contains(process_name) {
                let meeting = DetectedMeeting {
                    app_name: app_name.clone(),
                    process_name: process_name.clone(),
                    detected_at: chrono::Utc::now().to_rfc3339(),
                    is_active: true,
                };
                info!("🎙️ Meeting detected: {} ({})", app_name, process_name);
                events.push(MeetingDetectionEvent::MeetingStarted(meeting.clone()));
                let _ = self.event_sender.send(MeetingDetectionEvent::MeetingStarted(meeting));
                self.active_meetings.insert(process_name.clone());
            }
        }

        // Detect ended meetings
        let running_names: HashSet<String> = running.iter().map(|(p, _)| p.clone()).collect();
        let ended: Vec<String> = self.active_meetings
            .iter()
            .filter(|p| !running_names.contains(*p))
            .cloned()
            .collect();

        for process_name in ended {
            let meeting = DetectedMeeting {
                app_name: process_name.clone(),
                process_name: process_name.clone(),
                detected_at: chrono::Utc::now().to_rfc3339(),
                is_active: false,
            };
            info!("📴 Meeting ended: {}", process_name);
            events.push(MeetingDetectionEvent::MeetingEnded(meeting.clone()));
            let _ = self.event_sender.send(MeetingDetectionEvent::MeetingEnded(meeting));
            self.active_meetings.remove(&process_name);
        }

        events
    }

    /// Get currently running meeting apps using macOS `pgrep` / process list
    #[cfg(target_os = "macos")]
    fn get_running_meeting_apps(&self) -> Vec<(String, String)> {
        let mut found = Vec::new();

        // Use `ps aux` to check running processes
        let output = match std::process::Command::new("ps")
            .args(["-axo", "comm"])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to list processes: {}", e);
                return found;
            }
        };

        let process_list = String::from_utf8_lossy(&output.stdout);

        for (process_name, app_name) in MEETING_PROCESSES {
            // Skip generic processes that need window title check
            if *process_name == "Google Chrome Helper" {
                continue;
            }
            if process_list.lines().any(|line| line.contains(process_name)) {
                found.push((process_name.to_string(), app_name.to_string()));
            }
        }

        // For browser-based meetings (Google Meet), check via AppleScript
        if let Some(meet) = self.detect_browser_meeting() {
            found.push(meet);
        }

        found
    }

    #[cfg(not(target_os = "macos"))]
    fn get_running_meeting_apps(&self) -> Vec<(String, String)> {
        Vec::new() // Only macOS supported for now
    }

    /// Detect browser-based meetings via AppleScript window title check
    #[cfg(target_os = "macos")]
    fn detect_browser_meeting(&self) -> Option<(String, String)> {
        // Check Chrome tabs for Google Meet
        let script = r#"
            try
                tell application "System Events"
                    if exists process "Google Chrome" then
                        tell process "Google Chrome"
                            set windowNames to name of every window
                            repeat with wName in windowNames
                                if wName contains "Meet -" or wName contains "meet.google.com" then
                                    return "meet"
                                end if
                            end repeat
                        end tell
                    end if
                end tell
            end try
            return ""
        "#;

        let output = std::process::Command::new("osascript")
            .args(["-e", script])
            .output()
            .ok()?;

        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if result == "meet" {
            Some(("Google Meet".to_string(), "Google Meet".to_string()))
        } else {
            None
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn detect_browser_meeting(&self) -> Option<(String, String)> {
        None
    }
}

/// Background polling task for meeting detection
pub async fn run_meeting_detection_loop(
    enabled: Arc<AtomicBool>,
    event_sender: mpsc::UnboundedSender<MeetingDetectionEvent>,
) {
    let mut detector = MeetingDetector::new(event_sender);
    detector.enabled = enabled;

    let poll_interval = Duration::from_secs(5);

    loop {
        if detector.is_enabled() {
            detector.detect_meetings();
        }
        tokio::time::sleep(poll_interval).await;
    }
}
