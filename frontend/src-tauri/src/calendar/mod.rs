// Calendar module — ICS subscription for meeting detection

pub mod commands;
pub mod ics;
pub mod scheduler;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A calendar event parsed from ICS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub location: Option<String>,
    /// Detected conferencing URL (Zoom, Meet, Teams)
    pub conference_url: Option<String>,
    /// Attendee emails if available
    pub attendees: Vec<String>,
}

impl CalendarEvent {
    /// Check if event is happening right now (with buffer)
    pub fn is_active_now(&self, pre_buffer_secs: i64, post_buffer_secs: i64) -> bool {
        let now = Utc::now();
        let start_with_buffer = self.start - chrono::Duration::seconds(pre_buffer_secs);
        let end_with_buffer = self.end + chrono::Duration::seconds(post_buffer_secs);
        now >= start_with_buffer && now <= end_with_buffer
    }

    /// Check if this looks like a video meeting
    pub fn has_video_link(&self) -> bool {
        self.conference_url.is_some()
    }
}

/// Calendar subscription settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    pub ics_url: Option<String>,
    pub poll_interval_minutes: u64,
    pub auto_record_enabled: bool,
    pub pre_meeting_buffer_seconds: i64,
    pub post_meeting_buffer_seconds: i64,
    /// Only auto-record meetings with video links
    pub require_video_link: bool,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            ics_url: None,
            poll_interval_minutes: 5,
            auto_record_enabled: false,
            pre_meeting_buffer_seconds: 60,
            post_meeting_buffer_seconds: 120,
            require_video_link: false,
        }
    }
}

/// Shared state for calendar events
pub type CalendarState = Arc<RwLock<Vec<CalendarEvent>>>;

pub fn new_calendar_state() -> CalendarState {
    Arc::new(RwLock::new(Vec::new()))
}

// Google Calendar OAuth integration (#449)
pub mod google_calendar;
pub mod google_commands;
