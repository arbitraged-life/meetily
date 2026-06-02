//! System audio mute/unmute during recording.
//! Only active when `auto_mute_enabled` feature flag is true.

use log::{debug, error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};

/// Tracks whether we muted the system (to avoid unmuting if user had it muted already)
static DID_MUTE: AtomicBool = AtomicBool::new(false);

/// Mute system audio output (macOS only for now).
/// No-op if already muted or on unsupported platforms.
pub fn mute_system_audio() {
    #[cfg(target_os = "macos")]
    {
        // Check if already muted — don't double-mute
        if was_already_muted() {
            debug!("System audio already muted, skipping");
            return;
        }

        match std::process::Command::new("osascript")
            .args(["-e", "set volume output muted true"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    DID_MUTE.store(true, Ordering::SeqCst);
                    info!("System audio muted for recording");
                } else {
                    warn!(
                        "Failed to mute system audio: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(e) => error!("Failed to execute osascript for mute: {}", e),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        debug!("Auto-mute not implemented for this platform");
    }
}

/// Unmute system audio output. Only unmutes if we were the ones who muted it.
pub fn unmute_system_audio() {
    if !DID_MUTE.load(Ordering::SeqCst) {
        return; // We didn't mute, don't unmute
    }

    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("osascript")
            .args(["-e", "set volume output muted false"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    DID_MUTE.store(false, Ordering::SeqCst);
                    info!("System audio unmuted after recording");
                } else {
                    warn!(
                        "Failed to unmute system audio: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(e) => error!("Failed to execute osascript for unmute: {}", e),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        DID_MUTE.store(false, Ordering::SeqCst);
    }
}

/// Check if system audio is currently muted (macOS)
#[cfg(target_os = "macos")]
fn was_already_muted() -> bool {
    match std::process::Command::new("osascript")
        .args(["-e", "output muted of (get volume settings)"])
        .output()
    {
        Ok(output) => {
            let result = String::from_utf8_lossy(&output.stdout);
            result.trim() == "true"
        }
        Err(_) => false, // Assume not muted on error
    }
}

/// Safety: call on app startup to clean up if previous session crashed while muted
pub fn ensure_unmuted_on_startup() {
    if DID_MUTE.load(Ordering::SeqCst) {
        unmute_system_audio();
    }
    // Also check a persistent flag file for crash recovery
    #[cfg(target_os = "macos")]
    {
        let flag_path = dirs::data_local_dir()
            .unwrap_or_default()
            .join("com.meetily.app")
            .join(".muted_flag");
        if flag_path.exists() {
            warn!("Found stale mute flag — previous session may have crashed while muted. Unmuting.");
            let _ = std::process::Command::new("osascript")
                .args(["-e", "set volume output muted false"])
                .output();
            let _ = std::fs::remove_file(&flag_path);
        }
    }
}

/// Write mute flag file (call when muting)
pub fn write_mute_flag() {
    #[cfg(target_os = "macos")]
    {
        let flag_dir = dirs::data_local_dir()
            .unwrap_or_default()
            .join("com.meetily.app");
        let _ = std::fs::create_dir_all(&flag_dir);
        let _ = std::fs::write(flag_dir.join(".muted_flag"), "1");
    }
}

/// Remove mute flag file (call when unmuting)
pub fn clear_mute_flag() {
    #[cfg(target_os = "macos")]
    {
        let flag_path = dirs::data_local_dir()
            .unwrap_or_default()
            .join("com.meetily.app")
            .join(".muted_flag");
        let _ = std::fs::remove_file(flag_path);
    }
}
