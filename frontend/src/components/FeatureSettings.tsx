import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface FeatureFlags {
  urlImportEnabled: boolean;
  autoMuteEnabled: boolean;
  transcriptTagsEnabled: boolean;
}

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

  return (
    <div className="space-y-6 p-4">
      <div>
        <h3 className="text-lg font-semibold mb-1">Feature Flags</h3>
        <p className="text-sm text-muted-foreground mb-4">
          Toggle optional features. Disabled features have zero performance cost.
        </p>
      </div>

      <div className="space-y-4">
        {/* URL Import */}
        <label className="flex items-center justify-between p-3 rounded-lg border hover:bg-accent/50 cursor-pointer">
          <div>
            <div className="font-medium">URL Audio Import</div>
            <div className="text-sm text-muted-foreground">
              Import audio from YouTube links or direct URLs for transcription. Requires yt-dlp for YouTube.
            </div>
          </div>
          <input
            type="checkbox"
            checked={flags.urlImportEnabled}
            onChange={(e) => toggleFlag("urlImportEnabled", e.target.checked)}
            className="h-5 w-5 rounded"
          />
        </label>

        {/* Auto Mute */}
        <label className="flex items-center justify-between p-3 rounded-lg border hover:bg-accent/50 cursor-pointer">
          <div>
            <div className="font-medium">Auto-Mute System Audio</div>
            <div className="text-sm text-muted-foreground">
              Automatically mute system audio output while recording to prevent feedback. Restores on stop.
            </div>
          </div>
          <input
            type="checkbox"
            checked={flags.autoMuteEnabled}
            onChange={(e) => toggleFlag("autoMuteEnabled", e.target.checked)}
            className="h-5 w-5 rounded"
          />
        </label>

        {/* Transcript Tags */}
        <label className="flex items-center justify-between p-3 rounded-lg border hover:bg-accent/50 cursor-pointer">
          <div>
            <div className="font-medium">Transcript Tag Wrapping</div>
            <div className="text-sm text-muted-foreground">
              Wrap transcripts in {"<transcription>"} tags before sending to LLM. Prevents the model from answering questions found in the meeting dialog.
            </div>
          </div>
          <input
            type="checkbox"
            checked={flags.transcriptTagsEnabled}
            onChange={(e) => toggleFlag("transcriptTagsEnabled", e.target.checked)}
            className="h-5 w-5 rounded"
          />
        </label>
      </div>
    </div>
  );
}
