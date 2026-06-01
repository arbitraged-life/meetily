// Tauri commands for speaker diarization

use super::{DiarizationConfig, DiarizationState, Speaker};
use super::speaker_db;
use super::clustering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::RwLock;

/// Tauri command: get current speakers for this meeting
#[tauri::command]
pub async fn get_speakers<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<Speaker>, String> {
    let state = app.state::<Arc<RwLock<DiarizationState>>>();
    let s = state.read().await;
    Ok(s.speakers.clone())
}

/// Tauri command: get all known speakers (persistent database)
#[tauri::command]
pub async fn get_known_speakers<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<Vec<Speaker>, String> {
    Ok(speaker_db::load_speakers())
}

/// Tauri command: rename a speaker
#[tauri::command]
pub async fn rename_speaker<R: Runtime>(
    app: AppHandle<R>,
    speaker_id: String,
    new_name: String,
) -> Result<bool, String> {
    let state = app.state::<Arc<RwLock<DiarizationState>>>();
    let mut s = state.write().await;

    // Rename in current meeting
    let renamed = if let Some(speaker) = s.speakers.iter_mut().find(|sp| sp.id == speaker_id) {
        speaker.label = new_name.clone();
        true
    } else {
        false
    };

    // Also rename in persistent db
    let mut known = speaker_db::load_speakers();
    speaker_db::rename_speaker(&mut known, &speaker_id, &new_name);
    let _ = speaker_db::save_speakers(&known);

    Ok(renamed)
}

/// Tauri command: merge two speakers
#[tauri::command]
pub async fn merge_speakers<R: Runtime>(
    app: AppHandle<R>,
    keep_id: String,
    merge_id: String,
) -> Result<bool, String> {
    let state = app.state::<Arc<RwLock<DiarizationState>>>();
    let mut s = state.write().await;
    let result = clustering::merge_speakers(&mut s.speakers, &keep_id, &merge_id);
    Ok(result)
}

/// Tauri command: check if diarization model is available
#[tauri::command]
pub async fn is_diarization_available<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<bool, String> {
    Ok(super::is_model_available())
}

/// Tauri command: update diarization config
#[tauri::command]
pub async fn set_diarization_config<R: Runtime>(
    app: AppHandle<R>,
    config: DiarizationConfig,
) -> Result<(), String> {
    let state = app.state::<Arc<RwLock<DiarizationState>>>();
    let mut s = state.write().await;
    s.config = config;
    Ok(())
}
