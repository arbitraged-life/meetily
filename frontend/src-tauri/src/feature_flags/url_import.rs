//! URL audio import — download audio from YouTube or direct URLs for transcription.
//! Only active when `url_import_enabled` feature flag is true.

use anyhow::{anyhow, Result};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// URL import result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlImportResult {
    pub file_path: PathBuf,
    pub title: String,
    pub duration_secs: Option<f64>,
}

/// Detect if a URL is a YouTube link
pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/watch")
        || url.contains("youtu.be/")
        || url.contains("youtube.com/shorts/")
        || url.contains("youtube.com/live/")
}

/// Detect if a URL is a direct audio link
pub fn is_direct_audio_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.ends_with(".mp3")
        || lower.ends_with(".wav")
        || lower.ends_with(".m4a")
        || lower.ends_with(".ogg")
        || lower.ends_with(".flac")
        || lower.ends_with(".opus")
        || lower.ends_with(".webm")
        || lower.ends_with(".mp4")
}

/// Check if yt-dlp is available on the system
pub fn is_ytdlp_available() -> bool {
    Command::new("yt-dlp")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Download audio from a YouTube URL using yt-dlp
pub async fn download_youtube<R: Runtime>(
    app: &AppHandle<R>,
    url: &str,
    output_dir: &Path,
) -> Result<UrlImportResult> {
    if !is_ytdlp_available() {
        return Err(anyhow!(
            "yt-dlp not found. Install with: brew install yt-dlp"
        ));
    }

    info!("Downloading YouTube audio: {}", url);
    let _ = app.emit("url-import-progress", serde_json::json!({
        "status": "downloading",
        "url": url
    }));

    // Get video title first
    let title_output = tokio::process::Command::new("yt-dlp")
        .args(["--get-title", "--no-warnings", url])
        .output()
        .await
        .map_err(|e| anyhow!("Failed to get video title: {}", e))?;

    let title = String::from_utf8_lossy(&title_output.stdout)
        .trim()
        .to_string();
    let title = if title.is_empty() {
        "Untitled Video".to_string()
    } else {
        title
    };

    // Sanitize title for filename
    let safe_title: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let output_template = output_dir.join(format!("{}.%(ext)s", safe_title));

    // Download audio only, convert to wav for transcription
    let output = tokio::process::Command::new("yt-dlp")
        .args([
            "-x",                          // extract audio
            "--audio-format", "wav",       // convert to wav
            "--audio-quality", "0",        // best quality
            "--no-warnings",
            "--no-playlist",               // single video only
            "-o", output_template.to_str().unwrap_or("%(title)s.%(ext)s"),
            url,
        ])
        .output()
        .await
        .map_err(|e| anyhow!("yt-dlp failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("yt-dlp download failed: {}", stderr));
    }

    let wav_path = output_dir.join(format!("{}.wav", safe_title));
    if !wav_path.exists() {
        // Try finding any file that was created
        let entries = std::fs::read_dir(output_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_stem().map(|s| s.to_string_lossy().contains(&safe_title)).unwrap_or(false) {
                return Ok(UrlImportResult {
                    file_path: path,
                    title,
                    duration_secs: None,
                });
            }
        }
        return Err(anyhow!("Downloaded file not found at expected path"));
    }

    let _ = app.emit("url-import-progress", serde_json::json!({
        "status": "complete",
        "url": url,
        "title": &title
    }));

    Ok(UrlImportResult {
        file_path: wav_path,
        title,
        duration_secs: None,
    })
}

/// Download a direct audio URL via HTTPS
pub async fn download_direct_url<R: Runtime>(
    app: &AppHandle<R>,
    url: &str,
    output_dir: &Path,
) -> Result<UrlImportResult> {
    info!("Downloading direct audio URL: {}", url);

    // SSRF protection: only allow HTTPS
    if !url.starts_with("https://") {
        return Err(anyhow!("Only HTTPS URLs are supported for security"));
    }

    // Parse URL to get filename
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;
    
    // SSRF: reject private IPs (basic check — full impl would resolve DNS first)
    if let Some(host) = parsed.host_str() {
        if host == "localhost"
            || host == "127.0.0.1"
            || host.starts_with("192.168.")
            || host.starts_with("10.")
            || host.starts_with("172.16.")
            || host == "0.0.0.0"
        {
            return Err(anyhow!("Private/local URLs are not allowed"));
        }
    }

    let filename = parsed
        .path_segments()
        .and_then(|s| s.last())
        .unwrap_or("audio")
        .to_string();

    let _ = app.emit("url-import-progress", serde_json::json!({
        "status": "downloading",
        "url": url
    }));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| anyhow!("HTTP client error: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Download failed: {}", e))?;

    // Check content length (500MB limit)
    if let Some(len) = response.content_length() {
        if len > 500 * 1024 * 1024 {
            return Err(anyhow!("File too large (>500MB)"));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow!("Failed to read response: {}", e))?;

    let output_path = output_dir.join(&filename);
    tokio::fs::write(&output_path, &bytes)
        .await
        .map_err(|e| anyhow!("Failed to write file: {}", e))?;

    let _ = app.emit("url-import-progress", serde_json::json!({
        "status": "complete",
        "url": url,
        "title": &filename
    }));

    Ok(UrlImportResult {
        file_path: output_path,
        title: filename,
        duration_secs: None,
    })
}

/// Tauri command: import audio from URL
#[tauri::command]
pub async fn import_audio_from_url<R: Runtime>(
    app: AppHandle<R>,
    url: String,
) -> Result<UrlImportResult, String> {
    // Check feature flag
    let state = app.state::<super::FeatureFlagState>();
    if !state.is_enabled(super::Feature::UrlImport).await {
        return Err("URL import feature is disabled. Enable it in Settings → Features.".into());
    }

    // Create temp download directory
    let download_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("meetily-url-imports");
    tokio::fs::create_dir_all(&download_dir)
        .await
        .map_err(|e| format!("Failed to create download dir: {}", e))?;

    if is_youtube_url(&url) {
        download_youtube(&app, &url, &download_dir)
            .await
            .map_err(|e| e.to_string())
    } else if is_direct_audio_url(&url) {
        download_direct_url(&app, &url, &download_dir)
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("Unsupported URL. Provide a YouTube link or direct audio file URL (.mp3, .wav, .m4a, etc.)".into())
    }
}
