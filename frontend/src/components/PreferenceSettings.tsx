"use client"

import { useEffect, useState, useRef } from "react"
import { Switch } from "./ui/switch"
import { FolderOpen } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import Analytics from "@/lib/analytics"
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch"
import { useConfig, NotificationSettings } from "@/contexts/ConfigContext"

// Render a Tauri accelerator string as a readable shortcut for macOS.
function formatAccel(accel: string): string {
  return accel
    .split("+")
    .map((p) => {
      switch (p) {
        case "CmdOrCtrl":
        case "Cmd":
        case "Super":
        case "Meta":
          return "⌘";
        case "Ctrl":
        case "Control":
          return "⌃";
        case "Shift":
          return "⇧";
        case "Alt":
        case "Option":
          return "⌥";
        case "Space":
          return "Space";
        default:
          return p;
      }
    })
    .join(" ");
}

export function PreferenceSettings() {
  const {
    notificationSettings,
    storageLocations,
    isLoadingPreferences,
    loadPreferences,
    updateNotificationSettings
  } = useConfig();

  const [notificationsEnabled, setNotificationsEnabled] = useState<boolean | null>(null);
  const [isInitialLoad, setIsInitialLoad] = useState(true);
  const [previousNotificationsEnabled, setPreviousNotificationsEnabled] = useState<boolean | null>(null);
  const hasTrackedViewRef = useRef(false);

  // Recording hotkey (#869) + menu-bar-only mode (#428)
  const [hotkeyAccel, setHotkeyAccel] = useState<string>("CmdOrCtrl+Shift+R");
  const [hotkeyEnabled, setHotkeyEnabled] = useState<boolean>(true);
  const [capturingHotkey, setCapturingHotkey] = useState(false);
  const [hotkeyMsg, setHotkeyMsg] = useState<string | null>(null);
  const [menuBarOnly, setMenuBarOnly] = useState<boolean>(false);

  // Load hotkey + appearance configs from the backend on mount
  useEffect(() => {
    (async () => {
      try {
        const hk = await invoke<{ accelerator: string; enabled: boolean }>("get_recording_hotkey");
        setHotkeyAccel(hk.accelerator);
        setHotkeyEnabled(hk.enabled);
      } catch (e) {
        console.error("Failed to load recording hotkey:", e);
      }
      try {
        const ap = await invoke<{ menu_bar_only: boolean }>("get_dock_visibility");
        setMenuBarOnly(ap.menu_bar_only);
      } catch (e) {
        console.error("Failed to load appearance config:", e);
      }
    })();
  }, []);

  // Persist + live-apply the hotkey (accelerator + enabled)
  const applyHotkey = async (accelerator: string, enabled: boolean) => {
    try {
      const saved = await invoke<{ accelerator: string; enabled: boolean }>("set_recording_hotkey", {
        accelerator,
        enabled,
      });
      setHotkeyAccel(saved.accelerator);
      setHotkeyEnabled(saved.enabled);
      setHotkeyMsg(enabled ? `Bound to ${formatAccel(saved.accelerator)}` : "Hotkey disabled");
    } catch (e) {
      console.error("Failed to set recording hotkey:", e);
      setHotkeyMsg(`Could not bind that combo (${String(e)}). Try another.`);
    }
  };

  // Capture the next key-combo the user presses and bind it
  const handleHotkeyKeyDown = async (e: React.KeyboardEvent) => {
    if (!capturingHotkey) return;
    e.preventDefault();
    e.stopPropagation();
    const key = e.key;
    if (key === "Escape") {
      setCapturingHotkey(false);
      setHotkeyMsg(null);
      return;
    }
    // Ignore lone modifier presses — wait for a real key
    if (["Control", "Shift", "Alt", "Meta"].includes(key)) return;
    const parts: string[] = [];
    if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");
    let main = key.length === 1 ? key.toUpperCase() : key;
    if (main === " ") main = "Space";
    parts.push(main);
    if (parts.length < 2) {
      setHotkeyMsg("Use at least one modifier (Cmd/Ctrl/Shift/Alt) + a key.");
      return;
    }
    setCapturingHotkey(false);
    await applyHotkey(parts.join("+"), true);
  };

  const handleToggleHotkeyEnabled = async (enabled: boolean) => {
    await applyHotkey(hotkeyAccel, enabled);
  };

  const handleToggleMenuBarOnly = async (value: boolean) => {
    try {
      const saved = await invoke<{ menu_bar_only: boolean }>("set_dock_visibility", {
        menu_bar_only: value,
      });
      setMenuBarOnly(saved.menu_bar_only);
    } catch (e) {
      console.error("Failed to set dock visibility:", e);
    }
  };

  // Lazy load preferences on mount (only loads if not already cached)
  useEffect(() => {
    loadPreferences();
    // Reset tracking ref on mount (every tab visit)
    hasTrackedViewRef.current = false;
  }, [loadPreferences]);

  // Track preferences viewed analytics on every tab visit (once per mount)
  useEffect(() => {
    if (hasTrackedViewRef.current) return;

    const trackPreferencesViewed = async () => {
      // Wait for notification settings to be available (either from cache or after loading)
      if (notificationSettings) {
        await Analytics.track('preferences_viewed', {
          notifications_enabled: notificationSettings.notification_preferences.show_recording_started ? 'true' : 'false'
        });
        hasTrackedViewRef.current = true;
      } else if (!isLoadingPreferences) {
        // If not loading and no settings available, track with default value
        await Analytics.track('preferences_viewed', {
          notifications_enabled: 'false'
        });
        hasTrackedViewRef.current = true;
      }
    };

    trackPreferencesViewed();
  }, [notificationSettings, isLoadingPreferences]);

  // Update notificationsEnabled when notificationSettings are loaded from global state
  useEffect(() => {
    if (notificationSettings) {
      // Notification enabled means both started and stopped notifications are enabled
      const enabled =
        notificationSettings.notification_preferences.show_recording_started &&
        notificationSettings.notification_preferences.show_recording_stopped;
      setNotificationsEnabled(enabled);
      if (isInitialLoad) {
        setPreviousNotificationsEnabled(enabled);
        setIsInitialLoad(false);
      }
    } else if (!isLoadingPreferences) {
      // If not loading and no settings, use default
      setNotificationsEnabled(true);
      if (isInitialLoad) {
        setPreviousNotificationsEnabled(true);
        setIsInitialLoad(false);
      }
    }
  }, [notificationSettings, isLoadingPreferences, isInitialLoad])

  useEffect(() => {
    // Skip update on initial load or if value hasn't actually changed
    if (isInitialLoad || notificationsEnabled === null || notificationsEnabled === previousNotificationsEnabled) return;
    if (!notificationSettings) return;

    const handleUpdateNotificationSettings = async () => {
      console.log("Updating notification settings to:", notificationsEnabled);

      try {
        // Update the notification preferences
        const updatedSettings: NotificationSettings = {
          ...notificationSettings,
          notification_preferences: {
            ...notificationSettings.notification_preferences,
            show_recording_started: notificationsEnabled,
            show_recording_stopped: notificationsEnabled,
          }
        };

        console.log("Calling updateNotificationSettings with:", updatedSettings);
        await updateNotificationSettings(updatedSettings);
        setPreviousNotificationsEnabled(notificationsEnabled);
        console.log("Successfully updated notification settings to:", notificationsEnabled);

        // Track notification preference change - only fires when user manually toggles
        await Analytics.track('notification_settings_changed', {
          notifications_enabled: notificationsEnabled.toString()
        });
      } catch (error) {
        console.error('Failed to update notification settings:', error);
      }
    };

    handleUpdateNotificationSettings();
  }, [notificationsEnabled, notificationSettings, isInitialLoad, previousNotificationsEnabled, updateNotificationSettings])

  const handleOpenFolder = async (folderType: 'database' | 'models' | 'recordings') => {
    try {
      switch (folderType) {
        case 'database':
          await invoke('open_database_folder');
          break;
        case 'models':
          await invoke('open_models_folder');
          break;
        case 'recordings':
          await invoke('open_recordings_folder');
          break;
      }

      // Track storage folder access
      await Analytics.track('storage_folder_opened', {
        folder_type: folderType
      });
    } catch (error) {
      console.error(`Failed to open ${folderType} folder:`, error);
    }
  };

  // Show loading only if we're actually loading and don't have cached data
  if (isLoadingPreferences && !notificationSettings && !storageLocations) {
    return <div className="max-w-2xl mx-auto p-6">Loading Preferences...</div>
  }

  // Show loading if notificationsEnabled hasn't been determined yet
  if (notificationsEnabled === null && !isLoadingPreferences) {
    return <div className="max-w-2xl mx-auto p-6">Loading Preferences...</div>
  }

  // Ensure we have a boolean value for the Switch component
  const notificationsEnabledValue = notificationsEnabled ?? false;

  return (
    <div className="space-y-6">
      {/* Notifications Section */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Notifications</h3>
            <p className="text-sm text-gray-600">Enable or disable notifications of start and end of meeting</p>
          </div>
          <Switch checked={notificationsEnabledValue} onCheckedChange={setNotificationsEnabled} />
        </div>
      </div>

      {/* Recording Hotkey Section (#869) */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Global Recording Hotkey</h3>
            <p className="text-sm text-gray-600">
              Start or stop recording from anywhere, even when Meetily is in the background.
            </p>
          </div>
          <Switch checked={hotkeyEnabled} onCheckedChange={handleToggleHotkeyEnabled} />
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => {
              setCapturingHotkey(true);
              setHotkeyMsg("Press your new shortcut…");
            }}
            onKeyDown={handleHotkeyKeyDown}
            disabled={!hotkeyEnabled}
            className={`px-4 py-2 text-sm font-mono rounded-md border transition-colors ${
              capturingHotkey
                ? "bg-blue-50 border-blue-400 text-blue-700 ring-2 ring-blue-300"
                : "bg-gray-50 border-gray-300 text-gray-800 hover:bg-gray-100"
            } disabled:opacity-50 disabled:pointer-events-none`}
          >
            {capturingHotkey ? "Recording…" : formatAccel(hotkeyAccel)}
          </button>
          <button
            type="button"
            onClick={() => applyHotkey("CmdOrCtrl+Shift+R", true)}
            disabled={!hotkeyEnabled}
            className="px-3 py-2 text-xs border border-gray-300 rounded-md hover:bg-gray-100 disabled:opacity-50 disabled:pointer-events-none"
          >
            Reset
          </button>
        </div>
        {hotkeyMsg && <p className="text-xs text-gray-500 mt-2">{hotkeyMsg}</p>}
      </div>

      {/* Menu Bar Mode Section (#428) */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Menu Bar Only</h3>
            <p className="text-sm text-gray-600">
              Hide the Dock icon and run Meetily purely from the menu bar. Reach the app anytime
              from its tray icon.
            </p>
          </div>
          <Switch checked={menuBarOnly} onCheckedChange={handleToggleMenuBarOnly} />
        </div>
      </div>

      {/* Data Storage Locations Section */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <h3 className="text-lg font-semibold text-gray-900 mb-4">Data Storage Locations</h3>
        <p className="text-sm text-gray-600 mb-6">
          View and access where Meetily stores your data
        </p>

        <div className="space-y-4">
          {/* Database Location */}
          {/* <div className="p-4 border rounded-lg bg-gray-50">
            <div className="font-medium mb-2">Database</div>
            <div className="text-sm text-gray-600 mb-3 break-all font-mono text-xs">
              {storageLocations?.database || 'Loading...'}
            </div>
            <button
              onClick={() => handleOpenFolder('database')}
              className="flex items-center gap-2 px-3 py-2 text-sm border border-gray-300 rounded-md hover:bg-gray-100 transition-colors"
            >
              <FolderOpen className="w-4 h-4" />
              Open Folder
            </button>
          </div> */}

          {/* Models Location */}
          {/* <div className="p-4 border rounded-lg bg-gray-50">
            <div className="font-medium mb-2">Whisper Models</div>
            <div className="text-sm text-gray-600 mb-3 break-all font-mono text-xs">
              {storageLocations?.models || 'Loading...'}
            </div>
            <button
              onClick={() => handleOpenFolder('models')}
              className="flex items-center gap-2 px-3 py-2 text-sm border border-gray-300 rounded-md hover:bg-gray-100 transition-colors"
            >
              <FolderOpen className="w-4 h-4" />
              Open Folder
            </button>
          </div> */}

          {/* Recordings Location */}
          <div className="p-4 border rounded-lg bg-gray-50">
            <div className="font-medium mb-2">Meeting Recordings</div>
            <div className="text-sm text-gray-600 mb-3 break-all font-mono text-xs">
              {storageLocations?.recordings || 'Loading...'}
            </div>
            <button
              onClick={() => handleOpenFolder('recordings')}
              className="flex items-center gap-2 px-3 py-2 text-sm border border-gray-300 rounded-md hover:bg-gray-100 transition-colors"
            >
              <FolderOpen className="w-4 h-4" />
              Open Folder
            </button>
          </div>
        </div>

        <div className="mt-4 p-3 bg-blue-50 rounded-md">
          <p className="text-xs text-blue-800">
            <strong>Note:</strong> Database and models are stored together in your application data directory for unified management.
          </p>
        </div>
      </div>

      {/* Analytics Section */}
      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
        <AnalyticsConsentSwitch />
      </div>
    </div>
  )
}
