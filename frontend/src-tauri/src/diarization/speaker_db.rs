// Persistent speaker database — remembers speakers across meetings

use super::Speaker;
use log::{error, info};
use std::path::PathBuf;

/// Path to the speaker database JSON file
fn speaker_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("com.meetily.ai")
        .join("speakers.json")
}

/// Load known speakers from disk
pub fn load_speakers() -> Vec<Speaker> {
    let path = speaker_db_path();
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            error!("Failed to load speaker db: {}", e);
            Vec::new()
        }
    }
}

/// Save known speakers to disk
pub fn save_speakers(speakers: &[Speaker]) -> Result<(), String> {
    let path = speaker_db_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(speakers)
        .map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;
    info!("💾 Saved {} speakers to database", speakers.len());
    Ok(())
}

/// Rename a speaker in the database
pub fn rename_speaker(speakers: &mut [Speaker], speaker_id: &str, new_name: &str) -> bool {
    if let Some(speaker) = speakers.iter_mut().find(|s| s.id == speaker_id) {
        speaker.label = new_name.to_string();
        true
    } else {
        false
    }
}
