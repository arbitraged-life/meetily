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
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            url_import_enabled: false,
            auto_mute_enabled: false,
            transcript_tags_enabled: true, // low-cost, high-value — on by default
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
                let flags = FeatureFlags {
                    url_import_enabled: store
                        .get("url_import_enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    auto_mute_enabled: store
                        .get("auto_mute_enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    transcript_tags_enabled: store
                        .get("transcript_tags_enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
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
                if let Err(e) = store.save() {
                    warn!("Failed to save feature flags: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open feature flags store for save: {}", e);
            }
        }
    }

    /// Check if a specific feature is enabled (non-async for hot paths)
    pub async fn is_enabled(&self, feature: Feature) -> bool {
        let flags = self.flags.read().await;
        match feature {
            Feature::UrlImport => flags.url_import_enabled,
            Feature::AutoMute => flags.auto_mute_enabled,
            Feature::TranscriptTags => flags.transcript_tags_enabled,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Feature {
    UrlImport,
    AutoMute,
    TranscriptTags,
}
