# Meetily Feature Adoption from OpenWhispr PRs

## Goal

Identify high-value features from OpenWhispr's open PRs that would improve Meetily, prioritized by relevance to a meeting-focused app (vs. a general dictation tool).

## Context

- OpenWhispr: Electron-based dictation/transcription app (3.5k stars), 75 open PRs
- Meetily: Tauri (Rust) + Next.js meeting recorder/transcriber with AI summarization
- Meetily already has: whisper engine, parakeet engine, diarization, dictionary, audio import, Apple Speech, Groq/OpenAI/OpenRouter/Ollama providers, meeting detection, Atoll bridge

## Feature Analysis

### Tier 1 — High Value (directly relevant to meetings)

| # | Feature | OpenWhispr PR | Meetily Relevance | Effort |
|---|---------|---------------|-------------------|--------|
| 1 | **Speaker Diarization Improvements** (sherpa-onnx binary, cloud provider integration) | #754 | Meetily has `diarization/` module already — compare approach. sherpa-onnx may be better than current impl for offline. Cloud diarize via OpenAI `gpt-4o-transcribe-diarize` is free to add. | Medium |
| 2 | **URL Audio Download + Batch Upload** | #754 | Import YouTube/podcast meetings for transcription. Meetily has `audio/import.rs` but unclear if it supports URLs. Batch = process multiple recordings. | Medium |
| 3 | **Smart Spacing Around Pasted Text** | #868 | Applies if Meetily ever does live dictation/paste. Lower priority for a recorder but good UX polish for any text insertion. | Low |
| 4 | **Dictionary Prompt Echo Filter** | #852 | Meetily has `dictionary/` module. If using custom vocabulary prompts, silent audio would echo them. Direct port as a filter util. | Low |
| 5 | **Auto Mute System Audio While Recording** | #778 | Prevents feedback loops during meeting recording if user's speakers are on. macOS: `osascript` mute/unmute. | Low-Med |

### Tier 2 — Medium Value (nice-to-have)

| # | Feature | OpenWhispr PR | Notes |
|---|---------|---------------|-------|
| 6 | **Phrase/Snippet Substitution** | #777 | trigger→snippet post-transcription. Could auto-expand acronyms in meeting notes (e.g., "AR" → "Action Required"). |
| 7 | **Dictionary Replacement Rules** | #758 | Similar to #777 but simpler find/replace. Meetily's `dictionary/` may already cover this. |
| 8 | **OpenRouter as STT Provider** | #786 | Meetily already has `openrouter/` module — may just need STT endpoint added. |
| 9 | **Dedup Duplicate Transcription Completions** | #790 | Realtime transcription dedup by item_id. Relevant if Meetily's streaming transcription duplicates. |
| 10 | **Wrap Transcript in Tags to Deter LLM Answering** | #789 | When feeding transcript to LLM for summary, `<transcription>` tags prevent the model from "answering" questions in the transcript. |

### Tier 3 — Skip (Linux/platform-specific or not relevant)

- #886: Nix flake (Linux packaging)
- #872: KDE Wayland hotkey fix
- #873: Shift+Insert paste for Electron
- #875: Tinfoil private inference (niche provider)
- #850: Vulkan GPU acceleration (AMD/Intel, Linux focus — Meetily targets macOS primarily)
- #685: Fn push-to-talk cancel on keypress
- Dependabot bumps

## Proposed Approach — Top 3 to Implement

### 1. URL Audio Import (from #754)

Meetily already has audio import UI (`ImportAudio/` component). Add:
- URL input field (YouTube, direct audio links)
- yt-dlp sidecar for YouTube downloads
- SSRF protection (HTTPS-only, private IP rejection)
- Progress tracking per-item

**Files likely to change:**
- `frontend/src-tauri/src/audio/import.rs` — add URL download logic
- `frontend/src/components/ImportAudio/` — add URL input UI
- New: `frontend/src-tauri/src/audio/url_downloader.rs`
- `Cargo.toml` — maybe reqwest features

### 2. Auto Mute During Recording (#778)

Simple macOS integration — mute speakers while recording to prevent feedback:

**Files likely to change:**
- `frontend/src-tauri/src/audio/recording_manager.rs` — add mute/unmute hooks on start/stop
- New: `frontend/src-tauri/src/audio/system_mute.rs` — platform mute via osascript
- `frontend/src/app/settings/` — add toggle

### 3. Transcript Tag Wrapping for LLM Summary (#789)

When sending transcript to LLM for summarization, wrap in `<transcription>` tags so the model summarizes rather than answers questions it finds in the transcript.

**Files likely to change:**
- `frontend/src-tauri/src/summary/processor.rs` — wrap transcript in tags before sending to LLM
- Possibly `frontend/src-tauri/src/summary/templates/` — update prompt templates

## Validation

- URL import: test with a public YouTube video, verify transcription output
- Auto mute: verify macOS volume state before/after recording
- Transcript tags: compare summary quality with/without tags on a transcript containing questions

## Risks & Open Questions

1. **yt-dlp distribution** — Bundling as sidecar vs. expecting user install? OpenWhispr uses a download script. Meetily could use Homebrew check or auto-download.
2. **Auto mute crash safety** — If Meetily crashes mid-recording, system stays muted. Need a watchdog or startup check that unmutes if last session didn't cleanly stop.
3. **Diarization comparison** — Need to check what Meetily's existing `diarization/` module does vs. OpenWhispr's sherpa-onnx approach. May already be better.
4. **OpenRouter STT** — Is Meetily's OpenRouter module already doing STT or just LLM? Worth checking before treating as new work.
