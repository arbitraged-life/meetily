//! Global hotkey for start/stop recording (issue #869).
//!
//! Registers a system-wide shortcut (default: Cmd+Shift+R on macOS) that
//! toggles recording even when Meetily is in the background. The hotkey reuses
//! the exact same start/stop path as the tray menu (`tray::toggle_recording_handler`)
//! so behaviour stays identical across tray, UI button, and hotkey.
//!
//! The accelerator is configurable and persisted in a tiny JSON file under the
//! app config dir, so the user can rebind it without recompiling. A Tauri
//! command (`set_recording_hotkey`) lets the settings UI change it live.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Default accelerator if the user hasn't bound their own.
pub const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+R";

/// Debounce window (ms) so auto-repeat / press+release don't double-toggle.
const DEBOUNCE_MS: u64 = 600;

/// Last time (ms since epoch) the hotkey fired — guards against key-repeat.
static LAST_FIRE_MS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Accelerator string in Tauri format, e.g. "CmdOrCtrl+Shift+R".
    pub accelerator: String,
    /// Whether the global hotkey is active.
    pub enabled: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            accelerator: DEFAULT_HOTKEY.to_string(),
            enabled: true,
        }
    }
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("recording_hotkey.json"))
}

pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> HotkeyConfig {
    if let Some(p) = config_path(app) {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(cfg) = serde_json::from_slice::<HotkeyConfig>(&bytes) {
                return cfg;
            }
        }
    }
    HotkeyConfig::default()
}

fn save_config<R: Runtime>(app: &AppHandle<R>, cfg: &HotkeyConfig) -> Result<(), String> {
    let p = config_path(app).ok_or("could not resolve app config dir")?;
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&p, bytes).map_err(|e| e.to_string())
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn now_ms() -> u64 {
    let start = START_TIME.get_or_init(std::time::Instant::now);
    std::time::Instant::now().duration_since(*start).as_millis() as u64
}

/// True if enough time has passed since the last fire (debounce). Updates the
/// stored timestamp when it returns true.
fn should_fire() -> bool {
    let now = now_ms();
    let last = LAST_FIRE_MS.load(Ordering::SeqCst);
    if now.saturating_sub(last) >= DEBOUNCE_MS {
        LAST_FIRE_MS.store(now, Ordering::SeqCst);
        true
    } else {
        false
    }
}

/// Build the global-shortcut plugin with our toggle handler. Register this on
/// the Tauri builder. The actual accelerator is registered later in `init()`
/// during setup (so we can read the persisted config from the app handle).
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            // Only react on key-down, and debounce against auto-repeat.
            if event.state() == ShortcutState::Pressed && should_fire() {
                log::info!("Global hotkey pressed → toggling recording");
                crate::tray::toggle_recording_handler(app);
            }
        })
        .build()
}

/// Register the persisted (or default) accelerator. Call from `.setup()`.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let cfg = load_config(app);
    if !cfg.enabled {
        log::info!("Recording hotkey disabled by config");
        return;
    }
    match register(app, &cfg.accelerator) {
        Ok(_) => log::info!("✅ Recording hotkey registered: {}", cfg.accelerator),
        Err(e) => log::error!("Failed to register recording hotkey '{}': {}", cfg.accelerator, e),
    }
}

/// Register a single accelerator, replacing any previously-registered one.
fn register<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> Result<(), String> {
    let shortcut: Shortcut = accelerator.parse().map_err(|e| format!("invalid accelerator: {:?}", e))?;
    let gs = app.global_shortcut();
    // Clear everything we own so re-binding doesn't leak stale shortcuts.
    let _ = gs.unregister_all();
    gs.register(shortcut).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tauri commands (drive the settings UI)
// ---------------------------------------------------------------------------

/// Return the current hotkey config so the settings UI can display it.
#[tauri::command]
pub fn get_recording_hotkey<R: Runtime>(app: AppHandle<R>) -> HotkeyConfig {
    load_config(&app)
}

/// Rebind the recording hotkey live and persist it. Pass enabled=false to turn
/// it off entirely. Returns the saved config on success.
#[tauri::command]
pub fn set_recording_hotkey<R: Runtime>(
    app: AppHandle<R>,
    accelerator: String,
    enabled: bool,
) -> Result<HotkeyConfig, String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    if enabled {
        register(&app, &accelerator)?;
    }
    let cfg = HotkeyConfig { accelerator, enabled };
    save_config(&app, &cfg)?;
    Ok(cfg)
}
