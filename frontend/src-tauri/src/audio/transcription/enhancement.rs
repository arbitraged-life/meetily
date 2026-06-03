// audio/transcription/enhancement.rs
//
// Parallel Enhancement Pipeline: runs a secondary (typically cloud) transcription provider
// on the same audio chunks that the primary local engine already processed.
// The primary engine provides immediate low-latency results, while the enhancement pass
// asynchronously delivers higher-quality replacements keyed by sequence_id.
//
// Architecture:
//   [AudioChunk] → Primary Engine → emit "transcript-update" (immediate)
//                ↘ Enhancement Queue → Cloud Provider → emit "transcript-enhancement" (delayed)
//
// The frontend merges enhancements into the transcript by matching sequence_id.

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

// ============================================================================
// TYPES
// ============================================================================

/// Configuration for the enhancement pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementConfig {
    /// Whether enhancement is enabled
    pub enabled: bool,
    /// Provider to use for enhancement (e.g., "deepgram", "openai", "assemblyai", "groq")
    pub provider: String,
    /// Model to use (provider-specific, e.g., "whisper-large-v3" for Groq)
    pub model: String,
    /// API key for the enhancement provider
    pub api_key: Option<String>,
    /// Maximum concurrent enhancement requests
    pub max_concurrent: usize,
    /// Whether to skip enhancement for chunks shorter than this (seconds)
    pub min_chunk_duration: f64,
    /// Language hint for the enhancement provider
    pub language: Option<String>,
}

impl Default for EnhancementConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "groq".to_string(),
            model: "whisper-large-v3-turbo".to_string(),
            api_key: None,
            max_concurrent: 3,
            min_chunk_duration: 0.5,
            language: None,
        }
    }
}

/// An enhancement request queued for processing
#[derive(Debug, Clone)]
pub struct EnhancementRequest {
    /// The sequence_id of the primary transcript update this enhances
    pub sequence_id: u64,
    /// Audio data (16kHz mono f32)
    pub audio_data: Vec<f32>,
    /// Sample rate of the audio
    pub sample_rate: u32,
    /// Original primary transcript text (for comparison/logging)
    pub primary_text: String,
    /// Recording-relative start time
    pub audio_start_time: f64,
    /// Recording-relative end time
    pub audio_end_time: f64,
    /// Chunk duration in seconds
    pub duration: f64,
}

/// Event payload emitted when an enhanced transcript is ready
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEnhancement {
    /// Matches the sequence_id of the original transcript-update
    pub sequence_id: u64,
    /// The enhanced (higher-quality) text
    pub text: String,
    /// Confidence from the enhancement provider (if available)
    pub confidence: Option<f32>,
    /// Which provider produced this enhancement
    pub provider: String,
    /// Processing latency in milliseconds
    pub latency_ms: u64,
    /// Original primary text for comparison
    pub primary_text: String,
    /// Whether the enhanced text is meaningfully different from primary
    pub is_different: bool,
}

// ============================================================================
// ENHANCEMENT STATE (managed via Tauri app state)
// ============================================================================

pub struct EnhancementState {
    pub config: EnhancementConfig,
    pub sender: Option<mpsc::UnboundedSender<EnhancementRequest>>,
    pub is_running: AtomicBool,
    pub chunks_enhanced: AtomicU64,
    pub chunks_skipped: AtomicU64,
}

impl EnhancementState {
    pub fn new() -> Self {
        Self {
            config: EnhancementConfig::default(),
            sender: None,
            is_running: AtomicBool::new(false),
            chunks_enhanced: AtomicU64::new(0),
            chunks_skipped: AtomicU64::new(0),
        }
    }
}

/// Type alias for the managed state
pub type EnhancementStateHandle = Arc<RwLock<EnhancementState>>;

pub fn new_enhancement_state() -> EnhancementStateHandle {
    Arc::new(RwLock::new(EnhancementState::new()))
}

// ============================================================================
// ENHANCEMENT PIPELINE
// ============================================================================

