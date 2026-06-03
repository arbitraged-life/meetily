'use client';

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './ui/dialog';
import { Pencil, RefreshCw, Trash2, Upload } from 'lucide-react';

/** Mirror of the Rust `DictionaryEntry` struct (dictionary/mod.rs). */
interface DictionaryEntry {
  id: string;
  display: string;
  aliases: string[];
  source: string;
  updated_at: string;
}

type Mode = 'list' | 'edit';

export function DictionarySettings() {
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<Mode>('list');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingDisplay, setEditingDisplay] = useState('');
  const [editingAliases, setEditingAliases] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const list = await invoke<DictionaryEntry[]>('get_dictionary');
      setEntries(list);
    } catch (err) {
      console.error('Failed to load dictionary:', err);
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  const parseAliases = (raw: string): string[] =>
    raw
      .split(',')
      .map((a) => a.trim())
      .filter((a) => a.length > 0);

  const startCreate = () => {
    setMode('edit');
    setEditingId(null);
    setEditingDisplay('');
    setEditingAliases('');
    setError(null);
  };

  const startEdit = (entry: DictionaryEntry) => {
    setMode('edit');
    setEditingId(entry.id);
    setEditingDisplay(entry.display);
    setEditingAliases(entry.aliases.join(', '));
    setError(null);
  };

  const handleDelete = async (entry: DictionaryEntry) => {
    if (!confirm(`Delete dictionary entry "${entry.display}"?`)) return;
    try {
      await invoke('remove_dictionary_entry', { id: entry.id });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      const display = editingDisplay.trim();
      if (!display) {
        setError('Display word is required');
        setSaving(false);
        return;
      }
      const aliases = parseAliases(editingAliases);
      if (editingId) {
        await invoke('update_dictionary_entry', {
          id: editingId,
          display,
          aliases,
        });
      } else {
        await invoke('add_dictionary_entry', { display, aliases });
      }
      await refresh();
      setMode('list');
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleImportVoiceInk = async () => {
    setError(null);
    try {
      // Lazy-load the dialog plugin only when the user clicks import.
      const { open: openDialog } = await import('@tauri-apps/plugin-dialog');
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'VoiceInk Dictionary', extensions: ['json'] }],
      });
      if (!selected || typeof selected !== 'string') return;
      const count = await invoke<number>('import_voiceink_dictionary', {
        path: selected,
      });
      await refresh();
      setError(null);
      alert(`Imported ${count} entr${count === 1 ? 'y' : 'ies'} from VoiceInk.`);
    } catch (err) {
      setError(`Import failed: ${String(err)}`);
    }
  };

  return (
    <div>
      <Label className="block text-sm font-medium text-gray-700 mb-1">
        Custom Dictionary
      </Label>
      <p className="text-xs text-gray-500 mb-2 mx-1">
        Auto-corrects transcripts: maps misheard spellings (aliases) to the
        correct word/phrase. Shared across all transcription engines. Synced with
        VoiceInk and Raycast.
      </p>
      <div className="flex space-x-2 mx-1">
        <Button
          type="button"
          variant="outline"
          className="flex-1"
          onClick={() => {
            setMode('list');
            setError(null);
            setOpen(true);
          }}
        >
          Manage Dictionary…
        </Button>
      </div>

      <Dialog
        open={open}
        onOpenChange={(o) => {
          setOpen(o);
          if (!o) {
            setMode('list');
            setError(null);
          }
        }}
      >
        <DialogContent className="max-w-xl">
          <DialogHeader>
            <DialogTitle>Manage Custom Dictionary</DialogTitle>
            <DialogDescription>
              Each entry maps one or more aliases (what the engine mishears) to a
              display word (what you want in the transcript). Aliases are
              comma-separated.
            </DialogDescription>
          </DialogHeader>

          {error && (
            <div className="text-sm text-red-600 bg-red-50 border border-red-200 rounded p-2">
              {error}
            </div>
          )}

          {mode === 'list' ? (
            <div className="space-y-3">
              <div className="max-h-72 overflow-y-auto border rounded">
                {entries.length === 0 ? (
                  <div className="p-4 text-sm text-gray-500 text-center">
                    No entries yet. Click "+ Add new" to create one.
                  </div>
                ) : (
                  <ul>
                    {entries.map((entry) => (
                      <li
                        key={entry.id}
                        className="flex items-center justify-between p-2 border-b last:border-b-0 hover:bg-gray-50"
                      >
                        <div className="min-w-0 flex-1">
                          <span className="text-sm font-medium">
                            {entry.display}
                          </span>
                          {entry.aliases.length > 0 && (
                            <span className="text-xs text-gray-500 ml-2 truncate">
                              ← {entry.aliases.join(', ')}
                            </span>
                          )}
                          {entry.source !== 'meetily' && (
                            <span className="ml-2 text-[10px] uppercase tracking-wide text-gray-400">
                              {entry.source}
                            </span>
                          )}
                        </div>
                        <div className="flex space-x-1 shrink-0">
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            onClick={() => startEdit(entry)}
                            title="Edit"
                          >
                            <Pencil className="h-4 w-4" />
                          </Button>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            onClick={() => handleDelete(entry)}
                            title="Delete"
                          >
                            <Trash2 className="h-4 w-4 text-red-500" />
                          </Button>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="flex justify-between">
                <div className="flex space-x-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={refresh}
                    title="Reload entries"
                  >
                    <RefreshCw className="h-4 w-4 mr-1" />
                    Refresh
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleImportVoiceInk}
                    title="Import a VoiceInk dictionary JSON file"
                  >
                    <Upload className="h-4 w-4 mr-1" />
                    Import VoiceInk
                  </Button>
                </div>
                <Button type="button" size="sm" onClick={startCreate}>
                  + Add new
                </Button>
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              <div>
                <Label className="text-sm font-medium">Display word</Label>
                <Input
                  value={editingDisplay}
                  onChange={(e) => setEditingDisplay(e.target.value)}
                  placeholder="e.g. Kubernetes"
                  className="mt-1"
                />
                <p className="text-xs text-gray-500 mt-1">
                  The correct word/phrase as it should appear in transcripts.
                </p>
              </div>
              <div>
                <Label className="text-sm font-medium">Aliases</Label>
                <Input
                  value={editingAliases}
                  onChange={(e) => setEditingAliases(e.target.value)}
                  placeholder="e.g. cubernetes, koobernetes, k8s"
                  className="mt-1"
                />
                <p className="text-xs text-gray-500 mt-1">
                  Comma-separated misheard spellings that should map to the
                  display word. Leave blank for a pronunciation-only hint.
                </p>
              </div>
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setMode('list')}
                  disabled={saving}
                >
                  Cancel
                </Button>
                <Button type="button" onClick={handleSave} disabled={saving}>
                  {saving ? 'Saving…' : 'Save'}
                </Button>
              </DialogFooter>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
