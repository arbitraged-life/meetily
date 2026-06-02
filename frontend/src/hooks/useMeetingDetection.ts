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

export interface PendingMeetingAction {
  appName: string;
  detectedAt: number;
}

interface UseMeetingDetectionOptions {
  onMeetingDetected?: (appName: string) => void;
  onMeetingEnded?: (appName: string) => void;
  autoStartRecording?: boolean;
}

export function useMeetingDetection({
  onMeetingDetected,
  onMeetingEnded,
  autoStartRecording = false,
}: UseMeetingDetectionOptions = {}) {
  const [isEnabled, setIsEnabled] = useState(false);
  const [detectedApp, setDetectedApp] = useState<string | null>(null);
  const [pendingStart, setPendingStart] = useState<PendingMeetingAction | null>(null);
  const [pendingStop, setPendingStop] = useState<PendingMeetingAction | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const enable = useCallback(async () => {
    await invoke("set_meeting_detection_enabled", { enabled: true });
    setIsEnabled(true);
  }, []);

  const disable = useCallback(async () => {
    await invoke("set_meeting_detection_enabled", { enabled: false });
    setIsEnabled(false);
    setDetectedApp(null);
    setPendingStart(null);
    setPendingStop(null);
  }, []);

  const confirmStart = useCallback(async () => {
    if (!pendingStart) return;
    const { appName } = pendingStart;
    setPendingStart(null);
    try {
      await invoke("start_recording", { meetingName: `${appName} Meeting` });
    } catch (err) {
      console.error("start_recording failed:", err);
    }
  }, [pendingStart]);

  const cancelStart = useCallback(() => {
    setPendingStart(null);
  }, []);

  const confirmStop = useCallback(async () => {
    if (!pendingStop) return;
    setPendingStop(null);
    try {
      await invoke("stop_recording");
    } catch (err) {
      console.error("stop_recording failed:", err);
    }
  }, [pendingStop]);

  const cancelStop = useCallback(() => {
    setPendingStop(null);
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
              // Legacy direct mode (unused when AutoRecordPrompt is mounted)
              try {
                await invoke("start_recording", {
                  meetingName: `${appName} Meeting`,
                });
              } catch (err) {
                console.error("Auto-start recording failed:", err);
              }
            } else {
              // Signal pending start — AutoRecordPrompt picks this up
              setPendingStart({ appName, detectedAt: Date.now() });
            }
          }

          if (event.MeetingEnded) {
            const appName = event.MeetingEnded.app_name;
            setDetectedApp(null);
            onMeetingEnded?.(appName);

            if (!autoStartRecording) {
              setPendingStop({ appName, detectedAt: Date.now() });
            }
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
    pendingStart,
    pendingStop,
    enable,
    disable,
    confirmStart,
    cancelStart,
    confirmStop,
    cancelStop,
  };
}
