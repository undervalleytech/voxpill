use reqwest::blocking::{multipart, Client};
use serde::Deserialize;

const GROQ_STT_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: Option<String>,
    language: Option<String>,
}

fn normalize_model(model: &str) -> Option<&'static str> {
    match model.trim().to_ascii_lowercase().as_str() {
        "whisper-large-v3" => Some("whisper-large-v3"),
        "whisper-large-v3-turbo" => Some("whisper-large-v3-turbo"),
        _ => None,
    }
}

fn pcm16k_mono_to_wav(samples: &[f32]) -> Vec<u8> {
    let sample_rate: u32 = 16_000;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let block_align: u16 = channels * (bits_per_sample / 8);
    let byte_rate: u32 = sample_rate * block_align as u32;

    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for &x in samples {
        let clamped = x.clamp(-1.0, 1.0);
        let v = (clamped * 32767.0).round() as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
    }

    let data_len = pcm.len() as u32;
    let riff_len = 36 + data_len;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&pcm);
    out
}

pub fn transcribe_pcm_16k_mono(
    api_key: &str,
    model: &str,
    samples_16k_mono: &[f32],
    language: &str,
) -> Result<(String, Option<String>), String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Groq STT key missing.".to_string());
    }
    let model = normalize_model(model).ok_or_else(|| {
        "Unsupported Groq STT model. Use whisper-large-v3 or whisper-large-v3-turbo.".to_string()
    })?;
    if samples_16k_mono.is_empty() {
        return Ok((String::new(), None));
    }

    let wav = pcm16k_mono_to_wav(samples_16k_mono);
    let file_part = multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("Groq STT mime error: {e}"))?;

    let mut form = multipart::Form::new().text("model", model.to_string()).part("file", file_part);
    if !language.trim().is_empty() && !language.eq_ignore_ascii_case("auto") {
        form = form.text("language", language.trim().to_ascii_lowercase());
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Groq STT HTTP client error: {e}"))?;

    let resp = client
        .post(GROQ_STT_URL)
        .header("Authorization", format!("Bearer {key}"))
        .multipart(form)
        .send()
        .map_err(|e| format!("Groq STT request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("Groq STT HTTP {status}: {body}"));
    }

    let parsed: GroqTranscriptionResponse = resp
        .json()
        .map_err(|e| format!("Groq STT invalid response: {e}"))?;
    let text = parsed.text.unwrap_or_default().trim().to_string();
    let detected = parsed
        .language
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    Ok((text, detected))
}
