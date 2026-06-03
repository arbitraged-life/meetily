//! Auto-select the best microphone by input level (issue #514).
//!
//! Probes every available input device for a short window, measures the mean
//! RMS energy captured from each, and returns the device that picked up the
//! most signal — i.e. the mic the user is actually talking into. This avoids
//! the common failure where recording starts on a silent/wrong input (e.g. a
//! virtual device, or a webcam mic across the room) while the user speaks into
//! their headset.
//!
//! Exposed as a Tauri command (`auto_select_microphone`) the settings/recording
//! UI can call before starting a session, or bind to a "Detect best mic" button.
//!
//! Implementation notes:
//!   - cpal streams are not `Send`, so all probing happens on a dedicated
//!     std::thread and results come back over a channel.
//!   - Each device is sampled sequentially for `PROBE_MS` so we don't fight over
//!     exclusive-access drivers. Total time ≈ devices * PROBE_MS.
//!   - We rank by mean RMS (sustained energy), not peak, so a single click/pop
//!     doesn't win over real speech.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SampleRate, StreamConfig};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long to sample each device, in milliseconds.
const PROBE_MS: u64 = 700;

#[derive(Debug, Clone, Serialize)]
pub struct MicProbeResult {
    pub device_name: String,
    /// Mean RMS energy over the probe window (0.0–1.0).
    pub mean_rms: f32,
    /// Peak sample seen during the probe window (0.0–1.0).
    pub peak: f32,
    /// Number of audio frames observed (0 = device produced no callbacks).
    pub frames: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MicAutoSelectResult {
    /// Name of the winning device, or None if nothing produced signal.
    pub best: Option<String>,
    /// Per-device measurements, sorted loudest-first.
    pub ranked: Vec<MicProbeResult>,
}

/// Accumulates energy from a single device's stream callbacks.
#[derive(Default)]
struct Accum {
    sum_sq: f64,
    samples: u64,
    peak: f32,
    frames: u64,
}

fn probe_one(device: &cpal::Device, name: &str) -> MicProbeResult {
    let acc = Arc::new(Mutex::new(Accum::default()));

    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(_) => {
            return MicProbeResult {
                device_name: name.to_string(),
                mean_rms: 0.0,
                peak: 0.0,
                frames: 0,
            }
        }
    };
    let sample_format = config.sample_format();
    let stream_config = StreamConfig {
        channels: config.channels(),
        sample_rate: SampleRate(config.sample_rate().0),
        buffer_size: cpal::BufferSize::Default,
    };

    let acc_cb = acc.clone();
    let feed = move |data: &[f32]| {
        if let Ok(mut a) = acc_cb.lock() {
            a.frames += 1;
            for &s in data {
                let v = s.abs();
                a.sum_sq += (s as f64) * (s as f64);
                a.samples += 1;
                if v > a.peak {
                    a.peak = v;
                }
            }
        }
    };

    let err_fn = |e| log::debug!("mic probe stream error: {}", e);
    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| feed(data),
            err_fn,
            None,
        ),
        SampleFormat::I16 => {
            let feed = feed.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &_| {
                    let f: Vec<f32> = data.iter().map(|&s| s.to_sample()).collect();
                    feed(&f);
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let feed = feed.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _: &_| {
                    let f: Vec<f32> = data.iter().map(|&s| s.to_sample()).collect();
                    feed(&f);
                },
                err_fn,
                None,
            )
        }
        _ => {
            return MicProbeResult {
                device_name: name.to_string(),
                mean_rms: 0.0,
                peak: 0.0,
                frames: 0,
            }
        }
    };

    let stream = match stream {
        Ok(s) => s,
        Err(_) => {
            return MicProbeResult {
                device_name: name.to_string(),
                mean_rms: 0.0,
                peak: 0.0,
                frames: 0,
            }
        }
    };

    if stream.play().is_ok() {
        std::thread::sleep(Duration::from_millis(PROBE_MS));
    }
    drop(stream);

    let a = acc.lock().map(|g| {
        (
            if g.samples > 0 {
                ((g.sum_sq / g.samples as f64).sqrt() as f32).min(1.0)
            } else {
                0.0
            },
            g.peak.min(1.0),
            g.frames,
        )
    });
    let (mean_rms, peak, frames) = a.unwrap_or((0.0, 0.0, 0));

    MicProbeResult {
        device_name: name.to_string(),
        mean_rms,
        peak,
        frames,
    }
}

/// Probe all input devices and rank them by captured energy. Runs the cpal work
/// on a dedicated thread (streams are `!Send`) and returns the ranked result.
pub fn auto_select() -> MicAutoSelectResult {
    let (tx, rx) = std::sync::mpsc::channel::<MicAutoSelectResult>();

    std::thread::spawn(move || {
        let host = cpal::default_host();
        let mut results: Vec<MicProbeResult> = Vec::new();

        if let Ok(inputs) = host.input_devices() {
            for device in inputs {
                let name = match device.name() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                log::info!("🎙️  Probing input device: {}", name);
                results.push(probe_one(&device, &name));
            }
        }

        // Rank loudest-first by sustained RMS.
        results.sort_by(|a, b| {
            b.mean_rms
                .partial_cmp(&a.mean_rms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // A device only "wins" if it actually captured signal above the noise
        // floor — otherwise we report None and let the UI keep the user's choice.
        const NOISE_FLOOR: f32 = 0.002;
        let best = results
            .iter()
            .find(|r| r.frames > 0 && r.mean_rms > NOISE_FLOOR)
            .map(|r| r.device_name.clone());

        let _ = tx.send(MicAutoSelectResult { best, ranked: results });
    });

    rx.recv().unwrap_or(MicAutoSelectResult {
        best: None,
        ranked: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// Probe all mics and return the best one by input level, plus the full ranking
/// so the UI can show a level meter / let the user override.
#[tauri::command]
pub async fn auto_select_microphone() -> Result<MicAutoSelectResult, String> {
    // Offload to a blocking task — probing takes ~PROBE_MS * device_count.
    tauri::async_runtime::spawn_blocking(auto_select)
        .await
        .map_err(|e| e.to_string())
}
