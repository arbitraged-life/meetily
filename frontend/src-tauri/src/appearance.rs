//! Menu-bar-only / hide-from-Dock mode (issue #428).
//!
//! Lets the user run Meetily as a pure menu-bar (tray) app with no Dock icon
//! and no app-switcher entry — handy for an always-on recorder that shouldn't
//! clutter the Dock. On macOS this maps to `NSApplication.activationPolicy`:
//!   - `Regular`   → normal app (Dock icon + menu bar)
//!   - `Accessory` → no Dock icon, lives only in the menu bar (tray)
//!
//! The preference is persisted in a tiny JSON file in the app config dir and
//! applied at startup. A Tauri command (`set_dock_visibility`) flips it live so
//! the settings UI can toggle it without a restart.
//!
//! When hiding the Dock icon we make sure the tray exists (it always does in
//! Meetily) so the user still has a way to reach the app. We do NOT close the
//! main window — the user can still summon it from the tray menu.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// When true, hide the Dock icon and run menu-bar-only (Accessory policy).
    pub menu_bar_only: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            menu_bar_only: false,
        }
    }
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("appearance.json"))
}

pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> AppearanceConfig {
    if let Some(p) = config_path(app) {
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(cfg) = serde_json::from_slice::<AppearanceConfig>(&bytes) {
                return cfg;
            }
        }
    }
    AppearanceConfig::default()
}

fn save_config<R: Runtime>(app: &AppHandle<R>, cfg: &AppearanceConfig) -> Result<(), String> {
    let p = config_path(app).ok_or("could not resolve app config dir")?;
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&p, bytes).map_err(|e| e.to_string())
}

/// Apply the activation policy for the given preference. macOS-only effect;
/// on other platforms this is a no-op (Dock concept doesn't apply).
#[cfg(target_os = "macos")]
fn apply_policy<R: Runtime>(app: &AppHandle<R>, menu_bar_only: bool) -> Result<(), String> {
    use tauri::ActivationPolicy;
    let policy = if menu_bar_only {
        ActivationPolicy::Accessory
    } else {
        ActivationPolicy::Regular
    };
    app.set_activation_policy(policy)
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn apply_policy<R: Runtime>(_app: &AppHandle<R>, _menu_bar_only: bool) -> Result<(), String> {
    Ok(())
}

/// Apply the persisted preference at startup. Call from `.setup()`.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let cfg = load_config(app);
    if cfg.menu_bar_only {
        match apply_policy(app, true) {
            Ok(_) => log::info!("✅ Menu-bar-only mode active (Dock icon hidden)"),
            Err(e) => log::error!("Failed to apply menu-bar-only mode: {}", e),
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands (drive the settings UI)
// ---------------------------------------------------------------------------

/// Return the current appearance config so the settings UI can display it.
#[tauri::command]
pub fn get_dock_visibility<R: Runtime>(app: AppHandle<R>) -> AppearanceConfig {
    load_config(&app)
}

/// Toggle menu-bar-only mode live and persist it. `menu_bar_only=true` hides
/// the Dock icon; `false` restores the normal Dock + app-switcher presence.
/// When un-hiding, also bring the main window forward so the app is reachable.
#[tauri::command]
pub fn set_dock_visibility<R: Runtime>(
    app: AppHandle<R>,
    menu_bar_only: bool,
) -> Result<AppearanceConfig, String> {
    apply_policy(&app, menu_bar_only)?;

    // When returning to Regular mode, surface the main window so the user isn't
    // left with a hidden app and a now-visible Dock icon that does nothing.
    if !menu_bar_only {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }

    let cfg = AppearanceConfig { menu_bar_only };
    save_config(&app, &cfg)?;
    Ok(cfg)
}
