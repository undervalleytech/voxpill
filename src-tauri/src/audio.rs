use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

fn update_mic_level(mic_level: &Arc<AtomicU32>, data: &[f32]) {
    if data.is_empty() {
        return;
    }
    let peak = data.iter().fold(0.0f32, |a, &x| a.max(x.abs()));
    // Courbe plus sensible aux niveaux faibles (voix normale près du micro).
    let energy = (peak.powf(0.72) * 16.0).clamp(0.0, 1.0);
    let raw = (energy * 1000.0) as u32;
    let prev = mic_level.load(Ordering::Relaxed);
    let decay = prev.saturating_sub(22);
    mic_level.store(raw.max(decay), Ordering::Relaxed);
}

pub struct RecordingSession {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub _channels: u16,
    _stream: cpal::Stream,
}

impl RecordingSession {
    pub fn new(mic_level: Arc<AtomicU32>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "Aucun périphérique d'entrée audio".to_string())?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let sample_format = supported.sample_format();
        let cfg: StreamConfig = supported.config().into();
        let channels = cfg.channels.max(1);
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let samples_cb = Arc::clone(&samples);
        let mic_cb = Arc::clone(&mic_level);
        let ch = channels as usize;
        let stream = match sample_format {
            SampleFormat::F32 => device
                .build_input_stream(
                    &cfg,
                    move |data: &[f32], _| {
                        let mono: Vec<f32> = if ch == 1 {
                            data.to_vec()
                        } else {
                            data.chunks(ch)
                                .map(|chunk| chunk.iter().copied().sum::<f32>() / ch as f32)
                                .collect()
                        };
                        update_mic_level(&mic_cb, &mono);
                        let mut g = samples_cb.lock().unwrap();
                        g.extend_from_slice(&mono);
                    },
                    |e| eprintln!("cpal: {e}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            SampleFormat::I16 => device
                .build_input_stream(
                    &cfg,
                    move |data: &[i16], _| {
                        let mono: Vec<f32> = if ch == 1 {
                            data.iter().map(|&s| s as f32 / 32768.0).collect()
                        } else {
                            data.chunks(ch)
                                .map(|chunk| {
                                    chunk.iter().map(|&x| x as f32).sum::<f32>() / ch as f32
                                        / 32768.0
                                })
                                .collect()
                        };
                        update_mic_level(&mic_cb, &mono);
                        let mut g = samples_cb.lock().unwrap();
                        g.extend_from_slice(&mono);
                    },
                    |e| eprintln!("cpal: {e}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            SampleFormat::U16 => device
                .build_input_stream(
                    &cfg,
                    move |data: &[u16], _| {
                        let mono: Vec<f32> = if ch == 1 {
                            data.iter()
                                .map(|&s| (s as f32 - 32768.0) / 32768.0)
                                .collect()
                        } else {
                            data.chunks(ch)
                                .map(|chunk| {
                                    chunk
                                        .iter()
                                        .map(|&x| (x as f32 - 32768.0) / 32768.0)
                                        .sum::<f32>()
                                        / ch as f32
                                })
                                .collect()
                        };
                        update_mic_level(&mic_cb, &mono);
                        let mut g = samples_cb.lock().unwrap();
                        g.extend_from_slice(&mono);
                    },
                    |e| eprintln!("cpal: {e}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            _ => {
                return Err("Format audio non supporté (F32, I16 ou U16 requis).".to_string());
            }
        };
        stream.play().map_err(|e| e.to_string())?;
        Ok(RecordingSession {
            samples,
            sample_rate: cfg.sample_rate,
            _channels: channels,
            _stream: stream,
        })
    }
}

pub fn resample_to_16k_mono(input: &[f32], in_rate: u32) -> Vec<f32> {
    if in_rate == 16_000 {
        return input.to_vec();
    }
    let ratio = 16_000.0 / in_rate as f64;
    let out_len = ((input.len() as f64) * ratio).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_f = i as f64 / ratio;
        let idx = src_f.floor() as usize;
        let frac = src_f - idx as f64;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push((a as f64 * (1.0 - frac) + b as f64 * frac) as f32);
    }
    out
}
