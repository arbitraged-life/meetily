"use client";

import React, { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MeetingNote {
  id: string;
  text: string;
  timestamp_seconds: number;
  display_time: string;
  created_at: string;
}

interface MeetingNotesPanelProps {
  isRecording: boolean;
  recordingStartTime: number | null; // Unix ms when recording started
}

export default function MeetingNotesPanel({
  isRecording,
  recordingStartTime,
}: MeetingNotesPanelProps) {
  const [notes, setNotes] = useState<MeetingNote[]>([]);
  const [inputText, setInputText] = useState("");
  const [isExpanded, setIsExpanded] = useState(true);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const notesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to latest note
  useEffect(() => {
    notesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [notes]);

  // Load existing notes on mount/recording start
  useEffect(() => {
    if (isRecording) {
      invoke<MeetingNote[]>("get_meeting_notes")
        .then(setNotes)
        .catch(() => setNotes([]));
    } else {
      setNotes([]);
    }
  }, [isRecording]);

  const getCurrentTimestamp = useCallback((): number => {
    if (!recordingStartTime) return 0;
    return (Date.now() - recordingStartTime) / 1000;
  }, [recordingStartTime]);

  const handleAddNote = useCallback(async () => {
    const text = inputText.trim();
    if (!text || !isRecording) return;

    try {
      const timestamp = getCurrentTimestamp();
      const note = await invoke<MeetingNote>("add_meeting_note", {
        text,
        timestampSeconds: timestamp,
      });
      setNotes((prev) => [...prev, note]);
      setInputText("");
      inputRef.current?.focus();
    } catch (err) {
      console.error("Failed to add note:", err);
    }
  }, [inputText, isRecording, getCurrentTimestamp]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleAddNote();
    }
  };

  if (!isRecording) return null;

  return (
    <div className="flex flex-col border border-gray-200 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 shadow-sm">
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center justify-between px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-700 rounded-t-lg"
      >
        <span className="flex items-center gap-2">
          📝 Meeting Notes
          {notes.length > 0 && (
            <span className="text-xs bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 px-1.5 py-0.5 rounded-full">
              {notes.length}
            </span>
          )}
        </span>
        <span className="text-xs text-gray-400">
          {isExpanded ? "▼" : "▶"}
        </span>
      </button>

      {isExpanded && (
        <div className="flex flex-col gap-2 p-3 border-t border-gray-100 dark:border-gray-700">
          {/* Notes list */}
          {notes.length > 0 && (
            <div className="max-h-48 overflow-y-auto space-y-1.5">
              {notes.map((note) => (
                <div
                  key={note.id}
                  className="flex gap-2 text-sm group"
                >
                  <span className="text-xs font-mono text-blue-500 dark:text-blue-400 shrink-0 pt-0.5">
                    {note.display_time}
                  </span>
                  <span className="text-gray-700 dark:text-gray-300 whitespace-pre-wrap">
                    {note.text}
                  </span>
                </div>
              ))}
              <div ref={notesEndRef} />
            </div>
          )}

          {/* Input */}
          <div className="flex gap-2">
            <textarea
              ref={inputRef}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a note... (Enter to save)"
              rows={1}
              className="flex-1 px-3 py-1.5 text-sm border border-gray-200 dark:border-gray-600 rounded-md bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 placeholder-gray-400 resize-none focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
            <button
              onClick={handleAddNote}
              disabled={!inputText.trim()}
              className="px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed shrink-0"
            >
              +
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
