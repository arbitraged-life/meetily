import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface FeatureFlags {
  urlImportEnabled: boolean;
  autoMuteEnabled: boolean;
  transcriptTagsEnabled: boolean;
  diarizationEnabled: boolean;
  dictionaryEnabled: boolean;
  screenContextEnabled: boolean;
  calendarEnabled: boolean;
  atollBridgeEnabled: boolean;
  analyticsEnabled: boolean;
  whisperPreload: boolean;
  parakeetPreload: boolean;
  builtinAiPreload: boolean;
}

interface FlagDef {
  key: keyof FeatureFlags;
  label: string;
  description: string;
  category: "recording" | "startup" | "integration";
}

const FLAG_DEFS: FlagDef[] = [
  // Recording features
  { key: "urlImportEnabled", label: "URL Audio Import", description: "Import audio from YouTube links or direct URLs. Requires yt-dlp for YouTube.", category: "recording" },
  { key: "autoMuteEnabled", label: "Auto-Mute System Audio", description: "Mute system audio output while recording to prevent feedback. Restores on stop.", category: "recording" },
  { key: "transcriptTagsEnabled", label: "Transcript Tag Wrapping", description: "Wrap transcripts in <transcription> tags before sending to LLM.", category: "recording" },
  { key: "screenContextEnabled", label: "Screen Context OCR", description: "Capture screen context during recording via OCR. Adds CPU overhead.", category: "recording" },
  { key: "diarizationEnabled", label: "Speaker Diarization", description: "Load ONNX embedding model (~50MB RAM) for speaker label identification.", category: "recording" },
  { key: "dictionaryEnabled", label: "Custom Dictionary", description: "Load custom word dictionary + file watcher for domain-specific terms.", category: "recording" },
  // Startup preloading
  { key: "whisperPreload", label: "Preload Whisper Engine", description: "Initialize Whisper model at startup instead of lazy-loading on first use.", category: "startup" },
  { key: "parakeetPreload", label: "Preload Parakeet Engine", description: "Initialize Parakeet model at startup instead of lazy-loading on first use.", category: "startup" },
  { key: "builtinAiPreload", label: "Preload Built-in AI (ModelManager)", description: "Initialize the built-in AI summary engine at startup instead of lazy-loading on first use.", category: "startup" },
  // Integrations
  { key: "calendarEnabled", label: "Calendar ICS Poller", description: "Background sync of subscribed calendar feeds for auto-record scheduling.", category: "integration" },
  { key: "atollBridgeEnabled", label: "Atoll Notch Bridge", description: "Push meeting state to macOS notch via WebSocket (localhost:9020).", category: "integration" },
  { key: "analyticsEnabled", label: "Analytics Telemetry", description: "Send anonymous usage analytics via PostHog.", category: "integration" },
];

const CATEGORY_LABELS: Record<string, string> = {
  recording: "Recording & Transcription",
  startup: "Startup Preloading",
  integration: "Integrations",
};

export function FeatureSettings() {
  const [flags, setFlags] = useState<FeatureFlags | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<FeatureFlags>("get_feature_flags")
      .then(setFlags)
      .finally(() => setLoading(false));
  }, []);

  const toggleFlag = async (feature: string, enabled: boolean) => {
    const updated = await invoke<FeatureFlags>("set_feature_flag", { feature, enabled });
    setFlags(updated);
  };

  if (loading || !flags) {
    return <div className="p-4 text-muted-foreground">Loading...</div>;
  }

  const categories = ["recording", "startup", "integration"] as const;

  return (
    <div className="space-y-6 p-4">
      <div>
        <h3 className="text-lg font-semibold mb-1">Feature Flags</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Toggle optional features. Disabled features have zero performance cost — nothing is loaded or initialized.
        </p>
      </div>

      {categories.map((cat) => (
        <div key={cat} className="space-y-3">
          <h4 className="text-sm font-semibold text-muted-foreground uppercase tracking-wide">
            {CATEGORY_LABELS[cat]}
          </h4>
          {FLAG_DEFS.filter((f) => f.category === cat).map((def) => (
            <label
              key={def.key}
              className="flex items-center justify-between p-3 rounded-lg border hover:bg-accent/50 cursor-pointer"
            >
              <div>
                <div className="font-medium">{def.label}</div>
                <div className="text-sm text-muted-foreground">{def.description}</div>
              </div>
              <input
                type="checkbox"
                checked={flags[def.key]}
                onChange={(e) => toggleFlag(def.key, e.target.checked)}
                className="h-5 w-5 rounded"
              />
            </label>
          ))}
        </div>
      ))}

      <p className="text-xs text-muted-foreground italic">
        Some changes (Whisper/Parakeet preload, Diarization, Calendar, Atoll) take effect on next app restart.
      </p>
    </div>
  );
}
