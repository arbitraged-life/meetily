// Export hooks module — writes enriched markdown transcripts on meeting completion
// and emits events via UDS for external consumers

use crate::state::AppState;
use chrono::Utc;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub mod commands;

/// Configuration for export hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Directory where transcript markdown files are written
    pub export_dir: PathBuf,
    /// Whether to auto-export on recording complete
    pub auto_export: bool,
    /// Whether to emit UDS notification
    pub notify_uds: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            export_dir: home.join("Documents").join("Meetily").join("transcripts"),
            auto_export: true,
            notify_uds: true,
        }
    }
}

/// Exported meeting data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMeeting {
    pub meeting_id: String,
    pub title: String,
    pub date: String,
    pub duration_seconds: Option<f64>,
    pub transcript_path: PathBuf,
    pub audio_path: Option<PathBuf>,
}

/// Export a meeting transcript as enriched markdown
pub async fn export_meeting_markdown<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
) -> Result<ExportedMeeting, String> {
    let app_state = app.state::<AppState>();
    let db = app_state.db_manager.pool();

    // Fetch meeting metadata
    let meeting: Option<crate::database::models::MeetingModel> =
        sqlx::query_as("SELECT * FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(&*db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

    let meeting = meeting.ok_or_else(|| format!("Meeting {} not found", meeting_id))?;

    // Fetch transcript chunks ordered by timestamp
    let chunks: Vec<crate::database::models::TranscriptChunk> =
        sqlx::query_as("SELECT * FROM transcript_chunks WHERE meeting_id = ? ORDER BY created_at ASC")
            .bind(meeting_id)
            .fetch_all(&*db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

    // Fetch transcripts (segments with timestamps)
    let segments: Vec<crate::database::models::Transcript> =
        sqlx::query_as("SELECT * FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time ASC, timestamp ASC")
            .bind(meeting_id)
            .fetch_all(&*db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

    // Build markdown
    let config = ExportConfig::default();
    std::fs::create_dir_all(&config.export_dir)
        .map_err(|e| format!("Cannot create export dir: {}", e))?;

    let date_str = meeting.created_at.0.format("%Y-%m-%d").to_string();
    let time_str = meeting.created_at.0.format("%H%M").to_string();
    let safe_title = meeting
        .title
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
        .chars()
        .take(60)
        .collect::<String>();

    let filename = format!("{}_{}-{}.md", date_str, time_str, safe_title);
    let filepath = config.export_dir.join(&filename);

    // Calculate duration from segments
    let duration = segments
        .last()
        .and_then(|s| s.audio_end_time)
        .unwrap_or(0.0);

    let duration_display = if duration > 0.0 {
        let mins = (duration / 60.0) as u64;
        let secs = (duration % 60.0) as u64;
        format!("{}:{:02}", mins, secs)
    } else {
        "unknown".to_string()
    };

    // Build the markdown content
    let mut md = String::new();

    // YAML frontmatter
    md.push_str("---\n");
    md.push_str(&format!("meeting_id: \"{}\"\n", meeting.id));
    md.push_str(&format!("title: \"{}\"\n", meeting.title));
    md.push_str(&format!("date: {}\n", meeting.created_at.0.to_rfc3339()));
    md.push_str(&format!("duration: \"{}\"\n", duration_display));
    md.push_str(&format!("segments: {}\n", segments.len()));
    md.push_str(&format!("exported_at: {}\n", Utc::now().to_rfc3339()));

    // Speaker info if diarization state is available
    if let Some(diar_state) = app.try_state::<Arc<tokio::sync::RwLock<crate::diarization::DiarizationState>>>() {
        let ds = diar_state.read().await;
        if !ds.speakers.is_empty() {
            md.push_str("speakers:\n");
            for speaker in &ds.speakers {
                md.push_str(&format!("  - id: \"{}\"\n    name: \"{}\"\n", speaker.id, speaker.label));
            }
        }
    }

    // Screen context summary if available
    if let Some(screen_state) = app.try_state::<Arc<tokio::sync::RwLock<crate::screen_context::ScreenContextState>>>() {
        let ss = screen_state.read().await;
        if !ss.snapshots.is_empty() {
            let apps: std::collections::HashSet<&str> = ss.snapshots.iter()
                .map(|s| s.active_app.as_str())
                .collect();
            md.push_str("context_apps:\n");
            for app_name in &apps {
                md.push_str(&format!("  - \"{}\"\n", app_name));
            }
        }
    }

    md.push_str("---\n\n");

    // Title
    md.push_str(&format!("# {}\n", meeting.title));
    md.push_str(&format!(
        "_{} • {}_\n\n",
        meeting.created_at.0.format("%B %d, %Y %H:%M"),
        duration_display
    ));

    // Transcript body
    md.push_str("## Transcript\n\n");

    if !segments.is_empty() {
        for segment in &segments {
            let time_prefix = if let Some(start) = segment.audio_start_time {
                let mins = (start / 60.0) as u64;
                let secs = (start % 60.0) as u64;
                format!("[{:02}:{:02}] ", mins, secs)
            } else {
                String::new()
            };
            md.push_str(&format!("**{}**{}\n\n", time_prefix, segment.transcript));
        }
    } else if !chunks.is_empty() {
        // Fallback to chunks if no segments
        for chunk in &chunks {
            md.push_str(&format!("{}\n\n", chunk.transcript_text));
        }
    }

    // Screen context section
    if let Some(screen_state) = app.try_state::<Arc<tokio::sync::RwLock<crate::screen_context::ScreenContextState>>>() {
        let ss = screen_state.read().await;
        if !ss.snapshots.is_empty() {
            md.push_str("## Context (Screen Activity)\n\n");
            for snapshot in &ss.snapshots {
                let mins = (snapshot.audio_time / 60.0) as u64;
                let secs = (snapshot.audio_time % 60.0) as u64;
                let url_str = snapshot.url.as_deref().unwrap_or("");
                md.push_str(&format!(
                    "- [{:02}:{:02}] **{}** — {}{}\n",
                    mins, secs,
                    snapshot.active_app,
                    snapshot.window_title,
                    if url_str.is_empty() { String::new() } else { format!(" ({})", url_str) }
                ));
            }
            md.push_str("\n");
        }
    }

    // Write file
    std::fs::write(&filepath, &md).map_err(|e| format!("Write error: {}", e))?;

    info!(
        "📝 Exported meeting '{}' to {}",
        meeting.title,
        filepath.display()
    );

    // Emit event for UDS listeners
    let exported = ExportedMeeting {
        meeting_id: meeting.id.clone(),
        title: meeting.title.clone(),
        date: date_str,
        duration_seconds: if duration > 0.0 { Some(duration) } else { None },
        transcript_path: filepath.clone(),
        audio_path: meeting.folder_path.map(PathBuf::from),
    };

    // Emit Tauri event for any internal listeners
    let _ = app.emit("meeting-exported", &exported);

    // Write to UDS notification file (for external watchers)
    let notify_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local")
        .join("share")
        .join("meetily");
    let _ = std::fs::create_dir_all(&notify_path);
    let notify_file = notify_path.join("last-export.json");
    if let Ok(json) = serde_json::to_string_pretty(&exported) {
        let _ = std::fs::write(&notify_file, json);
    }

    Ok(exported)
}
