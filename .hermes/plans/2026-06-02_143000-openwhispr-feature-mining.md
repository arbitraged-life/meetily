# Meetily Feature Opportunities — mined from OpenWhispr open issues

**Date:** 2026-06-02
**Source:** https://github.com/OpenWhispr/openwhispr/issues (open, sorted by updated)
**Scope:** FEATURE IDEAS ONLY. Another Hermes instance is handling PRs — this plan
does **not** open PRs or touch code. Deliverable is a prioritized feature backlog.

---

## Goal

Mine OpenWhispr's open *issues* (competitor STT/dictation app, 3.5k stars) for
feature ideas portable to Meetily, filtering out (a) OpenWhispr-specific bugs,
(b) dictation-only concerns irrelevant to a meeting assistant, and (c) features
Meetily already ships.

## Context / assumptions

- **Meetily** = privacy-first *meeting* assistant: capture (mic + system audio) →
  transcribe (local Whisper / Parakeet) → summarize (Ollama/Claude/Groq/OpenRouter).
  Tauri 2 + Next.js 14 frontend, FastAPI + SQLite backend.
- **OpenWhispr** = *dictation* tool: push-to-talk → Whisper → paste into focused app.
  Different product shape, so dictation-paste/whitespace issues mostly don't port.
- Verified already-present in Meetily fork (do NOT re-propose):
  - System tray + dynamic tray menu (`src-tauri/src/tray.rs`, recording state aware)
  - Audio level metering UI (`frontend/src/components/AudioLevelMeter.tsx`,
    `CompactAudioLevelMeter`, used in `DeviceSelection.tsx`)
  - Multiple ASR engines: Whisper.cpp **and** Parakeet (`parakeet_engine/`)
  - Multi-LLM provider selection (Ollama/Claude/Groq/OpenRouter)
  - GPU auto-detect (Metal/CoreML, CUDA, Vulkan) w/ CPU fallback
- Verified **absent** (genuine gaps): no `global_shortcut` / `GlobalShortcut` /
  `register_shortcut` anywhere in `src-tauri/src` → no global hotkey support.

## Issue → opportunity mapping (filtered)

| OW issue | Theme | Ports to Meetily? | Verdict |
|---|---|---|---|
| #869 global hotkey outside app | Global start/stop hotkey | YES — gap confirmed | **P1** |
| #885 API key wiped on restart | Credential persistence/security | YES — Meetily stores LLM keys | **P1** (harden) |
| #428 hide Dock icon (macOS, 8 comments) | Menu-bar-only mode | YES — has tray, lacks hide-dock | **P2** |
| #514 auto-select best mic by level | Smart device pick | PARTIAL — has meter, not auto-pick | **P2** |
| #707 Homebrew formula (6 comments) | Distribution | YES — only DMG today | **P2** |
| #874 FunASR/SenseVoice ASR engine | Pluggable ASR backend | YES — engine abstraction exists | **P3** |
| #889 "English (American only)" | i18n / UI localization | YES — UI not localized | **P3** |
| #857/#871/#864 audio reliability | Transcription robustness | YES as *features* (retry, empty-audio warn) | **P3** |
| #846 mic loudness icon | Input viz | ALREADY HAVE (AudioLevelMeter) | skip |
| #856 insert whitespace, #858 paste | Dictation-paste | dictation-only | skip |
| #867 Snapdragon NPU, #870 turbo crash | HW-specific bugs | OW build bugs | skip |

## Proposed feature backlog (prioritized)

### P1 — high value, clear gap

