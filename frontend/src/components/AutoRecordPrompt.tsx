"use client";

import React, { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Mic, MicOff } from "lucide-react";
import type { PendingMeetingAction } from "@/hooks/useMeetingDetection";

interface AutoRecordPromptProps {
  pendingStart: PendingMeetingAction | null;
  pendingStop: PendingMeetingAction | null;
  onConfirmStart: () => void;
  onCancelStart: () => void;
  onConfirmStop: () => void;
  onCancelStop: () => void;
  /** Countdown duration in seconds (default: 10) */
  startCountdown?: number;
  stopCountdown?: number;
  autoStart?: boolean;
  autoStop?: boolean;
}

interface CountdownBannerProps {
  label: string;
  countdown: number;
  total: number;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  icon: React.ReactNode;
  colorClass: string;
}

function CountdownBanner({
  label,
  countdown,
  total,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  icon,
  colorClass,
}: CountdownBannerProps) {
  const pct = total > 0 ? ((total - countdown) / total) * 100 : 100;

  return (
    <div
      className={`fixed bottom-6 right-6 z-50 w-80 rounded-xl shadow-2xl border bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-700 overflow-hidden`}
    >
      {/* Progress bar */}
      <div className="h-1 w-full bg-gray-100 dark:bg-gray-800">
        <div
          className={`h-full transition-all duration-1000 ease-linear ${colorClass}`}
          style={{ width: `${pct}%` }}
        />
      </div>

      <div className="p-4 space-y-3">
        <div className="flex items-start gap-3">
          <div className={`mt-0.5 shrink-0 ${colorClass.replace("bg-", "text-")}`}>
            {icon}
          </div>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100 leading-snug">
            {label}{" "}
            <span className="font-normal text-gray-500 dark:text-gray-400">
              ({countdown}s)
            </span>
          </p>
        </div>

        <div className="flex gap-2 justify-end">
          <Button
            size="sm"
            variant="outline"
            onClick={onCancel}
            className="text-xs"
          >
            {cancelLabel}
          </Button>
          <Button
            size="sm"
            onClick={onConfirm}
            className={`text-xs text-white ${colorClass} hover:opacity-90`}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}

export function AutoRecordPrompt({
  pendingStart,
  pendingStop,
  onConfirmStart,
  onCancelStart,
  onConfirmStop,
  onCancelStop,
  startCountdown: startTotal = 10,
  stopCountdown: stopTotal = 10,
  autoStart = true,
  autoStop = true,
}: AutoRecordPromptProps) {
  const [startSecs, setStartSecs] = useState(startTotal);
  const [stopSecs, setStopSecs] = useState(stopTotal);
  const startRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const stopRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Reset and start countdown when pendingStart appears
  useEffect(() => {
    if (startRef.current) clearInterval(startRef.current);
    if (!pendingStart || !autoStart) {
      setStartSecs(startTotal);
      return;
    }
    setStartSecs(startTotal);
    startRef.current = setInterval(() => {
      setStartSecs((prev) => {
        if (prev <= 1) {
          clearInterval(startRef.current!);
          startRef.current = null;
          onConfirmStart();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => {
      if (startRef.current) clearInterval(startRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingStart]);

  // Reset and start countdown when pendingStop appears
  useEffect(() => {
    if (stopRef.current) clearInterval(stopRef.current);
    if (!pendingStop || !autoStop) {
      setStopSecs(stopTotal);
      return;
    }
    setStopSecs(stopTotal);
    stopRef.current = setInterval(() => {
      setStopSecs((prev) => {
        if (prev <= 1) {
          clearInterval(stopRef.current!);
          stopRef.current = null;
          onConfirmStop();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => {
      if (stopRef.current) clearInterval(stopRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingStop]);

  // Prefer showing stop prompt over start prompt if both somehow pending
  if (pendingStop && autoStop) {
    return (
      <CountdownBanner
        label={`Meeting ended — Recording will stop`}
        countdown={stopSecs}
        total={stopTotal}
        confirmLabel="Stop Now"
        cancelLabel="Keep Recording"
        onConfirm={onConfirmStop}
        onCancel={onCancelStop}
        icon={<MicOff className="w-4 h-4" />}
        colorClass="bg-red-500"
      />
    );
  }

  if (pendingStart && autoStart) {
    return (
      <CountdownBanner
        label={`Meeting detected: ${pendingStart.appName} — Recording will start`}
        countdown={startSecs}
        total={startTotal}
        confirmLabel="Start Now"
        cancelLabel="Cancel"
        onConfirm={onConfirmStart}
        onCancel={onCancelStart}
        icon={<Mic className="w-4 h-4" />}
        colorClass="bg-blue-500"
      />
    );
  }

  return null;
}