/// Start the enhancement worker. Called when recording starts (if enhancement is enabled).
/// Returns an UnboundedSender to queue enhancement requests.
pub fn start_enhancement_pipeline<R: Runtime>(
    app: AppHandle<R>,
    config: EnhancementConfig,
) -> mpsc::UnboundedSender<EnhancementRequest> {
    let (sender, receiver) = mpsc::unbounded_channel::<EnhancementRequest>();

    let max_concurrent = config.max_concurrent;
    let _provider_name = config.provider.clone();

    tokio::spawn(async move {
        info!(
            "🔄 Enhancement pipeline started (provider: {}, model: {}, max_concurrent: {})",
            config.provider, config.model, config.max_concurrent
        );

        // Use a semaphore to limit concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        let mut receiver = receiver;

        while let Some(request) = receiver.recv().await {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    warn!("Enhancement semaphore closed, stopping pipeline");
                    break;
                }
            };

            let app_clone = app.clone();
            let config_clone = config.clone();

            tokio::spawn(async move {
                let start = std::time::Instant::now();

                match enhance_chunk(&config_clone, &request).await {
                    Ok(enhanced_text) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        let is_different = normalized_different(&request.primary_text, &enhanced_text);

                        let enhancement = TranscriptEnhancement {
                            sequence_id: request.sequence_id,
                            text: enhanced_text.clone(),
                            confidence: None, // Could be populated by some providers
                            provider: config_clone.provider.clone(),
                            latency_ms,
                            primary_text: request.primary_text.clone(),
                            is_different,
                        };

                        // Only emit if meaningfully different
                        if is_different {
                            info!(
                                "✨ Enhancement ready for seq {} ({}ms): '{}' → '{}'",
                                request.sequence_id, latency_ms,
                                truncate_str(&request.primary_text, 40),
                                truncate_str(&enhanced_text, 40)
                            );

                            if let Err(e) = app_clone.emit("transcript-enhancement", &enhancement) {
                                error!("Failed to emit transcript-enhancement: {}", e);
                            }
                        } else {
                            info!(
                                "📎 Enhancement matches primary for seq {} ({}ms), skipping emit",
                                request.sequence_id, latency_ms
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Enhancement failed for seq {}: {}",
                            request.sequence_id, e
                        );
                        // Don't emit errors to frontend — the primary result is still valid
                    }
                }

                drop(permit);
            });
        }

        info!("🔄 Enhancement pipeline stopped");
    });

    sender
}

/// Perform enhancement transcription for a single chunk
async fn enhance_chunk(
    config: &EnhancementConfig,
    request: &EnhancementRequest,
) -> Result<String, String> {
    match config.provider.as_str() {
        "groq" => enhance_with_groq(config, request).await,
        "openai" => enhance_with_openai(config, request).await,
        "deepgram" => enhance_with_deepgram(config, request).await,
        other => Err(format!("Unsupported enhancement provider: {}", other)),
    }
}

