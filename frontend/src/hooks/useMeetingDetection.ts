"use client";

import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MeetingDetectionEvent {
  MeetingStarted?: {
    app_name: string;
    process_name: string;
    detected_at: string;
    is_active: boolean;
  };
  MeetingEnded?: {
    app_name: string;
    process_name: string;
    detected_at: string;
    is_active: boolean;
  };
}

interface UseMeetingDetectionOptions {
  onMeetingDetected?: (appName: string) => void;
  onMeetingEnded?: (appName: string) => void;
  autoStartRecording?: boolean;
}

export function useMeetingDetection({
  onMeetingDetected,
  onMeetingEnded,
  autoStartRecording = true,
}: UseMeetingDetectionOptions = {}) {
  const [isEnabled, setIsEnabled] = useState(false);
  const [detectedApp, setDetectedApp] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const enable = useCallback(async () => {
    await invoke("set_meeting_detection_enabled", { enabled: true });
    setIsEnabled(true);
  }, []);

  const disable = useCallback(async () => {
    await invoke("set_meeting_detection_enabled", { enabled: false });
    setIsEnabled(false);
    setDetectedApp(null);
  }, []);

  // Poll for events
  useEffect(() => {
    if (!isEnabled) {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
      return;
    }

    pollRef.current = setInterval(async () => {
      try {
        const events = await invoke<MeetingDetectionEvent[]>(
          "poll_meeting_detection_events"
        );

        for (const event of events) {
          if (event.MeetingStarted) {
            const appName = event.MeetingStarted.app_name;
            setDetectedApp(appName);
            onMeetingDetected?.(appName);

            if (autoStartRecording) {
              try {
                await invoke("start_recording_with_meeting_name", {
                  meetingName: `${appName} Meeting`,
                });
              } catch (err) {
                console.error("Auto-start recording failed:", err);
              }
            }
          }

          if (event.MeetingEnded) {
            const appName = event.MeetingEnded.app_name;
            setDetectedApp(null);
            onMeetingEnded?.(appName);
          }
        }
      } catch (err) {
        console.error("Meeting detection poll failed:", err);
      }
    }, 3000); // Poll every 3 seconds

    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [isEnabled, onMeetingDetected, onMeetingEnded, autoStartRecording]);

  return {
    isEnabled,
    detectedApp,
    enable,
    disable,
  };
}
