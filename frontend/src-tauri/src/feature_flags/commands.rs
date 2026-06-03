//! Tauri commands for feature flag management

use super::{FeatureFlagState, FeatureFlags};
use tauri::{AppHandle, Runtime, State};

#[tauri::command]
pub async fn get_feature_flags(
    state: State<'_, FeatureFlagState>,
) -> Result<FeatureFlags, String> {
    Ok(state.flags.read().await.clone())
}

#[tauri::command]
pub async fn set_feature_flag<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, FeatureFlagState>,
    feature: String,
    enabled: bool,
) -> Result<FeatureFlags, String> {
    {
        let mut flags = state.flags.write().await;
        match feature.as_str() {
            "url_import_enabled" | "urlImportEnabled" => flags.url_import_enabled = enabled,
            "auto_mute_enabled" | "autoMuteEnabled" => flags.auto_mute_enabled = enabled,
            "transcript_tags_enabled" | "transcriptTagsEnabled" => {
                flags.transcript_tags_enabled = enabled;
                super::transcript_tags::set_enabled(enabled);
            }
            "diarization_enabled" | "diarizationEnabled" => flags.diarization_enabled = enabled,
            "dictionary_enabled" | "dictionaryEnabled" => flags.dictionary_enabled = enabled,
            "screen_context_enabled" | "screenContextEnabled" => flags.screen_context_enabled = enabled,
            "calendar_enabled" | "calendarEnabled" => flags.calendar_enabled = enabled,
            "atoll_bridge_enabled" | "atollBridgeEnabled" => flags.atoll_bridge_enabled = enabled,
            "analytics_enabled" | "analyticsEnabled" => flags.analytics_enabled = enabled,
            "whisper_preload" | "whisperPreload" => flags.whisper_preload = enabled,
            "parakeet_preload" | "parakeetPreload" => flags.parakeet_preload = enabled,
            "builtin_ai_preload" | "builtinAiPreload" => flags.builtin_ai_preload = enabled,
            _ => return Err(format!("Unknown feature flag: {}", feature)),
        }
    }
    state.save(&app).await;
    Ok(state.flags.read().await.clone())
}
