use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use log::info;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

// ============================================================================
// GOOGLE CALENDAR INTEGRATION (#449)
// ============================================================================

/// OAuth2 token stored locally
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64, // Unix timestamp
}

/// A calendar event that may trigger recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: String,
    pub start_time: String,    // ISO 8601
    pub end_time: String,      // ISO 8601
    pub meeting_url: Option<String>, // Zoom/Meet/Teams link
    pub is_online: bool,
    pub organizer: Option<String>,
    pub attendees_count: usize,
}

/// Calendar integration state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarIntegrationStatus {
    pub is_connected: bool,
    pub account_email: Option<String>,
    pub auto_record_enabled: bool,
    pub upcoming_events: Vec<CalendarEvent>,
    pub next_event: Option<CalendarEvent>,
}

/// Configuration for calendar auto-recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarAutoRecordConfig {
    /// Only auto-record events with video/meeting links
    pub only_online_meetings: bool,
    /// Minutes before meeting to start recording
    pub start_offset_minutes: i64,
    /// Minutes after meeting end to stop recording
    pub stop_offset_minutes: i64,
    /// Minimum attendees to auto-record (1 = all events)
    pub min_attendees: usize,
    /// Keywords in event title that skip auto-record
    pub skip_keywords: Vec<String>,
}

impl Default for CalendarAutoRecordConfig {
    fn default() -> Self {
        Self {
            only_online_meetings: true,
            start_offset_minutes: 1,
            stop_offset_minutes: 2,
            min_attendees: 2,
            skip_keywords: vec![
                "lunch".to_string(),
                "block".to_string(),
                "focus".to_string(),
                "OOO".to_string(),
            ],
        }
    }
}

pub struct GoogleCalendarClient {
    token: Arc<AsyncMutex<Option<GoogleAuthToken>>>,
    config: Arc<AsyncMutex<CalendarAutoRecordConfig>>,
    enabled: Arc<AtomicBool>,
    client_id: String,
    client_secret: String,
}

