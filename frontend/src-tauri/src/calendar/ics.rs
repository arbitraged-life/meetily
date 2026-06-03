// ICS parser — fetches and parses iCalendar format

use super::CalendarEvent;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use log::info;
use regex::Regex;

/// Fetch and parse an ICS URL into calendar events
pub async fn fetch_and_parse_ics(url: &str) -> Result<Vec<CalendarEvent>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to fetch ICS: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("ICS fetch returned status: {}", response.status()));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read ICS body: {}", e))?;

    parse_ics_content(&body)
}

/// Parse ICS content string into events
pub fn parse_ics_content(content: &str) -> Result<Vec<CalendarEvent>, String> {
    let mut events = Vec::new();
    let mut in_event = false;
    let mut current_event: Option<IcsEventBuilder> = None;

    for line in content.lines() {
        let line = line.trim_end_matches('\r');

        match line {
            "BEGIN:VEVENT" => {
                in_event = true;
                current_event = Some(IcsEventBuilder::default());
            }
            "END:VEVENT" => {
                in_event = false;
                if let Some(builder) = current_event.take() {
                    if let Some(event) = builder.build() {
                        events.push(event);
                    }
                }
            }
            _ if in_event => {
                if let Some(ref mut builder) = current_event {
                    builder.parse_line(line);
                }
            }
            _ => {}
        }
    }

    // Filter to upcoming events (next 7 days)
    let now = Utc::now();
    let week_from_now = now + chrono::Duration::days(7);
    let events: Vec<_> = events
        .into_iter()
        .filter(|e| e.end >= now && e.start <= week_from_now)
        .collect();

    info!("📅 Parsed {} upcoming calendar events", events.len());
    Ok(events)
}

#[derive(Default)]
struct IcsEventBuilder {
    uid: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    dtstart: Option<String>,
    dtend: Option<String>,
    location: Option<String>,
    attendees: Vec<String>,
}

impl IcsEventBuilder {
    fn parse_line(&mut self, line: &str) {
        if let Some(value) = line.strip_prefix("UID:") {
            self.uid = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("SUMMARY:") {
            self.summary = Some(unescape_ics(value));
        } else if let Some(value) = line.strip_prefix("DESCRIPTION:") {
            self.description = Some(unescape_ics(value));
        } else if line.starts_with("DTSTART") {
            if let Some(value) = line.split(':').last() {
                self.dtstart = Some(value.to_string());
            }
        } else if line.starts_with("DTEND") {
            if let Some(value) = line.split(':').last() {
                self.dtend = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("LOCATION:") {
            self.location = Some(unescape_ics(value));
        } else if line.starts_with("ATTENDEE") {
            // Extract email from ATTENDEE;...;CN=Name:mailto:email@example.com
            if let Some(email) = line.split("mailto:").last() {
                self.attendees.push(email.to_string());
            }
        }
    }

    fn build(self) -> Option<CalendarEvent> {
        let uid = self.uid?;
        let summary = self.summary.unwrap_or_else(|| "Untitled".to_string());
        let start = parse_ics_datetime(&self.dtstart?)?;
        let end = self.dtend.and_then(|d| parse_ics_datetime(&d)).unwrap_or(start + chrono::Duration::hours(1));

        // Detect conference URLs from description or location
        let conference_url = detect_conference_url(
            self.description.as_deref().unwrap_or(""),
            self.location.as_deref().unwrap_or(""),
        );

        Some(CalendarEvent {
            uid,
            summary,
            description: self.description,
            start,
            end,
            location: self.location,
            conference_url,
            attendees: self.attendees,
        })
    }
}

/// Parse ICS datetime format (e.g., 20240531T100000Z or 20240531T100000)
fn parse_ics_datetime(s: &str) -> Option<DateTime<Utc>> {
    // Try with Z suffix (UTC)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ") {
        return Some(Utc.from_utc_datetime(&dt));
    }
    // Try without Z (assume UTC)
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S") {
        return Some(Utc.from_utc_datetime(&dt));
    }
    // Try date-only
    if s.len() == 8 {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&format!("{}T000000", s), "%Y%m%dT%H%M%S") {
            return Some(Utc.from_utc_datetime(&dt));
        }
    }
    None
}

/// Detect video conferencing URLs in text
fn detect_conference_url(description: &str, location: &str) -> Option<String> {
    let combined = format!("{} {}", description, location);
    let patterns = [
        r"https://[a-z0-9]+\.zoom\.us/j/\S+",
        r"https://meet\.google\.com/\S+",
        r"https://teams\.microsoft\.com/l/meetup-join/\S+",
        r"https://[a-z0-9]+\.webex\.com/\S+",
    ];

    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(m) = re.find(&combined) {
                return Some(m.as_str().to_string());
            }
        }
    }
    None
}

/// Unescape ICS text encoding
fn unescape_ics(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}