1. **Global hotkey for recording control**
   - Register OS-level shortcut(s) to start / stop / pause recording without
     focusing the app. Tauri v2 `tauri-plugin-global-shortcut`.
   - Why: #869 shows strong demand; Meetily users want to trigger capture when a
     meeting starts while in Zoom/Meet. No existing impl.
   - Likely files: `src-tauri/Cargo.toml` (add plugin), `src-tauri/src/lib.rs`
     (register + handler → call existing recording commands in
     `audio/recording_commands.rs`), a settings UI for rebinding shortcut,
     persisted to existing settings store.
   - Wayland/Linux caveat (per #869 COSMIC): document portal limitations.

2. **LLM API-key persistence + secure storage audit**
   - #885 = data-loss bug where keys vanish on restart. Treat as a *hardening
     feature* for Meetily: confirm Claude/Groq/OpenRouter keys persist across
     restarts and are stored in the OS keychain, not plaintext.
   - Likely files: settings persistence layer (frontend settings store +
     `backend/app/db.py` if keys live server-side), consider
     `tauri-plugin-stronghold` or OS keychain via `keyring`.
   - Validation: set key → quit → relaunch → key still present & functional.

### P2 — solid wins

3. **Menu-bar-only mode (hide Dock icon, macOS)** — #428
   - Add a setting to run as accessory app (`NSApplicationActivationPolicy
     .accessory`) so Meetily lives only in the menu bar. Tray already exists.
   - Likely files: `src-tauri/src/lib.rs` / `tray.rs`, `Info.plist`
     (`LSUIElement`), settings toggle.

4. **Auto-select best microphone by input level** — #514
   - On device-selection, sample input RMS across available mics (meter data
     already computed) and pre-select the loudest/active one.
   - Likely files: `audio/devices/discovery.rs`, `DeviceSelection.tsx`,
     reuse `AudioLevelData`/`AudioLevelUpdate` types.

5. **Homebrew cask distribution** — #707
   - Ship a `brew install --cask meetily` path. Cask pointing at the released
     DMG/zip + version automation.
   - Likely files: new tap repo or `Casks/meetily.rb`, release CI to bump sha256.

### P3 — nice-to-have / longer tail

6. **Additional ASR engine option (FunASR / SenseVoice)** — #874
   - Engine abstraction already exists (Whisper + Parakeet). Adding SenseVoice
     (strong multilingual + emotion/event tags) is a differentiator for meetings.
   - Likely files: new `*_engine/` module mirroring `parakeet_engine/`, engine
     selector in settings + tray.

7. **UI localization / non-American English** — #889
   - Whisper already transcribes many languages; gap is *app UI* strings + en-GB
     spelling in summaries. Add i18n scaffolding (next-intl) + locale setting.

8. **Transcription robustness features** — #857/#871/#864
   - Inspired by OW reliability bugs, as proactive Meetily features:
     - Detect empty/too-short audio segments and surface a non-blocking warning.
     - "Re-transcribe segment" action when a chunk looks incomplete
       (OW #857: retranscribe of same audio is perfect → offer manual redo).
   - Likely files: `audio/pipeline.rs` (empty-chunk detection),
     `whisper_engine/whisper_engine.rs`, transcript UI in `frontend/src/app/page.tsx`.

## Files likely to change (by feature) — summary

- Global hotkey: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`,
  `audio/recording_commands.rs`, settings UI.
- Key persistence: settings store, `backend/app/db.py`, keychain plugin.
- Menu-bar mode: `src-tauri/src/lib.rs`, `tray.rs`, `Info.plist`.
- Auto-mic: `audio/devices/discovery.rs`, `DeviceSelection.tsx`.
- Homebrew: external cask/tap + release CI.
- New ASR: new `*_engine/` module + settings/tray.
- i18n: frontend-wide string extraction + locale store.
- Robustness: `audio/pipeline.rs`, `whisper_engine/`, transcript UI.

## Tests / validation

- Hotkey: integration — shortcut fires recording start/stop while app unfocused;
  rebind persists; Linux/Wayland graceful-degrade message shown.
- Key persistence: quit/relaunch round-trip; key never written to plaintext logs.
- Menu-bar mode: toggle hides/shows Dock icon without restart where possible.
- Auto-mic: with 2+ mics, loudest active device is pre-selected.
- New ASR / i18n / robustness: unit + manual transcription smoke tests.

## Risks, tradeoffs, open questions

- **Coordination:** other Hermes instance owns PRs — this stays planning-only to
  avoid branch/PR collisions. Suggest tagging any resulting issues `from:openwhispr`
  and checking open PRs before implementation so the two instances don't both grab
  the same feature.
- **Global hotkey on Wayland** is unreliable (OW #869 proves it) — scope macOS +
  Windows first, Linux best-effort.
- **Keychain migration**: if keys currently live in plaintext settings, need a
  one-time migration path; don't silently drop existing keys.
- **Scope creep on ASR/i18n** — P3 items are larger; treat as separate epics.
- **Open Q:** Does Meetily already persist LLM keys to keychain or plaintext?
  Needs a read of the settings layer before committing P1 #2 effort.
- **Open Q:** Which features (if any) is the PR-running instance already building?
  Reconcile before promoting any of these to implementation.

## Suggested next step

Pick P1 items (global hotkey + key-persistence audit) for the first
implementation pass; convert P2/P3 into tracked issues. Hand off to the
implementation instance only after confirming no overlapping PRs are in flight.
