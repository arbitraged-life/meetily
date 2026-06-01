'use client';

import { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface ScreenContextEntry {
  timestamp: number; // seconds from recording start
  app_name: string;
  window_title: string;
  url?: string;
  bundle_id?: string;
}

interface Props {
  isRecording: boolean;
  recordingStartTime?: number;
}

const APP_ICONS: Record<string, string> = {
  'Safari': '🌐',
  'Google Chrome': '🌐',
  'Arc': '🌐',
  'Firefox': '🌐',
  'Microsoft Edge': '🌐',
  'Slack': '💬',
  'Discord': '💬',
  'Messages': '💬',
  'Microsoft Teams': '📞',
  'Zoom': '📹',
  'Finder': '📁',
  'Terminal': '⬛',
  'iTerm2': '⬛',
  'Visual Studio Code': '📝',
  'Cursor': '📝',
  'Xcode': '🔨',
  'Notes': '📒',
  'Mail': '📧',
  'Calendar': '📅',
  'Preview': '🖼️',
  'Figma': '🎨',
};

function getAppIcon(appName: string): string {
  return APP_ICONS[appName] || '📱';
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export default function ScreenContextTimeline({ isRecording, recordingStartTime }: Props) {
  const [entries, setEntries] = useState<ScreenContextEntry[]>([]);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const unlisten = listen<ScreenContextEntry>('screen-context-update', (event) => {
      setEntries(prev => [...prev, event.payload]);
    });

    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Reset on new recording
  useEffect(() => {
    if (isRecording) {
      setEntries([]);
    }
  }, [isRecording]);

  // Group consecutive entries with same app
  const grouped = useMemo(() => {
    if (entries.length === 0) return [];
    const groups: { app: string; icon: string; entries: ScreenContextEntry[]; startTime: number; endTime: number }[] = [];
    let current = {
      app: entries[0].app_name,
      icon: getAppIcon(entries[0].app_name),
      entries: [entries[0]],
      startTime: entries[0].timestamp,
      endTime: entries[0].timestamp,
    };

    for (let i = 1; i < entries.length; i++) {
      const e = entries[i];
      if (e.app_name === current.app) {
        current.entries.push(e);
        current.endTime = e.timestamp;
      } else {
        groups.push(current);
        current = {
          app: e.app_name,
          icon: getAppIcon(e.app_name),
          entries: [e],
          startTime: e.timestamp,
          endTime: e.timestamp,
        };
      }
    }
    groups.push(current);
    return groups;
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="text-xs text-gray-500 px-3 py-2">
        {isRecording ? 'Capturing screen context...' : 'No screen context captured'}
      </div>
    );
  }

  return (
    <div className="border-t border-gray-200 dark:border-gray-700">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-3 py-2 text-xs font-medium text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800"
      >
        <span>🖥 Screen Context ({entries.length} captures)</span>
        <span>{expanded ? '▼' : '▶'}</span>
      </button>

      {expanded && (
        <div className="px-3 pb-2 space-y-1 max-h-48 overflow-y-auto">
          {grouped.map((group, idx) => (
            <div key={idx} className="flex items-start gap-2 text-xs py-1 border-l-2 border-gray-300 dark:border-gray-600 pl-2">
              <span className="flex-shrink-0">{group.icon}</span>
              <div className="min-w-0 flex-1">
                <div className="font-medium text-gray-700 dark:text-gray-300 truncate">
                  {group.app}
                  <span className="text-gray-400 ml-1">
                    {formatTime(group.startTime)}
                    {group.endTime > group.startTime && ` – ${formatTime(group.endTime)}`}
                  </span>
                </div>
                {group.entries.length <= 3 ? (
                  group.entries.map((e, i) => (
                    <div key={i} className="text-gray-500 dark:text-gray-500 truncate">
                      {e.window_title}
                      {e.url && <span className="text-blue-500 ml-1">({new URL(e.url).hostname})</span>}
                    </div>
                  ))
                ) : (
                  <div className="text-gray-500 dark:text-gray-500 truncate">
                    {group.entries[0].window_title}
                    <span className="text-gray-400 ml-1">+{group.entries.length - 1} more</span>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
