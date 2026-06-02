//! Feature flags module — runtime toggles for optional features.
//! Features are disabled by default and only initialize when enabled.
//! This ensures zero performance cost for unused features.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;
use tokio::sync::RwLock;

pub mod commands;
pub mod system_mute;
pub mod transcript_tags;
pub mod url_import;

/// All optional feature toggles
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    /// Enable URL audio import (YouTube, direct links)
    pub url_import_enabled: bool,
    /// Auto-mute system audio while recording
    pub auto_mute_enabled: bool,
    /// Wrap transcripts in <transcription> tags for LLM summary
    pub transcript_tags_enabled: bool,
    /// Speaker diarization (loads ONNX model ~50MB into memory)
    pub diarization_enabled: bool,
    /// Custom dictionary loading + file watcher
    pub dictionary_enabled: bool,
    /// Screen context OCR capture during recording
    pub screen_context_enabled: bool,
    /// Calendar ICS poller (background network sync)
    pub calendar_enabled: bool,
    /// Atoll notch bridge (WebSocket to localhost:9020)
    pub atoll_bridge_enabled: bool,
    /// Analytics telemetry (PostHog)
    pub analytics_enabled: bool,
    /// Eager Whisper engine initialization on startup (vs lazy on first use)
    pub whisper_preload: bool,
    /// Eager Parakeet engine initialization on startup (vs lazy on first use)
    pub parakeet_preload: bool,
    /// Eager ModelManager (built-in AI) initialization on startup (vs lazy on first use)
    pub builtin_ai_preload: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            url_import_enabled: false,
            auto_mute_enabled: false,
            transcript_tags_enabled: true, // low-cost, high-value — on by default
            diarization_enabled: false,    // heavy ONNX model — opt-in
            dictionary_enabled: false,     // file watcher + load — opt-in
            screen_context_enabled: false, // OCR overhead — opt-in
            calendar_enabled: false,       // network polling — opt-in
            atoll_bridge_enabled: false,   // WebSocket — opt-in
            analytics_enabled: false,      // telemetry — opt-in
            whisper_preload: false,        // lazy-init on first transcription
            parakeet_preload: false,       // lazy-init on first transcription
            builtin_ai_preload: false,     // lazy-init on first summary
        }
    }
}

/// Global feature flag state
pub struct FeatureFlagState {
    pub flags: Arc<RwLock<FeatureFlags>>,
}

impl FeatureFlagState {
    pub fn new() -> Self {
        Self {
            flags: Arc::new(RwLock::new(FeatureFlags::default())),
        }
    }

    /// Load flags from tauri-plugin-store
    pub async fn load<R: Runtime>(&self, app: &AppHandle<R>) {
        match app.store("feature_flags.json") {
            Ok(store) => {
                let def = FeatureFlags::default();
                let flags = FeatureFlags {
                    url_import_enabled: store.get("url_import_enabled").and_then(|v| v.as_bool()).unwrap_or(def.url_import_enabled),
                    auto_mute_enabled: store.get("auto_mute_enabled").and_then(|v| v.as_bool()).unwrap_or(def.auto_mute_enabled),
                    transcript_tags_enabled: store.get("transcript_tags_enabled").and_then(|v| v.as_bool()).unwrap_or(def.transcript_tags_enabled),
                    diarization_enabled: store.get("diarization_enabled").and_then(|v| v.as_bool()).unwrap_or(def.diarization_enabled),
                    dictionary_enabled: store.get("dictionary_enabled").and_then(|v| v.as_bool()).unwrap_or(def.dictionary_enabled),
                    screen_context_enabled: store.get("screen_context_enabled").and_then(|v| v.as_bool()).unwrap_or(def.screen_context_enabled),
                    calendar_enabled: store.get("calendar_enabled").and_then(|v| v.as_bool()).unwrap_or(def.calendar_enabled),
                    atoll_bridge_enabled: store.get("atoll_bridge_enabled").and_then(|v| v.as_bool()).unwrap_or(def.atoll_bridge_enabled),
                    analytics_enabled: store.get("analytics_enabled").and_then(|v| v.as_bool()).unwrap_or(def.analytics_enabled),
                    whisper_preload: store.get("whisper_preload").and_then(|v| v.as_bool()).unwrap_or(def.whisper_preload),
                    parakeet_preload: store.get("parakeet_preload").and_then(|v| v.as_bool()).unwrap_or(def.parakeet_preload),
                    builtin_ai_preload: store.get("builtin_ai_preload").and_then(|v| v.as_bool()).unwrap_or(def.builtin_ai_preload),
                };
                info!("Feature flags loaded: {:?}", flags);
                transcript_tags::set_enabled(flags.transcript_tags_enabled);
                *self.flags.write().await = flags;
            }
            Err(e) => {
                warn!("Failed to load feature flags store, using defaults: {}", e);
            }
        }
    }

    /// Save current flags to store
    pub async fn save<R: Runtime>(&self, app: &AppHandle<R>) {
        let flags = self.flags.read().await.clone();
        match app.store("feature_flags.json") {
            Ok(store) => {
                store.set("url_import_enabled", flags.url_import_enabled);
                store.set("auto_mute_enabled", flags.auto_mute_enabled);
                store.set("transcript_tags_enabled", flags.transcript_tags_enabled);
                store.set("diarization_enabled", flags.diarization_enabled);
                store.set("dictionary_enabled", flags.dictionary_enabled);
                store.set("screen_context_enabled", flags.screen_context_enabled);
                store.set("calendar_enabled", flags.calendar_enabled);
                store.set("atoll_bridge_enabled", flags.atoll_bridge_enabled);
                store.set("analytics_enabled", flags.analytics_enabled);
                store.set("whisper_preload", flags.whisper_preload);
                store.set("parakeet_preload", flags.parakeet_preload);
                store.set("builtin_ai_preload", flags.builtin_ai_preload);
                if let Err(e) = store.save() {
                    warn!("Failed to save feature flags: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open feature flags store for save: {}", e);
            }
        }
    }

    /// Check if a specific feature is enabled
    pub async fn is_enabled(&self, feature: Feature) -> bool {
        let flags = self.flags.read().await;
        match feature {
            Feature::UrlImport => flags.url_import_enabled,
            Feature::AutoMute => flags.auto_mute_enabled,
            Feature::TranscriptTags => flags.transcript_tags_enabled,
            Feature::Diarization => flags.diarization_enabled,
            Feature::Dictionary => flags.dictionary_enabled,
            Feature::ScreenContext => flags.screen_context_enabled,
            Feature::Calendar => flags.calendar_enabled,
            Feature::AtollBridge => flags.atoll_bridge_enabled,
            Feature::Analytics => flags.analytics_enabled,
            Feature::WhisperPreload => flags.whisper_preload,
            Feature::ParakeetPreload => flags.parakeet_preload,
            Feature::BuiltinAiPreload => flags.builtin_ai_preload,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Feature {
    UrlImport,
    AutoMute,
    TranscriptTags,
    Diarization,
    Dictionary,
    ScreenContext,
    Calendar,
    AtollBridge,
    Analytics,
    WhisperPreload,
    ParakeetPreload,
    BuiltinAiPreload,
}
