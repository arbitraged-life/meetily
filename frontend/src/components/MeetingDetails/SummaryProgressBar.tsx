"use client";

import { useEffect, useRef, useState } from 'react';
import { Progress } from '@/components/ui/progress';
import { Loader2 } from 'lucide-react';

type SummaryStatus = 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';

interface SummaryProgressBarProps {
  status: SummaryStatus;
  message?: string;
  className?: string;
}

const ACTIVE_STATUSES: SummaryStatus[] = ['processing', 'summarizing', 'regenerating'];

/**
 * Non-blocking progress indicator for summary generation / regeneration.
 *
 * The backend performs a single LLM call and only reports coarse status
 * (no per-chunk percentage over the wire), so we render a time-based
 * simulated bar: it climbs quickly at first then asymptotically approaches
 * ~95%, and snaps to 100% on completion. This is the same honest pattern
 * used by NProgress / YouTube-style loaders — it communicates "work is
 * happening" without pretending to know the exact percentage.
 */
export function SummaryProgressBar({ status, message, className }: SummaryProgressBarProps) {
  const [progress, setProgress] = useState(0);
  const startRef = useRef<number | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isActive = ACTIVE_STATUSES.includes(status);

  useEffect(() => {
    if (isActive) {
      if (startRef.current === null) {
        startRef.current = Date.now();
      }

      const tick = () => {
        const elapsedSec = (Date.now() - (startRef.current as number)) / 1000;
        // Asymptotic approach to 95%. Time constant ~20s: ~63% at 20s, ~86% at 40s.
        const target = 95 * (1 - Math.exp(-elapsedSec / 20));
        setProgress((prev) => (target > prev ? target : prev));
        timerRef.current = setTimeout(tick, 400);
      };

      tick();

      return () => {
        if (timerRef.current) clearTimeout(timerRef.current);
      };
    }

    // Not active: reset so the next run starts from zero.
    startRef.current = null;
    if (status === 'completed') {
      setProgress(100);
    } else {
      setProgress(0);
    }
  }, [isActive, status]);

  // Only render while actively generating.
  if (!isActive) return null;

  return (
    <div className={className}>
      <div className="flex items-center gap-2 mb-1.5">
        <Loader2 className="h-3.5 w-3.5 animate-spin text-blue-500 flex-shrink-0" />
        <span className="text-xs font-medium text-blue-600 truncate">
          {message || 'Generating summary…'}
        </span>
        <span className="ml-auto text-xs tabular-nums text-gray-400 flex-shrink-0">
          {Math.round(progress)}%
        </span>
      </div>
      <Progress
        value={progress}
        className="h-1.5 bg-blue-100 [&>div]:bg-gradient-to-r [&>div]:from-blue-500 [&>div]:to-purple-500"
      />
    </div>
  );
}
