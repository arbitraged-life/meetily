"use client";

import React, { useState, useEffect, useCallback } from 'react';
import { Switch } from '@/components/ui/switch';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Calendar, Link2, RefreshCw, Plug, PlugZap, Loader2, Radio } from 'lucide-react';
import { load } from '@tauri-apps/plugin-store';
import { useMeetingDetection } from '@/hooks/useMeetingDetection';
import { AutoRecordPrompt } from '@/components/AutoRecordPrompt';

interface CalendarEvent {
  id: string;
  summary: string;
  start_time: string;
  end_time: string;
  meeting_url?: string | null;
  is_online: boolean;
  organizer?: string | null;
  attendees_count: number;
}

interface CalendarAutoRecordConfig {
  only_online_meetings: boolean;
  start_offset_minutes: number;
  stop_offset_minutes: number;
  min_attendees: number;
  skip_keywords: string[];
}

interface CalendarIntegrationStatus {
  is_connected: boolean;
  account_email?: string | null;
  auto_record_enabled: boolean;
  upcoming_events: CalendarEvent[];
  next_event?: CalendarEvent | null;
}

const DEFAULT_AUTO_RECORD_CONFIG: CalendarAutoRecordConfig = {
  only_online_meetings: true,
  start_offset_minutes: 1,
  stop_offset_minutes: 2,
  min_attendees: 2,
  skip_keywords: ['lunch', 'break', 'focus', 'hold'],
};

