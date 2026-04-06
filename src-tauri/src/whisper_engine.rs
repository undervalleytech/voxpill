use std::path::Path;
use whisper_rs::{
    get_lang_str, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
};

pub fn load_context(model_path: &Path, use_gpu: bool) -> Result<WhisperContext, String> {
    let mut p = WhisperContextParameters::default();
    p.use_gpu = use_gpu;
    p.gpu_device = 0;
    WhisperContext::new_with_params(model_path, p).map_err(|e| e.to_string())
}

/// Retourne le texte transcrit et, si disponible, la langue détectée (ISO) après décodage.
/// Si `translate_to_english` est vrai : tâche *translate* Whisper (sortie en anglais, sans API externe).
pub fn transcribe(
    ctx: &WhisperContext,
    samples: &[f32],
    language: Option<&str>,
    threads: i32,
    translate_to_english: bool,
    decode_profile: &str,
) -> Result<(String, Option<String>), String> {
    let mut state = ctx.create_state().map_err(|e| e.to_string())?;
    let profile = decode_profile.trim().to_ascii_lowercase();
    let (best_of, single_segment, no_context) = match profile.as_str() {
        "fast" => (1, true, true),
        "accurate" => (5, false, false),
        _ => (3, false, false),
    };
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of });
    params.set_n_threads(threads);
    params.set_print_progress(false);
    params.set_translate(translate_to_english);
    params.set_single_segment(single_segment);
    params.set_no_context(no_context);
    // whisper.cpp : si `detect_language == true`, whisper_full retourne tout de suite après
    // whisper_lang_auto_detect (log « auto-detected language ») SANS décoder → 0 segment.
    // Pour transcrire avec langue auto : laisser `detect_language` à false ; la détection
    // se fait quand même (language null / vide / "auto").
    match language {
        None | Some("auto") => {
            params.set_language(None);
            params.set_detect_language(false);
        }
        Some(lang) => {
            params.set_language(Some(lang));
        }
    }
    state.full(params, samples).map_err(|e| e.to_string())?;
    let lang_id = state.full_lang_id_from_state();
    let detected_iso = if lang_id >= 0 {
        get_lang_str(lang_id as i32).map(|s| s.to_string())
    } else {
        None
    };
    let mut out = String::new();
    let n = state.full_n_segments();
    if n <= 0 {
        let rms = if samples.is_empty() {
            0.0
        } else {
            (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
        };
        eprintln!(
            "[Voxpill] Whisper : 0 segment (PCM {} samples @16k, RMS ≈ {:.5})",
            samples.len(),
            rms
        );
    }
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            let s = seg.to_str_lossy().map_err(|e| e.to_string())?;
            out.push_str(&s);
        }
    }
    Ok((out.trim().to_string(), detected_iso))
}
