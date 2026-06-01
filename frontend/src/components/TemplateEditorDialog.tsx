'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Textarea } from './ui/textarea';
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from './ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
} from './ui/dropdown-menu';
import {
  Plus,
  Pencil,
  Copy,
  Trash2,
  RotateCcw,
  GripVertical,
  ChevronUp,
  ChevronDown,
} from 'lucide-react';

// ── Types ──────────────────────────────────────────────────────────

interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  isCustom: boolean;
  source: 'custom' | 'bundled' | 'builtIn';
  templateJson: string;
}

interface TemplateSection {
  title: string;
  instruction: string;
  format: 'paragraph' | 'list' | 'string';
  item_format?: string;
}

interface TemplateDraft {
  name: string;
  description: string;
  sections: TemplateSection[];
}

// ── Main Component ─────────────────────────────────────────────────

interface TemplateEditorDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onTemplatesChanged?: () => void;
}

export function TemplateEditorDialog({
  open,
  onOpenChange,
  onTemplatesChanged,
}: TemplateEditorDialogProps) {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [isDirty, setIsDirty] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [mode, setMode] = useState<'list' | 'edit' | 'create'>('list');

  // ── Load templates ────────────────────────────────────────────────

  const loadTemplates = useCallback(async () => {
    try {
      const list = await invoke<TemplateInfo[]>('api_list_templates');
      // Enrich with full data
      const enriched: TemplateInfo[] = [];
      for (const t of list) {
        try {
          const full = await invoke<TemplateInfo>('api_get_template_json', {
            templateId: t.id,
          });
          enriched.push(full);
        } catch {
          enriched.push({ ...t, isCustom: false, source: 'builtIn', templateJson: '' });
        }
      }
      setTemplates(enriched);
    } catch (error) {
      toast.error('Failed to load templates');
    }
  }, []);

  useEffect(() => {
    if (open) {
      loadTemplates();
      setMode('list');
      setSelectedId(null);
      setDraft(null);
    }
  }, [open, loadTemplates]);

  // ── Actions ───────────────────────────────────────────────────────

  const openEditor = (template: TemplateInfo) => {
    try {
      const parsed = JSON.parse(template.templateJson);
      setDraft({
        name: parsed.name || template.name,
        description: parsed.description || template.description,
        sections: (parsed.sections || []).map((s: TemplateSection) => ({
          title: s.title || '',
          instruction: s.instruction || '',
          format: s.format || 'paragraph',
          item_format: s.item_format,
        })),
      });
      setSelectedId(template.id);
      setMode('edit');
      setIsDirty(false);
    } catch {
      toast.error('Failed to parse template');
    }
  };

  const startCreate = () => {
    setDraft({
      name: '',
      description: '',
      sections: [{ title: 'Summary', instruction: 'Provide a brief summary', format: 'paragraph' }],
    });
    setSelectedId(null);
    setMode('create');
    setIsDirty(true);
  };

  const handleDuplicate = async (template: TemplateInfo) => {
    try {
      await invoke<TemplateInfo>('api_duplicate_template', {
        templateId: template.id,
      });
      toast.success(`Duplicated "${template.name}"`);
      await loadTemplates();
      onTemplatesChanged?.();
    } catch (error) {
      toast.error('Failed to duplicate template', { description: String(error) });
    }
  };

  const handleDelete = async (template: TemplateInfo) => {
    if (!template.isCustom) {
      toast.error('Cannot delete bundled templates');
      return;
    }
    try {
      await invoke('api_delete_template', { templateId: template.id });
      toast.success(`Deleted "${template.name}"`);
      await loadTemplates();
      onTemplatesChanged?.();
    } catch (error) {
      toast.error('Failed to delete', { description: String(error) });
    }
  };

  const handleReset = async (template: TemplateInfo) => {
    try {
      await invoke<TemplateInfo>('api_reset_template', { templateId: template.id });
      toast.success(`Reset "${template.name}" to default`);
      await loadTemplates();
      onTemplatesChanged?.();
    } catch (error) {
      toast.error('Failed to reset', { description: String(error) });
    }
  };

  const handleSave = async () => {
    if (!draft) return;
    setIsSaving(true);

    const templateObj = {
      name: draft.name,
      description: draft.description,
      sections: draft.sections.map((s) => {
        const section: Record<string, string> = {
          title: s.title,
          instruction: s.instruction,
          format: s.format,
        };
        if (s.item_format) section.item_format = s.item_format;
        return section;
      }),
    };
    const json = JSON.stringify(templateObj, null, 2);

    try {
      if (mode === 'create') {
        await invoke<TemplateInfo>('api_create_template', { templateJson: json });
        toast.success(`Created "${draft.name}"`);
      } else if (selectedId) {
        await invoke<TemplateInfo>('api_save_template', {
          templateId: selectedId,
          templateJson: json,
        });
        toast.success(`Saved "${draft.name}"`);
      }
      await loadTemplates();
      onTemplatesChanged?.();
      setMode('list');
      setIsDirty(false);
    } catch (error) {
      toast.error('Save failed', { description: String(error) });
    } finally {
      setIsSaving(false);
    }
  };

  // ── Section operations ────────────────────────────────────────────

  const updateSection = (index: number, field: keyof TemplateSection, value: string) => {
    if (!draft) return;
    const sections = [...draft.sections];
    sections[index] = { ...sections[index], [field]: value };
    setDraft({ ...draft, sections });
    setIsDirty(true);
  };

  const addSection = () => {
    if (!draft) return;
    setDraft({
      ...draft,
      sections: [...draft.sections, { title: '', instruction: '', format: 'paragraph' }],
    });
    setIsDirty(true);
  };

  const removeSection = (index: number) => {
    if (!draft) return;
    setDraft({ ...draft, sections: draft.sections.filter((_, i) => i !== index) });
    setIsDirty(true);
  };

  const moveSection = (index: number, direction: -1 | 1) => {
    if (!draft) return;
    const newIndex = index + direction;
    if (newIndex < 0 || newIndex >= draft.sections.length) return;
    const sections = [...draft.sections];
    [sections[index], sections[newIndex]] = [sections[newIndex], sections[index]];
    setDraft({ ...draft, sections });
    setIsDirty(true);
  };

  // ── Render ────────────────────────────────────────────────────────

  if (mode === 'edit' || mode === 'create') {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl max-h-[85vh] overflow-y-auto">
          <DialogTitle>{mode === 'create' ? 'Create Template' : 'Edit Template'}</DialogTitle>

          <div className="space-y-4 mt-4">
            {/* Name & Description */}
            <div className="grid grid-cols-1 gap-3">
              <div>
                <Label htmlFor="tpl-name">Name</Label>
                <Input
                  id="tpl-name"
                  value={draft?.name || ''}
                  onChange={(e) => {
                    if (draft) { setDraft({ ...draft, name: e.target.value }); setIsDirty(true); }
                  }}
                  placeholder="e.g. Sprint Retrospective"
                />
              </div>
              <div>
                <Label htmlFor="tpl-desc">Description</Label>
                <Input
                  id="tpl-desc"
                  value={draft?.description || ''}
                  onChange={(e) => {
                    if (draft) { setDraft({ ...draft, description: e.target.value }); setIsDirty(true); }
                  }}
                  placeholder="Brief description of when to use this template"
                />
              </div>
            </div>

            {/* Sections */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <Label>Sections</Label>
                <Button variant="outline" size="sm" onClick={addSection}>
                  <Plus className="h-3 w-3 mr-1" /> Add Section
                </Button>
              </div>

              <div className="space-y-3">
                {draft?.sections.map((section, idx) => (
                  <div
                    key={idx}
                    className="border border-gray-200 rounded-lg p-3 space-y-2 bg-gray-50/50"
                  >
                    <div className="flex items-center gap-2">
                      <GripVertical className="h-4 w-4 text-gray-400 shrink-0" />
                      <Input
                        value={section.title}
                        onChange={(e) => updateSection(idx, 'title', e.target.value)}
                        placeholder="Section title"
                        className="font-medium"
                      />
                      <Select
                        value={section.format}
                        onValueChange={(val) => updateSection(idx, 'format', val)}
                      >
                        <SelectTrigger className="w-32">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="paragraph">Paragraph</SelectItem>
                          <SelectItem value="list">List</SelectItem>
                          <SelectItem value="string">String</SelectItem>
                        </SelectContent>
                      </Select>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 shrink-0"
                        onClick={() => moveSection(idx, -1)}
                        disabled={idx === 0}
                      >
                        <ChevronUp className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 shrink-0"
                        onClick={() => moveSection(idx, 1)}
                        disabled={idx === (draft?.sections.length || 0) - 1}
                      >
                        <ChevronDown className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 shrink-0 text-red-500 hover:text-red-700"
                        onClick={() => removeSection(idx)}
                        disabled={(draft?.sections.length || 0) <= 1}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                    <Textarea
                      value={section.instruction}
                      onChange={(e) => updateSection(idx, 'instruction', e.target.value)}
                      placeholder="Instruction for the AI (what to extract/summarize for this section)"
                      className="text-sm min-h-[60px]"
                    />
                    {section.format === 'list' && (
                      <Input
                        value={section.item_format || ''}
                        onChange={(e) => updateSection(idx, 'item_format', e.target.value)}
                        placeholder="Optional: item format (e.g. '- [ ] {action} — @{owner}')"
                        className="text-xs"
                      />
                    )}
                  </div>
                ))}
              </div>
            </div>

            {/* Actions */}
            <div className="flex justify-end gap-2 pt-2 border-t">
              <Button
                variant="outline"
                onClick={() => { setMode('list'); setIsDirty(false); }}
              >
                Cancel
              </Button>
              <Button
                onClick={handleSave}
                disabled={isSaving || !draft?.name.trim() || !draft?.sections.length}
              >
                {isSaving ? 'Saving...' : mode === 'create' ? 'Create' : 'Save'}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    );
  }

  // ── List view ─────────────────────────────────────────────────────

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg max-h-[80vh] overflow-y-auto">
        <DialogTitle>Manage Templates</DialogTitle>

        <div className="space-y-2 mt-4">
          {templates.map((template) => (
            <div
              key={template.id}
              className="flex items-center justify-between p-3 rounded-lg border border-gray-200 hover:bg-gray-50 transition-colors"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-sm truncate">{template.name}</span>
                  {template.isCustom && (
                    <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-blue-100 text-blue-700">
                      Custom
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-500 truncate">{template.description}</p>
              </div>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="sm" className="shrink-0 ml-2">
                    •••
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={() => openEditor(template)}>
                    <Pencil className="h-4 w-4 mr-2" /> Edit
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => handleDuplicate(template)}>
                    <Copy className="h-4 w-4 mr-2" /> Duplicate
                  </DropdownMenuItem>
                  {template.isCustom && (
                    <>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem onClick={() => handleReset(template)}>
                        <RotateCcw className="h-4 w-4 mr-2" /> Reset to Default
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() => handleDelete(template)}
                        className="text-red-600"
                      >
                        <Trash2 className="h-4 w-4 mr-2" /> Delete
                      </DropdownMenuItem>
                    </>
                  )}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          ))}
        </div>

        <div className="flex justify-between pt-3 border-t mt-3">
          <Button variant="outline" onClick={startCreate}>
            <Plus className="h-4 w-4 mr-1" /> New Template
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Done
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