export function CalendarAutoRecordSettings() {
  // ─── ICS / Outlook ───
  const [icsUrl, setIcsUrl] = useState('');
  const [icsSaving, setIcsSaving] = useState(false);
  const [icsRefreshing, setIcsRefreshing] = useState(false);
  const [icsEvents, setIcsEvents] = useState<CalendarEvent[]>([]);

  // ─── Google Calendar ───
  const [googleClientId, setGoogleClientId] = useState('');
  const [googleClientSecret, setGoogleClientSecret] = useState('');
  const [googleStatus, setGoogleStatus] = useState<CalendarIntegrationStatus | null>(null);
  const [authCode, setAuthCode] = useState('');
  const [googleBusy, setGoogleBusy] = useState(false);
  const [showAuthInput, setShowAuthInput] = useState(false);

  // ─── Auto-record prompt settings ───
  const [promptAutoStart, setPromptAutoStart] = useState(true);
  const [promptAutoStop, setPromptAutoStop] = useState(true);
  const [startCountdownSecs, setStartCountdownSecs] = useState(10);
  const [stopCountdownSecs, setStopCountdownSecs] = useState(10);

  // Load / persist prompt settings via Tauri store
  useEffect(() => {
    (async () => {
      try {
        const store = await load('meetily-prefs.json', { autoSave: true, defaults: {} });
        const ps = await store.get<boolean>('promptAutoStart');
        const pp = await store.get<boolean>('promptAutoStop');
        const sc = await store.get<number>('startCountdownSecs');
        const ec = await store.get<number>('stopCountdownSecs');
        if (ps !== null && ps !== undefined) setPromptAutoStart(ps);
        if (pp !== null && pp !== undefined) setPromptAutoStop(pp);
        if (sc !== null && sc !== undefined) setStartCountdownSecs(sc);
        if (ec !== null && ec !== undefined) setStopCountdownSecs(ec);
      } catch { /* store not yet created */ }
    })();
  }, []);

  const persistPromptSetting = useCallback(async (
    key: string, value: boolean | number,
    setter: (v: any) => void,
  ) => {
    setter(value);
    try {
      const store = await load('meetily-prefs.json', { autoSave: true, defaults: {} });
      await store.set(key, value);
    } catch (e) {
      console.warn('Failed to persist prompt setting:', e);
    }
  }, []);

  // ─── Meeting detection ───
  const {
    isEnabled: detectionEnabled,
    detectedApp,
    pendingStart,
    pendingStop,
    enable: enableDetection,
    disable: disableDetection,
    confirmStart,
    cancelStart,
    confirmStop,
    cancelStop,
  } = useMeetingDetection({
    autoStartRecording: false,
    onMeetingDetected: (app) => toast.info(`Meeting detected: ${app}`),
    onMeetingEnded: (app) => toast.info(`${app} meeting ended`),
  });

  // ─── Calendar auto-record config ───
  const [autoRecordEnabled, setAutoRecordEnabled] = useState(false);
  const [autoConfig, setAutoConfig] = useState<CalendarAutoRecordConfig>(DEFAULT_AUTO_RECORD_CONFIG);
  const [skipKeywordsText, setSkipKeywordsText] = useState(DEFAULT_AUTO_RECORD_CONFIG.skip_keywords.join(', '));

  // ── Load existing state on mount ──
  useEffect(() => {
    (async () => {
      try {
        const events = await invoke<CalendarEvent[]>('get_calendar_events');
        setIcsEvents(events ?? []);
      } catch { /* no ICS configured yet */ }

      try {
        const status = await invoke<CalendarIntegrationStatus>('google_calendar_get_status');
        setGoogleStatus(status);
        setAutoRecordEnabled(status.auto_record_enabled);
      } catch { /* google not initialized */ }

      try {
        const cfg = await invoke<CalendarAutoRecordConfig>('google_calendar_get_config');
        if (cfg) {
          setAutoConfig(cfg);
          setSkipKeywordsText((cfg.skip_keywords ?? []).join(', '));
        }
      } catch { /* not initialized */ }
    })();
  }, []);

  // ─────────── ICS handlers ───────────
  const saveIcsUrl = useCallback(async () => {
    if (!icsUrl.trim()) {
      toast.error('Enter an ICS feed URL first');
      return;
    }
    setIcsSaving(true);
    try {
      await invoke('set_calendar_url', { url: icsUrl.trim() });
      toast.success('Calendar URL saved');
      // Immediately pull events
      setIcsRefreshing(true);
      const count = await invoke<number>('refresh_calendar');
      const events = await invoke<CalendarEvent[]>('get_calendar_events');
      setIcsEvents(events ?? []);
      toast.success(`Fetched ${count} event${count === 1 ? '' : 's'}`);
    } catch (e) {
      toast.error('Failed to save / fetch calendar', { description: String(e) });
    } finally {
      setIcsSaving(false);
      setIcsRefreshing(false);
    }
  }, [icsUrl]);

  const refreshIcs = useCallback(async () => {
    setIcsRefreshing(true);
    try {
      const count = await invoke<number>('refresh_calendar');
      const events = await invoke<CalendarEvent[]>('get_calendar_events');
      setIcsEvents(events ?? []);
      toast.success(`Refreshed — ${count} event${count === 1 ? '' : 's'}`);
    } catch (e) {
      toast.error('Refresh failed', { description: String(e) });
    } finally {
      setIcsRefreshing(false);
    }
  }, []);

  // ─────────── Google handlers ───────────
  const connectGoogle = useCallback(async () => {
    if (!googleClientId.trim() || !googleClientSecret.trim()) {
      toast.error('Enter your Google OAuth Client ID and Secret first');
      return;
    }
    setGoogleBusy(true);
    try {
      await invoke('google_calendar_init', {
        clientId: googleClientId.trim(),
        clientSecret: googleClientSecret.trim(),
      });
      const url = await invoke<string>('google_calendar_get_auth_url');
      window.open(url, '_blank');
      setShowAuthInput(true);
      toast.info('Approve access in your browser, then paste the code below');
    } catch (e) {
      toast.error('Failed to start Google OAuth', { description: String(e) });
    } finally {
      setGoogleBusy(false);
    }
  }, [googleClientId, googleClientSecret]);

  const submitAuthCode = useCallback(async () => {
    if (!authCode.trim()) {
      toast.error('Paste the authorization code');
      return;
    }
    setGoogleBusy(true);
    try {
      await invoke('google_calendar_auth_callback', { code: authCode.trim() });
      const status = await invoke<CalendarIntegrationStatus>('google_calendar_get_status');
      setGoogleStatus(status);
      setShowAuthInput(false);
      setAuthCode('');
      toast.success('Google Calendar connected');
    } catch (e) {
      toast.error('Authorization failed', { description: String(e) });
    } finally {
      setGoogleBusy(false);
    }
  }, [authCode]);

  const disconnectGoogle = useCallback(async () => {
    setGoogleBusy(true);
    try {
      await invoke('google_calendar_disconnect');
      const status = await invoke<CalendarIntegrationStatus>('google_calendar_get_status').catch(() => null);
      setGoogleStatus(status);
      toast.success('Google Calendar disconnected');
    } catch (e) {
      toast.error('Disconnect failed', { description: String(e) });
    } finally {
      setGoogleBusy(false);
    }
  }, []);

  // ─────────── Auto-record config ───────────
  const toggleAutoRecord = useCallback(async (enabled: boolean) => {
    setAutoRecordEnabled(enabled);
    try {
      // Wire both the ICS calendar module and the Google client
      await Promise.allSettled([
        invoke('set_auto_record', { enabled }),
        invoke('google_calendar_set_auto_record', { enabled }),
      ]);
      toast.success(`Calendar auto-record ${enabled ? 'enabled' : 'disabled'}`);
    } catch (e) {
      toast.error('Failed to toggle auto-record', { description: String(e) });
    }
  }, []);

  const persistAutoConfig = useCallback(async (next: CalendarAutoRecordConfig) => {
    setAutoConfig(next);
    try {
      await invoke('google_calendar_set_config', { config: next });
    } catch (e) {
      // Google may not be initialized; non-fatal for ICS-only users
      console.warn('google_calendar_set_config failed:', e);
    }
  }, []);

  const commitSkipKeywords = useCallback(() => {
    const kws = skipKeywordsText
      .split(',')
      .map((k) => k.trim().toLowerCase())
      .filter(Boolean);
    persistAutoConfig({ ...autoConfig, skip_keywords: kws });
  }, [skipKeywordsText, autoConfig, persistAutoConfig]);

  const fmtTime = (iso: string) => {
    try {
      return new Date(iso).toLocaleString(undefined, {
        month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
      });
    } catch { return iso; }
  };

  const inputCls =
    'w-full px-3 py-2 text-sm border border-gray-300 dark:border-gray-700 rounded-md bg-white dark:bg-gray-950 focus:outline-none focus:ring-2 focus:ring-blue-500';

  const isGoogleConnected = !!googleStatus?.is_connected;
  const upcoming = isGoogleConnected ? (googleStatus?.upcoming_events ?? []) : icsEvents;

  return (
    <div className="space-y-8">
      {/* Floating countdown prompt */}
      <AutoRecordPrompt
        pendingStart={pendingStart}
        pendingStop={pendingStop}
        onConfirmStart={confirmStart}
        onCancelStart={cancelStart}
        onConfirmStop={confirmStop}
        onCancelStop={cancelStop}
        startCountdown={startCountdownSecs}
        stopCountdown={stopCountdownSecs}
        autoStart={promptAutoStart}
        autoStop={promptAutoStop}
      />

      <div>
        <h3 className="text-lg font-semibold mb-1 flex items-center gap-2">
          <Calendar className="w-5 h-5" /> Calendar & Auto-Record
        </h3>
        <p className="text-sm text-gray-600 dark:text-gray-300">
          Subscribe to your calendar and let Meetily start recording automatically when meetings begin.
        </p>
      </div>

      {/* ─── Meeting auto-detection ─── */}
      <section className="space-y-3">
        <div className="flex items-center justify-between p-4 border border-gray-200 dark:border-gray-800 rounded-lg bg-white dark:bg-gray-900">
          <div className="flex-1">
            <div className="font-medium flex items-center gap-2">
              <Radio className="w-4 h-4" /> Auto-detect Meetings
            </div>
            <div className="text-sm text-gray-600 dark:text-gray-300">
              Watches for Zoom, Teams, Meet, Webex, Slack, Discord & FaceTime and starts recording on its own.
            </div>
            {detectionEnabled && (
              <div className="text-xs mt-1 text-green-600 dark:text-green-400">
                {detectedApp ? `In a ${detectedApp} meeting` : 'Monitoring active'}
              </div>
            )}
          </div>
          <Switch
            checked={detectionEnabled}
            onCheckedChange={(v) => (v ? enableDetection() : disableDetection())}
          />
        </div>

        {detectionEnabled && (
          <div className="space-y-3 pl-1">
            <div className="flex items-center justify-between p-3 border border-gray-200 dark:border-gray-800 rounded-lg">
              <div className="text-sm">Auto-start recording when meeting detected</div>
              <Switch
                checked={promptAutoStart}
                onCheckedChange={(v) => persistPromptSetting('promptAutoStart', v, setPromptAutoStart)}
              />
            </div>
            <div className="flex items-center justify-between p-3 border border-gray-200 dark:border-gray-800 rounded-lg">
              <div className="text-sm">Auto-stop recording when meeting ends</div>
              <Switch
                checked={promptAutoStop}
                onCheckedChange={(v) => persistPromptSetting('promptAutoStop', v, setPromptAutoStop)}
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <label className="text-sm space-y-1">
                <span className="text-gray-700 dark:text-gray-300">Auto-start countdown (seconds)</span>
                <input
                  type="number"
                  min={3}
                  max={60}
                  value={startCountdownSecs}
                  onChange={(e) =>
                    persistPromptSetting('startCountdownSecs', Math.max(3, parseInt(e.target.value || '10', 10)), setStartCountdownSecs)
                  }
                  className={inputCls}
                />
              </label>
              <label className="text-sm space-y-1">
                <span className="text-gray-700 dark:text-gray-300">Auto-stop countdown (seconds)</span>
                <input
                  type="number"
                  min={3}
                  max={60}
                  value={stopCountdownSecs}
                  onChange={(e) =>
                    persistPromptSetting('stopCountdownSecs', Math.max(3, parseInt(e.target.value || '10', 10)), setStopCountdownSecs)
                  }
                  className={inputCls}
                />
              </label>
            </div>
          </div>
        )}
      </section>

      {/* ─── Outlook / ICS ─── */}
      <section className="space-y-3">
        <h4 className="text-base font-medium flex items-center gap-2">
          <Link2 className="w-4 h-4" /> Outlook / ICS Subscription
        </h4>
        <p className="text-sm text-gray-600 dark:text-gray-300">
          Paste your published calendar feed URL (Outlook: Settings → Calendar → Shared calendars → Publish → ICS link).
        </p>
        <div className="flex gap-2">
          <input
            type="url"
            value={icsUrl}
            onChange={(e) => setIcsUrl(e.target.value)}
            placeholder="https://outlook.office365.com/owa/calendar/.../calendar.ics"
            className={inputCls}
          />
          <button
            onClick={saveIcsUrl}
            disabled={icsSaving}
            className="px-4 py-2 text-sm rounded-md bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50 whitespace-nowrap flex items-center gap-2"
          >
            {icsSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
            Save & Fetch
          </button>
        </div>
        {icsEvents.length > 0 && !isGoogleConnected && (
          <button
            onClick={refreshIcs}
            disabled={icsRefreshing}
            className="flex items-center gap-2 px-3 py-2 text-sm border border-gray-300 dark:border-gray-700 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800 disabled:opacity-50"
          >
            <RefreshCw className={`w-4 h-4 ${icsRefreshing ? 'animate-spin' : ''}`} />
            Refresh events
          </button>
        )}
      </section>

      {/* ─── Google Calendar ─── */}
      <section className="space-y-3">
        <h4 className="text-base font-medium flex items-center gap-2">
          <Calendar className="w-4 h-4" /> Google Calendar
        </h4>

        {isGoogleConnected ? (
          <div className="p-4 border border-green-200 dark:border-green-900/50 rounded-lg bg-green-50 dark:bg-green-900/20 flex items-center justify-between">
            <div className="text-sm text-green-800 dark:text-green-300 flex items-center gap-2">
              <PlugZap className="w-4 h-4" /> Connected
              {googleStatus?.account_email ? ` — ${googleStatus.account_email}` : ''}
            </div>
            <button
              onClick={disconnectGoogle}
              disabled={googleBusy}
              className="px-3 py-1.5 text-sm border border-red-300 dark:border-red-800 text-red-600 dark:text-red-400 rounded-md hover:bg-red-50 dark:hover:bg-red-900/20 disabled:opacity-50"
            >
              Disconnect
            </button>
          </div>
        ) : (
          <div className="space-y-3 p-4 border border-gray-200 dark:border-gray-800 rounded-lg bg-gray-50 dark:bg-gray-900">
            <p className="text-xs text-gray-600 dark:text-gray-400">
              Provide a Google OAuth Desktop Client ID/Secret (Google Cloud Console → APIs & Services → Credentials).
              Redirect URI must include <code className="px-1 bg-gray-200 dark:bg-gray-800 rounded">http://localhost:17249</code>.
            </p>
            <input
              type="text"
              value={googleClientId}
              onChange={(e) => setGoogleClientId(e.target.value)}
              placeholder="Client ID"
              className={inputCls}
            />
            <input
              type="password"
              value={googleClientSecret}
              onChange={(e) => setGoogleClientSecret(e.target.value)}
              placeholder="Client Secret"
              className={inputCls}
            />
            <button
              onClick={connectGoogle}
              disabled={googleBusy}
              className="flex items-center gap-2 px-4 py-2 text-sm rounded-md bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
            >
              {googleBusy ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plug className="w-4 h-4" />}
              Connect Google
            </button>

            {showAuthInput && (
              <div className="flex gap-2 pt-2">
                <input
                  type="text"
                  value={authCode}
                  onChange={(e) => setAuthCode(e.target.value)}
                  placeholder="Paste authorization code"
                  className={inputCls}
                />
                <button
                  onClick={submitAuthCode}
                  disabled={googleBusy}
                  className="px-4 py-2 text-sm rounded-md bg-green-600 text-white hover:bg-green-700 disabled:opacity-50 whitespace-nowrap"
                >
                  Submit
                </button>
              </div>
            )}
          </div>
        )}
      </section>

      {/* ─── Auto-record config ─── */}
      <section className="space-y-4">
        <h4 className="text-base font-medium">Calendar Auto-Record</h4>

        <div className="flex items-center justify-between p-4 border border-gray-200 dark:border-gray-800 rounded-lg bg-white dark:bg-gray-900">
          <div className="flex-1">
            <div className="font-medium">Auto-record from calendar</div>
            <div className="text-sm text-gray-600 dark:text-gray-300">
              Start/stop recording around scheduled events from your subscribed calendars.
            </div>
          </div>
          <Switch checked={autoRecordEnabled} onCheckedChange={toggleAutoRecord} />
        </div>

        {autoRecordEnabled && (
          <div className="space-y-4 pl-1">
            <div className="flex items-center justify-between p-3 border border-gray-200 dark:border-gray-800 rounded-lg">
              <div className="text-sm">Only online meetings (with a video link)</div>
              <Switch
                checked={autoConfig.only_online_meetings}
                onCheckedChange={(v) => persistAutoConfig({ ...autoConfig, only_online_meetings: v })}
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <label className="text-sm space-y-1">
                <span className="text-gray-700 dark:text-gray-300">Start offset (min before)</span>
                <input
                  type="number"
                  value={autoConfig.start_offset_minutes}
                  onChange={(e) =>
                    persistAutoConfig({ ...autoConfig, start_offset_minutes: parseInt(e.target.value || '0', 10) })
                  }
                  className={inputCls}
                />
              </label>
              <label className="text-sm space-y-1">
                <span className="text-gray-700 dark:text-gray-300">Stop offset (min after)</span>
                <input
                  type="number"
                  value={autoConfig.stop_offset_minutes}
                  onChange={(e) =>
                    persistAutoConfig({ ...autoConfig, stop_offset_minutes: parseInt(e.target.value || '0', 10) })
                  }
                  className={inputCls}
                />
              </label>
              <label className="text-sm space-y-1">
                <span className="text-gray-700 dark:text-gray-300">Min attendees</span>
                <input
                  type="number"
                  min={1}
                  value={autoConfig.min_attendees}
                  onChange={(e) =>
                    persistAutoConfig({ ...autoConfig, min_attendees: Math.max(1, parseInt(e.target.value || '1', 10)) })
                  }
                  className={inputCls}
                />
              </label>
            </div>

            <label className="text-sm space-y-1 block">
              <span className="text-gray-700 dark:text-gray-300">Skip keywords (comma-separated)</span>
              <input
                type="text"
                value={skipKeywordsText}
                onChange={(e) => setSkipKeywordsText(e.target.value)}
                onBlur={commitSkipKeywords}
                placeholder="lunch, break, focus, hold"
                className={inputCls}
              />
            </label>
          </div>
        )}
      </section>

      {/* ─── Upcoming events preview ─── */}
      {upcoming.length > 0 && (
        <section className="space-y-2">
          <h4 className="text-base font-medium">Upcoming events</h4>
          <div className="space-y-2">
            {upcoming.slice(0, 6).map((ev) => (
              <div
                key={ev.id}
                className="p-3 border border-gray-200 dark:border-gray-800 rounded-lg bg-white dark:bg-gray-900 flex items-center justify-between"
              >
                <div className="min-w-0">
                  <div className="font-medium truncate">{ev.summary || 'Untitled event'}</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {fmtTime(ev.start_time)}
                    {ev.is_online ? ' · online' : ''}
                    {ev.attendees_count ? ` · ${ev.attendees_count} attendees` : ''}
                  </div>
                </div>
                {ev.meeting_url && (
                  <button
                    onClick={() => window.open(ev.meeting_url!, '_blank')}
                    className="text-xs text-blue-600 dark:text-blue-400 hover:underline whitespace-nowrap ml-3"
                  >
                    Join
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