/// Groq Whisper API enhancement
async fn enhance_with_groq(
    config: &EnhancementConfig,
    request: &EnhancementRequest,
) -> Result<String, String> {
    let api_key = config.api_key.as_ref().ok_or("Groq API key not configured")?;

    // Encode audio to WAV bytes for the API
    let wav_bytes = encode_wav_from_f32(&request.audio_data, request.sample_rate);

    let client = reqwest::Client::new();
    let form = reqwest::multipart::Form::new()
        .text("model", config.model.clone())
        .text("response_format", "json")
        .text("temperature", "0")
        .part(
            "file",
            reqwest::multipart::Part::bytes(wav_bytes)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?,
        );

    // Add language hint if configured
    let form = if let Some(ref lang) = config.language {
        form.text("language", lang.clone())
    } else {
        form
    };

    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Groq request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Groq API error {}: {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    json["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No text in Groq response".to_string())
}

/// OpenAI Whisper API enhancement
async fn enhance_with_openai(
    config: &EnhancementConfig,
    request: &EnhancementRequest,
) -> Result<String, String> {
    let api_key = config.api_key.as_ref().ok_or("OpenAI API key not configured")?;

    let wav_bytes = encode_wav_from_f32(&request.audio_data, request.sample_rate);

    let client = reqwest::Client::new();
    let form = reqwest::multipart::Form::new()
        .text("model", config.model.clone())
        .text("response_format", "json")
        .text("temperature", "0")
        .part(
            "file",
            reqwest::multipart::Part::bytes(wav_bytes)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?,
        );

    let form = if let Some(ref lang) = config.language {
        form.text("language", lang.clone())
    } else {
        form
    };

    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {}: {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    json["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No text in OpenAI response".to_string())
}

/// Deepgram enhancement
async fn enhance_with_deepgram(
    config: &EnhancementConfig,
    request: &EnhancementRequest,
) -> Result<String, String> {
    let api_key = config.api_key.as_ref().ok_or("Deepgram API key not configured")?;

    let wav_bytes = encode_wav_from_f32(&request.audio_data, request.sample_rate);

    let mut url = format!(
        "https://api.deepgram.com/v1/listen?model={}&punctuate=true&smart_format=true",
        config.model
    );
    if let Some(ref lang) = config.language {
        url.push_str(&format!("&language={}", lang));
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "audio/wav")
        .body(wav_bytes)
        .send()
        .await
        .map_err(|e| format!("Deepgram request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Deepgram API error {}: {}", status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Deepgram response: {}", e))?;

    // Deepgram response: results.channels[0].alternatives[0].transcript
    json["results"]["channels"][0]["alternatives"][0]["transcript"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No transcript in Deepgram response".to_string())
}

// ============================================================================
// UTILITIES
// ============================================================================

/// Encode f32 audio samples as a WAV byte buffer (16-bit PCM)
fn encode_wav_from_f32(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = samples.len();
    let bytes_per_sample: u16 = 2; // 16-bit
    let num_channels: u16 = 1;
    let byte_rate = sample_rate * bytes_per_sample as u32 * num_channels as u32;
    let block_align = num_channels * bytes_per_sample;
    let data_size = num_samples as u32 * bytes_per_sample as u32;
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&(bytes_per_sample * 8).to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    // Convert f32 [-1.0, 1.0] to i16
    for &sample in samples {
        let clamped = sample.max(-1.0).min(1.0);
        let i16_val = (clamped * 32767.0) as i16;
        buf.extend_from_slice(&i16_val.to_le_bytes());
    }

    buf
}

/// Check if two transcript texts are meaningfully different (ignoring case/punctuation differences)
fn normalized_different(a: &str, b: &str) -> bool {
    let norm_a = normalize_for_comparison(a);
    let norm_b = normalize_for_comparison(b);
    norm_a != norm_b
}

fn normalize_for_comparison(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Get enhancement configuration
#[tauri::command]
pub async fn get_enhancement_config<R: Runtime>(
    app: AppHandle<R>,
) -> Result<EnhancementConfig, String> {
    if let Some(state) = app.try_state::<EnhancementStateHandle>() {
        let s = state.read().await;
        Ok(s.config.clone())
    } else {
        Ok(EnhancementConfig::default())
    }
}

/// Update enhancement configuration
#[tauri::command]
pub async fn set_enhancement_config<R: Runtime>(
    app: AppHandle<R>,
    config: EnhancementConfig,
) -> Result<(), String> {
    if let Some(state) = app.try_state::<EnhancementStateHandle>() {
        let mut s = state.write().await;
        s.config = config;
        Ok(())
    } else {
        Err("Enhancement state not initialized".to_string())
    }
}

/// Get enhancement statistics
#[tauri::command]
pub async fn get_enhancement_stats<R: Runtime>(
    app: AppHandle<R>,
) -> Result<serde_json::Value, String> {
    if let Some(state) = app.try_state::<EnhancementStateHandle>() {
        let s = state.read().await;
        Ok(serde_json::json!({
            "enabled": s.config.enabled,
            "is_running": s.is_running.load(Ordering::SeqCst),
            "chunks_enhanced": s.chunks_enhanced.load(Ordering::SeqCst),
            "chunks_skipped": s.chunks_skipped.load(Ordering::SeqCst),
            "provider": s.config.provider,
            "model": s.config.model,
        }))
    } else {
        Ok(serde_json::json!({"enabled": false}))
    }
}
