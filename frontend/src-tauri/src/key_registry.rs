//! Central API-key registry integration (NERV).
//!
//! Single source of truth for provider API keys lives in the macOS Keychain
//! under the neutral service `com.nerv.keyreg`, one generic-password item per
//! canonical provider id (account = provider). This module reads from it so
//! Meetily auto-detects keys the user already configured elsewhere (VoiceInk,
//! MacWhisper, env vars — all seeded into the registry by the `nervkeys` CLI)
//! instead of forcing the user to retype them.
//!
//! Design notes:
//! - Read-only from Meetily's side by default (the CLI owns writes), but a
//!   `registry_set_key` command is provided so the app can push a key the user
//!   types in Meetily back into the shared registry — keeping one source of truth.
//! - No new crate dependency: shells out to the `security` binary that ships
//!   with macOS. On non-macOS this module degrades to "nothing detected".
//! - Never logs key values.

use serde::Serialize;

/// Neutral keychain service shared by all NERV apps.
const REGISTRY_SERVICE: &str = "com.nerv.keyreg";

/// All canonical providers the registry knows about.
pub const KNOWN_PROVIDERS: &[&str] = &[
    "groq", "openai", "anthropic", "openrouter", "cerebras", "deepgram",
    "elevenlabs", "gemini", "mistral", "xai", "assemblyai", "soniox",
    "speechmatics", "cartesia",
];

/// Map a Meetily provider id to the registry's canonical provider id.
/// Meetily calls Anthropic "claude" and uses camelCase ids like "elevenLabs"
/// in its transcript settings; the registry uses lowercase canonical ids.
fn canonical(provider: &str) -> &str {
    match provider {
        "claude" => "anthropic",
        "elevenLabs" => "elevenlabs",
        "assemblyAI" => "assemblyai",
        "localWhisper" => "localwhisper",
        other => other,
    }
}

/// Read a key from the central registry. Returns `None` if absent or on
/// non-macOS platforms. Never logs the value.
#[cfg(target_os = "macos")]
pub fn get_key(provider: &str) -> Option<String> {
    use std::process::Command;
    let account = canonical(provider);
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            REGISTRY_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_key(_provider: &str) -> Option<String> {
    None
}

/// Write/update a key in the central registry so all NERV apps see it.
#[cfg(target_os = "macos")]
pub fn set_key(provider: &str, value: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};
    use std::io::Write;
    if value.is_empty() {
        return Err("empty value".into());
    }
    let account = canonical(provider);
    let label = format!("NERV key: {}", account);
    
    let mut child = Command::new("security")
        .args([
            "add-generic-password",
            "-U", // update if exists
            "-s",
            REGISTRY_SERVICE,
            "-a",
            account,
            "-l",
            &label,
            "-w",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
        
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(value.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("security exited with {:?}", status.code()))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_key(_provider: &str, _value: &str) -> Result<(), String> {
    Err("central registry only supported on macOS".into())
}

/// True if the registry holds a key for this provider.
pub fn has_key(provider: &str) -> bool {
    get_key(provider).is_some()
}

#[derive(Serialize)]
pub struct RegistryStatus {
    /// Canonical provider ids that have a key available in the registry.
    pub providers: Vec<String>,
}

/// Resolve a key with the registry as a FALLBACK: prefer the explicitly
/// provided key (e.g. from Meetily's own DB), else pull from the central
/// registry. Lets existing call sites stay unchanged while gaining
/// auto-detection. Returns None if neither source has a usable key.
pub fn resolve(provider: &str, existing: Option<String>) -> Option<String> {
    match existing {
        Some(k) if !k.trim().is_empty() => Some(k),
        _ => get_key(provider),
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Returns which providers have a key in the central registry — drives
/// zero-typing UI (show "✓ detected" badges, prefill dropdowns).
#[tauri::command]
pub fn registry_status() -> RegistryStatus {
    let providers = KNOWN_PROVIDERS
        .iter()
        .filter(|&&p| has_key(p))
        .map(|&p| p.to_string())
        .collect();
    RegistryStatus { providers }
}

/// True/false whether a single provider has a key available centrally.
#[tauri::command]
pub fn registry_has_key(provider: String) -> bool {
    has_key(&provider)
}

/// Fetch a provider's key from the central registry (used to prefill or to
/// satisfy a request without the user typing). Returns empty string if absent.
#[tauri::command]
pub fn registry_get_key(provider: String) -> String {
    get_key(&provider).unwrap_or_default()
}

/// Push a key the user entered in Meetily into the shared registry so every
/// NERV app benefits. No-op error if value empty.
#[tauri::command]
pub fn registry_set_key(provider: String, value: String) -> Result<(), String> {
    set_key(&provider, &value)
}