impl GoogleCalendarClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            token: Arc::new(AsyncMutex::new(None)),
            config: Arc::new(AsyncMutex::new(CalendarAutoRecordConfig::default())),
            enabled: Arc::new(AtomicBool::new(false)),
            client_id,
            client_secret,
        }
    }

    /// Generate the OAuth2 authorization URL
    pub fn get_auth_url(&self, redirect_port: u16) -> String {
        let redirect_uri = format!("http://localhost:{}/callback", redirect_port);
        let scopes = "https://www.googleapis.com/auth/calendar.readonly";
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?\
            client_id={}&\
            redirect_uri={}&\
            response_type=code&\
            scope={}&\
            access_type=offline&\
            prompt=consent",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(scopes),
        )
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(&self, code: &str, redirect_port: u16) -> Result<GoogleAuthToken, String> {
        let redirect_uri = format!("http://localhost:{}/callback", redirect_port);
        let client = reqwest::Client::new();

        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("redirect_uri", &redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: i64,
        }

        let token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("Token parse failed: {}", e))?;

        let token = GoogleAuthToken {
            access_token: token_resp.access_token,
            refresh_token: token_resp.refresh_token,
            expires_at: Utc::now().timestamp() + token_resp.expires_in,
        };

        *self.token.lock().await = Some(token.clone());
        info!("✅ Google Calendar connected");
        Ok(token)
    }

    /// Refresh the access token using the refresh token
    pub async fn refresh_token(&self) -> Result<(), String> {
        let current = self.token.lock().await;
        let refresh_token = current
            .as_ref()
            .and_then(|t| t.refresh_token.clone())
            .ok_or("No refresh token available")?;
        drop(current);

        let client = reqwest::Client::new();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("refresh_token", refresh_token.as_str()),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| format!("Token refresh failed: {}", e))?;

        #[derive(Deserialize)]
        struct RefreshResponse {
            access_token: String,
            expires_in: i64,
        }

        let refresh_resp: RefreshResponse = resp
            .json()
            .await
            .map_err(|e| format!("Refresh parse failed: {}", e))?;

        let mut token = self.token.lock().await;
        if let Some(t) = token.as_mut() {
            t.access_token = refresh_resp.access_token;
            t.expires_at = Utc::now().timestamp() + refresh_resp.expires_in;
        }

        Ok(())
    }

    /// Get upcoming events from Google Calendar
    pub async fn get_upcoming_events(&self, minutes_ahead: i64) -> Result<Vec<CalendarEvent>, String> {
        // Ensure token is fresh
        {
            let token = self.token.lock().await;
            if let Some(t) = token.as_ref() {
                if Utc::now().timestamp() >= t.expires_at - 60 {
                    drop(token);
                    self.refresh_token().await?;
                }
            } else {
                return Err("Not authenticated".to_string());
            }
        }

        let token = self.token.lock().await;
        let access_token = token.as_ref().unwrap().access_token.clone();
        drop(token);

        let now = Utc::now();
        let time_max = now + chrono::Duration::minutes(minutes_ahead);

        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events?\
            timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=10",
            urlencoding::encode(&now.to_rfc3339()),
            urlencoding::encode(&time_max.to_rfc3339()),
        );

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| format!("Calendar API failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Calendar API error {}: {}", status, body));
        }

        #[derive(Deserialize)]
        struct EventsResponse {
            items: Option<Vec<GoogleEvent>>,
        }

        #[derive(Deserialize)]
        struct GoogleEvent {
            id: Option<String>,
            summary: Option<String>,
            start: Option<EventTime>,
            end: Option<EventTime>,
            #[serde(rename = "hangoutLink")]
            hangout_link: Option<String>,
            #[serde(rename = "conferenceData")]
            conference_data: Option<ConferenceData>,
            organizer: Option<Organizer>,
            attendees: Option<Vec<Attendee>>,
            description: Option<String>,
        }

        #[derive(Deserialize)]
        struct EventTime {
            #[serde(rename = "dateTime")]
            date_time: Option<String>,
        }

        #[derive(Deserialize)]
        struct ConferenceData {
            #[serde(rename = "entryPoints")]
            entry_points: Option<Vec<EntryPoint>>,
        }

        #[derive(Deserialize)]
        struct EntryPoint {
            uri: Option<String>,
            #[serde(rename = "entryPointType")]
            entry_point_type: Option<String>,
        }

        #[derive(Deserialize)]
        struct Organizer {
            email: Option<String>,
            #[serde(rename = "displayName")]
            display_name: Option<String>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)] // email parsed from API but not yet surfaced
        struct Attendee {
            email: Option<String>,
        }

        let events_resp: EventsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Events parse failed: {}", e))?;

        let events = events_resp.items.unwrap_or_default();
        let mut result = Vec::new();

        for event in events {
            let meeting_url = event.hangout_link.clone().or_else(|| {
                event.conference_data.as_ref().and_then(|cd| {
                    cd.entry_points.as_ref().and_then(|eps| {
                        eps.iter()
                            .find(|ep| ep.entry_point_type.as_deref() == Some("video"))
                            .and_then(|ep| ep.uri.clone())
                    })
                })
            }).or_else(|| {
                // Check description for meeting links
                event.description.as_ref().and_then(|desc| {
                    if desc.contains("zoom.us") || desc.contains("teams.microsoft.com") || desc.contains("meet.google.com") {
                        // Extract first URL-like thing
                        desc.split_whitespace()
                            .find(|w| w.starts_with("http"))
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            });

            let is_online = meeting_url.is_some();

            result.push(CalendarEvent {
                id: event.id.unwrap_or_default(),
                summary: event.summary.unwrap_or_else(|| "(No title)".to_string()),
                start_time: event.start.and_then(|s| s.date_time).unwrap_or_default(),
                end_time: event.end.and_then(|e| e.date_time).unwrap_or_default(),
                meeting_url,
                is_online,
                organizer: event.organizer.and_then(|o| o.display_name.or(o.email)),
                attendees_count: event.attendees.map(|a| a.len()).unwrap_or(0),
            });
        }

        Ok(result)
    }

    /// Check if an event should trigger auto-recording
    pub async fn should_auto_record(&self, event: &CalendarEvent) -> bool {
        let config = self.config.lock().await;

        if config.only_online_meetings && !event.is_online {
            return false;
        }

        if event.attendees_count < config.min_attendees {
            return false;
        }

        let title_lower = event.summary.to_lowercase();
        for keyword in &config.skip_keywords {
            if title_lower.contains(&keyword.to_lowercase()) {
                return false;
            }
        }

        true
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub async fn set_config(&self, config: CalendarAutoRecordConfig) {
        *self.config.lock().await = config;
    }

    pub async fn get_config(&self) -> CalendarAutoRecordConfig {
        self.config.lock().await.clone()
    }

    pub async fn is_authenticated(&self) -> bool {
        self.token.lock().await.is_some()
    }

    pub async fn set_token(&self, token: GoogleAuthToken) {
        *self.token.lock().await = Some(token);
    }

    pub async fn disconnect(&self) {
        *self.token.lock().await = None;
        self.enabled.store(false, Ordering::SeqCst);
        info!("📅 Google Calendar disconnected");
    }
}

/// Token persistence helpers
pub fn get_token_path() -> std::path::PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("meetily");
    path.push("google_calendar_token.json");
    path
}

pub fn save_token_to_disk(token: &GoogleAuthToken) -> Result<(), String> {
    let path = get_token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(token).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    info!("📅 Token saved to {:?}", path);
    Ok(())
}

pub fn load_token_from_disk() -> Option<GoogleAuthToken> {
    let path = get_token_path();
    let json = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&json).ok()
}
