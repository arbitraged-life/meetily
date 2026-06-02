// Dictionary sync — shared vocabulary across Meetily, VoiceInk, Raycast

pub mod commands;

use log::{error, info};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A dictionary entry — word/phrase replacement or pronunciation hint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub id: String,
    /// The word/phrase as it should appear in transcripts
    pub display: String,
    /// Alternative spellings/pronunciations that should map to this
    pub aliases: Vec<String>,
    /// Source app that created this entry
    pub source: String,
    /// When this entry was last updated
    pub updated_at: String,
}

/// Dictionary state
pub type DictionaryState = Arc<RwLock<Vec<DictionaryEntry>>>;

pub fn new_dictionary_state() -> DictionaryState {
    Arc::new(RwLock::new(Vec::new()))
}

/// Get the shared dictionary file path
pub fn dictionary_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("unified-dictionary")
        .join("dictionary.json")
}

/// Load dictionary from disk
pub async fn load_dictionary(state: &DictionaryState) -> Result<(), String> {
    let path = dictionary_path();
    if !path.exists() {
        // Create default empty dictionary
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create dictionary dir: {}", e))?;
        std::fs::write(&path, "[]").map_err(|e| format!("Cannot write dictionary: {}", e))?;
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read dictionary: {}", e))?;
    let entries: Vec<DictionaryEntry> =
        serde_json::from_str(&content).map_err(|e| format!("Invalid dictionary JSON: {}", e))?;

    let mut guard = state.write().await;
    *guard = entries;
    info!("📖 Loaded {} dictionary entries", guard.len());
    Ok(())
}

/// Save dictionary to disk (atomic write)
pub async fn save_dictionary(state: &DictionaryState) -> Result<(), String> {
    let path = dictionary_path();
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create dictionary dir: {}", e))?;

    let guard = state.read().await;
    let json = serde_json::to_string_pretty(&*guard)
        .map_err(|e| format!("Cannot serialize dictionary: {}", e))?;

    // Atomic write: write to temp, then rename
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json).map_err(|e| format!("Cannot write temp: {}", e))?;
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("Cannot rename: {}", e))?;

    info!("📖 Saved {} dictionary entries", guard.len());
    Ok(())
}

/// Apply dictionary to transcript text — replace aliases with display forms
pub fn apply_dictionary(text: &str, entries: &[DictionaryEntry]) -> String {
    let mut result = text.to_string();
    for entry in entries {
        for alias in &entry.aliases {
            // Case-insensitive whole-word replacement
            let pattern = format!(r"(?i)\b{}\b", regex::escape(alias));
            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re.replace_all(&result, entry.display.as_str()).to_string();
            }
        }
    }
    result
}

/// Start watching the dictionary file for external changes (from VoiceInk, Raycast, etc.)
pub fn start_dictionary_watcher(
    state: DictionaryState,
) -> Result<RecommendedWatcher, String> {
    let path = dictionary_path();
    let dir = path.parent().unwrap().to_path_buf();

    let state_clone = state.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() {
                // Reload dictionary on file change
                let rt = tokio::runtime::Handle::current();
                let state_inner = state_clone.clone();
                rt.spawn(async move {
                    if let Err(e) = load_dictionary(&state_inner).await {
                        error!("📖 Failed to reload dictionary: {}", e);
                    } else {
                        info!("📖 Dictionary reloaded from external change");
                    }
                });
            }
        }
    })
    .map_err(|e| format!("Cannot create file watcher: {}", e))?;

    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Cannot watch dictionary dir: {}", e))?;

    info!("📖 Watching dictionary at {}", dir.display());
    Ok(watcher)
}
