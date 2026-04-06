mod audio;
mod audio_decode;
mod foreground;
mod groq_stt;
mod hardware;
mod model;
mod reformulate;
mod translate;
mod whisper_engine;

use crate::model::ModelTier;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::Color;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard::Clipboard;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use whisper_rs::WhisperContext;

use crate::audio::{resample_to_16k_mono, RecordingSession};

const PALETTE_W: f64 = 880.0;
const PALETTE_H: f64 = 720.0;
const HUD_W: f64 = 320.0;
/// Marge au-dessus de la capsule pour le halo / flou (mode assistant IA) — aligné avec `tauri.conf.json` → rec-hud.
const HUD_H: f64 = 120.0;
/// Large assez pour deux menus + marge halo ; aligné avec `tauri.conf.json` → file-pill.
const FILE_PILL_W: f64 = 500.0;
/// Hauteur : zone flexible au-dessus (listes vers le haut, toast, glow) + capsule en bas.
const FILE_PILL_H: f64 = 360.0;
/// Décodage Whisper pour la transcription fichier (indépendant du profil actif).
const FILE_TRANSCRIBE_DECODE_PROFILE: &str = "balanced";
const PROFILE_PILL_W: f64 = 340.0;
const PROFILE_PILL_H: f64 = 52.0;
/// Limite de caractères pour le texte sélectionné envoyé au LLM (mode assist).
const AI_ASSIST_SELECTED_MAX_CHARS: usize = 48_000;
/// Pause après `SetForegroundWindow` avant la simulation Ctrl+C (stabilisation du focus).
const AI_ASSIST_FOCUS_SETTLE_MS: u64 = 100;
/// Pause après Ctrl+C simulé avant lecture du presse-papiers.
const AI_ASSIST_POST_COPY_DELAY_MS: u64 = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum InsertMode {
    Paste,
    Clipboard,
}

fn default_translate_target() -> String {
    "en".to_string()
}

fn default_decode_profile() -> String {
    "balanced".to_string()
}

fn default_ptt_shortcut() -> String {
    "Ctrl+Space".to_string()
}

fn default_shortcut_transcribe_file() -> String {
    "Ctrl+Shift+T".to_string()
}

fn default_shortcut_ai_assist() -> String {
    "Ctrl+Alt+Space".to_string()
}

fn default_shortcut_cycle_profile() -> String {
    "Ctrl+Alt+Shift+P".to_string()
}

fn default_reformulation_provider() -> String {
    "groq".to_string()
}

fn default_stt_mode() -> String {
    "local".to_string()
}

fn default_stt_cloud_model() -> String {
    "whisper-large-v3-turbo".to_string()
}

fn new_profile_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_reformulation_profiles() -> Vec<ReformulationProfile> {
    vec![
        ReformulationProfile {
            id: new_profile_id(),
            profile_name: "Profile 1".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
            prompt: "Rewrite the dictated text for clarity. Preserve meaning and language; do not add facts or code. Output only the rewritten text.".to_string(),
            decode_profile: default_decode_profile(),
            translate_enabled: false,
            translate_target: default_translate_target(),
            reformulation_enabled: false,
        },
        ReformulationProfile {
            id: new_profile_id(),
            profile_name: "Profile 2".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
            prompt: "Rewrite for a coding-assistant prompt: concise, actionable, preserve technical terms and intent. Output only the rewritten text.".to_string(),
            decode_profile: default_decode_profile(),
            translate_enabled: false,
            translate_target: default_translate_target(),
            reformulation_enabled: false,
        },
        ReformulationProfile {
            id: new_profile_id(),
            profile_name: "Profile 3".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
            prompt: "Rewrite briefly for professional messaging. Preserve meaning. Output only the rewritten text.".to_string(),
            decode_profile: default_decode_profile(),
            translate_enabled: false,
            translate_target: default_translate_target(),
            reformulation_enabled: false,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReformulationProfile {
    /// Identifiant stable (UUID) — ordre du `Vec` peut changer sans perdre le preset actif.
    #[serde(default)]
    id: String,
    #[serde(default)]
    profile_name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    prompt: String,
    /// Preset Whisper decode (fast / balanced / accurate) pour ce profil.
    #[serde(default = "default_decode_profile")]
    decode_profile: String,
    #[serde(default)]
    translate_enabled: bool,
    #[serde(default = "default_translate_target")]
    translate_target: String,
    /// Reformulation LLM activée pour ce profil (clé API globale).
    #[serde(default)]
    reformulation_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
    language: String,
    insert_mode: InsertMode,
    model_tier: String,
    #[serde(default = "default_decode_profile")]
    decode_profile: String,
    #[serde(default = "default_ptt_shortcut")]
    ptt_shortcut: String,
    #[serde(default = "default_shortcut_transcribe_file")]
    shortcut_transcribe_file: String,
    #[serde(default = "default_shortcut_ai_assist")]
    shortcut_ai_assist: String,
    #[serde(default = "default_shortcut_cycle_profile")]
    shortcut_cycle_profile: String,
    #[serde(default = "default_status_sounds_enabled")]
    status_sounds_enabled: bool,
    /// Traduire le texte après Whisper (MyMemory, en ligne).
    #[serde(default)]
    translate_enabled: bool,
    /// Code langue ISO cible (ex. en, fr, de).
    #[serde(default = "default_translate_target")]
    translate_target: String,
    /// Reformulation IA (Groq / OpenRouter) — optionnel, BYOK.
    #[serde(default)]
    reformulation_enabled: bool,
    #[serde(default)]
    reformulation_api_key: String,
    #[serde(default = "default_reformulation_provider")]
    reformulation_provider: String,
    /// Profil actif par id (source de vérité).
    #[serde(default)]
    reformulation_active_profile_id: String,
    /// Ancien format : index 1-based. Migré vers `reformulation_active_profile_id` puis omis à la sauvegarde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reformulation_active_profile: Option<u8>,
    #[serde(default = "default_reformulation_profiles")]
    reformulation_profiles: Vec<ReformulationProfile>,
    /// Modèle API pour le mode assistant vocal uniquement ; vide = même modèle que le profil actif.
    #[serde(default)]
    reformulation_assist_model: String,
    /// Moteur STT : local Whisper ou cloud Groq.
    #[serde(default = "default_stt_mode")]
    stt_mode: String,
    /// Modèle STT Groq (utilisé quand `stt_mode=cloud-groq`).
    #[serde(default = "default_stt_cloud_model")]
    stt_cloud_model: String,
    /// Clé API Groq dédiée au STT cloud.
    #[serde(default)]
    stt_groq_api_key: String,
    /// Migration one-shot : copier decode/traduction/reformulation globaux dans chaque profil.
    #[serde(default)]
    pipeline_profiles_migrated: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let reformulation_profiles = default_reformulation_profiles();
        let reformulation_active_profile_id = reformulation_profiles
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        Self {
            language: "fr".to_string(),
            insert_mode: InsertMode::Paste,
            model_tier: "tiny".to_string(),
            decode_profile: default_decode_profile(),
            ptt_shortcut: default_ptt_shortcut(),
            shortcut_transcribe_file: default_shortcut_transcribe_file(),
            shortcut_ai_assist: default_shortcut_ai_assist(),
            shortcut_cycle_profile: default_shortcut_cycle_profile(),
            status_sounds_enabled: default_status_sounds_enabled(),
            translate_enabled: false,
            translate_target: default_translate_target(),
            reformulation_enabled: false,
            reformulation_api_key: String::new(),
            reformulation_provider: default_reformulation_provider(),
            reformulation_active_profile_id,
            reformulation_active_profile: None,
            reformulation_profiles,
            reformulation_assist_model: String::new(),
            stt_mode: default_stt_mode(),
            stt_cloud_model: default_stt_cloud_model(),
            stt_groq_api_key: String::new(),
            pipeline_profiles_migrated: false,
        }
    }
}

fn default_status_sounds_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordingMode {
    Normal,
    /// Dictée puis réponse LLM à la consigne (même clé / modèle que le profil actif).
    AiAssist,
}

pub(crate) struct Managed {
    pub(crate) config: Mutex<AppConfig>,
    pub(crate) recording: Mutex<Option<RecordingSession>>,
    pub(crate) whisper: Mutex<Option<WhisperContext>>,
    /// Dernier modèle chargé : chemin + indicateur GPU utilisé pour ce chargement.
    pub(crate) loaded_model_key: Mutex<Option<(PathBuf, bool)>>,
    /// Niveau micro 0–1000 pour le HUD (ondes).
    pub(crate) mic_level: Arc<AtomicU32>,
    pub(crate) recording_active: Arc<AtomicBool>,
    /// HWND Windows (handle) de la fenêtre active au début de la dictée — pour renvoyer le focus avant Ctrl+V.
    pub(crate) paste_target_hwnd: Mutex<Option<usize>>,
    /// Mode de la session d’enregistrement en cours (PTT classique vs assistant vocal).
    pub(crate) recording_mode: Mutex<RecordingMode>,
    /// Texte issu de la sélection (Ctrl+C simulé au début d’une session assist), consommé au stop.
    pub(crate) ai_assist_selected_text: Mutex<Option<String>>,
    /// Zone interactive file-pill (CSS px relatifs au client webview) pour clic à travers sous Windows.
    pub(crate) file_pill_interact_rect: Mutex<Option<(f64, f64, f64, f64)>>,
    /// Zone interactive rec-hud : uniquement la capsule `#hud-pill` (pas le spacer / halo).
    pub(crate) rec_hud_interact_rect: Mutex<Option<(f64, f64, f64, f64)>>,
    /// Demande d’annulation transcription fichier (lu dans `do_transcribe_file`).
    pub(crate) file_transcribe_cancel: Arc<AtomicBool>,
    /// Transcription / collage en cours sur un worker (évite double dictée et débloque le thread raccourci).
    pub(crate) transcription_pipeline_busy: Arc<AtomicBool>,
}

fn config_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(base.join("settings.json"))
}

fn assign_reformulation_profile_ids(profiles: &mut [ReformulationProfile]) {
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in profiles.iter_mut() {
        if p.id.trim().is_empty() {
            p.id = new_profile_id();
        }
        while used.contains(&p.id) || p.id.trim().is_empty() {
            p.id = new_profile_id();
        }
        used.insert(p.id.clone());
    }
}

fn active_profile_index(cfg: &AppConfig) -> Option<usize> {
    let id = cfg.reformulation_active_profile_id.trim();
    if id.is_empty() {
        return None;
    }
    cfg.reformulation_profiles.iter().position(|p| p.id == id)
}

/// Remplit les ids manquants, résout `reformulation_active_profile_id` (legacy index ou premier profil), puis efface l’index legacy.
fn normalize_shortcut_token(s: &str, default: &str) -> String {
    let t = s.trim();
    if t.is_empty() || t.len() > 64 {
        default.to_string()
    } else {
        t.to_string()
    }
}

fn pick_distinct_shortcut(
    preferred: String,
    default: &str,
    fallbacks: &[&str],
    used: &mut Vec<String>,
) -> String {
    for cand in std::iter::once(preferred)
        .chain(std::iter::once(default.to_string()))
        .chain(fallbacks.iter().map(|s| s.to_string()))
    {
        if !used.iter().any(|u| u == &cand) {
            used.push(cand.clone());
            return cand;
        }
    }
    let c = format!("Ctrl+Shift+{}", used.len().saturating_add(9));
    used.push(c.clone());
    c
}

/// Raccourcis globaux distincts (priorité : PTT, fichier, assist, cycle profil).
fn ensure_distinct_global_shortcuts(cfg: &mut AppConfig) {
    let d_ptt = default_ptt_shortcut();
    let d_file = default_shortcut_transcribe_file();
    let d_assist = default_shortcut_ai_assist();
    let d_cycle = default_shortcut_cycle_profile();
    let fb_file = ["Ctrl+Shift+Y", "Ctrl+Shift+U", "Ctrl+Shift+I"];
    let fb_assist = ["Ctrl+Alt+Shift+Space", "Ctrl+Shift+Alt+N", "Ctrl+Alt+Shift+N"];
    let fb_cycle = ["Ctrl+Alt+Shift+O", "Ctrl+Alt+Shift+I", "Ctrl+Alt+Shift+K"];

    let ptt = normalize_shortcut_token(&cfg.ptt_shortcut, &d_ptt);
    let mut used = vec![ptt.clone()];
    cfg.ptt_shortcut = ptt;

    let file_raw = normalize_shortcut_token(&cfg.shortcut_transcribe_file, &d_file);
    cfg.shortcut_transcribe_file = pick_distinct_shortcut(file_raw, &d_file, &fb_file, &mut used);

    let assist_raw = normalize_shortcut_token(&cfg.shortcut_ai_assist, &d_assist);
    cfg.shortcut_ai_assist = pick_distinct_shortcut(assist_raw, &d_assist, &fb_assist, &mut used);

    let cycle_raw = normalize_shortcut_token(&cfg.shortcut_cycle_profile, &d_cycle);
    cfg.shortcut_cycle_profile = pick_distinct_shortcut(cycle_raw, &d_cycle, &fb_cycle, &mut used);
}

fn resolve_reformulation_active_profile(cfg: &mut AppConfig) {
    let n = cfg.reformulation_profiles.len();
    if n == 0 {
        return;
    }
    assign_reformulation_profile_ids(&mut cfg.reformulation_profiles);

    let mut active_id = cfg.reformulation_active_profile_id.trim().to_string();
    if active_id.is_empty() {
        if let Some(k) = cfg.reformulation_active_profile {
            let k = k as usize;
            if k >= 1 && k <= n {
                active_id = cfg.reformulation_profiles[k - 1].id.clone();
            }
        }
        if active_id.is_empty() {
            active_id = cfg.reformulation_profiles[0].id.clone();
        }
    } else if !cfg
        .reformulation_profiles
        .iter()
        .any(|p| p.id == active_id)
    {
        active_id = cfg.reformulation_profiles[0].id.clone();
    }
    cfg.reformulation_active_profile_id = active_id;
    cfg.reformulation_active_profile = None;
}

fn normalize_config(cfg: &mut AppConfig) {
    cfg.model_tier = cfg
        .model_tier
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-");
    if !matches!(
        cfg.model_tier.as_str(),
        "tiny" | "base" | "small" | "medium" | "large-v3-turbo"
    ) {
        cfg.model_tier = "tiny".to_string();
    }
    cfg.translate_target = cfg.translate_target.trim().to_ascii_lowercase();
    if cfg.translate_target.is_empty() {
        cfg.translate_target = default_translate_target();
    }
    cfg.decode_profile = cfg.decode_profile.trim().to_ascii_lowercase();
    if !matches!(cfg.decode_profile.as_str(), "fast" | "balanced" | "accurate") {
        cfg.decode_profile = default_decode_profile();
    }
    ensure_distinct_global_shortcuts(cfg);
    cfg.reformulation_provider = cfg.reformulation_provider.trim().to_ascii_lowercase();
    if !matches!(cfg.reformulation_provider.as_str(), "groq" | "openrouter") {
        cfg.reformulation_provider = default_reformulation_provider();
    }
    if cfg.reformulation_profiles.is_empty() {
        cfg.reformulation_profiles = default_reformulation_profiles();
    }
    resolve_reformulation_active_profile(cfg);
    if !cfg.pipeline_profiles_migrated {
        let g_dec = cfg.decode_profile.clone();
        let g_tr = cfg.translate_enabled;
        let g_tt = cfg.translate_target.clone();
        let g_re = cfg.reformulation_enabled;
        for p in &mut cfg.reformulation_profiles {
            p.decode_profile = g_dec.clone();
            p.translate_enabled = g_tr;
            p.translate_target = g_tt.clone();
            p.reformulation_enabled = g_re;
        }
        cfg.pipeline_profiles_migrated = true;
    }
    for (i, p) in cfg.reformulation_profiles.iter_mut().enumerate() {
        if p.profile_name.trim().is_empty() {
            p.profile_name = format!("Profile {}", i + 1);
        } else {
            p.profile_name = p.profile_name.trim().to_string();
        }
        p.decode_profile = p.decode_profile.trim().to_ascii_lowercase();
        if !matches!(p.decode_profile.as_str(), "fast" | "balanced" | "accurate") {
            p.decode_profile = default_decode_profile();
        }
        p.translate_target = p.translate_target.trim().to_ascii_lowercase();
        if p.translate_target.is_empty() {
            p.translate_target = default_translate_target();
        }
    }
    cfg.reformulation_assist_model = cfg.reformulation_assist_model.trim().to_string();
    cfg.stt_mode = cfg.stt_mode.trim().to_ascii_lowercase();
    if !matches!(cfg.stt_mode.as_str(), "local" | "cloud-groq") {
        cfg.stt_mode = default_stt_mode();
    }
    cfg.stt_cloud_model = cfg.stt_cloud_model.trim().to_ascii_lowercase();
    if !matches!(
        cfg.stt_cloud_model.as_str(),
        "whisper-large-v3" | "whisper-large-v3-turbo"
    ) {
        cfg.stt_cloud_model = default_stt_cloud_model();
    }
    cfg.stt_groq_api_key = cfg.stt_groq_api_key.trim().to_string();
    sync_globals_from_active_profile(cfg);
}

/// Aligne les champs globaux (compat UI / ancien code) sur le profil actif.
fn sync_globals_from_active_profile(cfg: &mut AppConfig) {
    let Some(idx) = active_profile_index(cfg) else {
        return;
    };
    if let Some(p) = cfg.reformulation_profiles.get(idx) {
        cfg.decode_profile = p.decode_profile.clone();
        cfg.translate_enabled = p.translate_enabled;
        cfg.translate_target = p.translate_target.clone();
        cfg.reformulation_enabled = p.reformulation_enabled;
    }
}

/// Décodage Whisper, traduction : valeurs du profil actif (fallback sur champs globaux).
fn active_profile_pipeline(cfg: &AppConfig) -> (String, bool, String) {
    let idx = active_profile_index(cfg).unwrap_or(0);
    cfg.reformulation_profiles
        .get(idx)
        .map(|p| {
            (
                p.decode_profile.clone(),
                p.translate_enabled,
                p.translate_target.clone(),
            )
        })
        .unwrap_or_else(|| {
            (
                cfg.decode_profile.clone(),
                cfg.translate_enabled,
                cfg.translate_target.clone(),
            )
        })
}

fn reformulation_params_from_cfg(cfg: &AppConfig) -> reformulate::ReformulationParams {
    let idx = active_profile_index(cfg).unwrap_or(0);
    let profile_reformulation_on = cfg
        .reformulation_profiles
        .get(idx)
        .map(|p| p.reformulation_enabled)
        .unwrap_or(false);
    let one_based = (idx.saturating_add(1)).min(u8::MAX as usize) as u8;
    reformulate::ReformulationParams {
        enabled: profile_reformulation_on,
        api_key: cfg.reformulation_api_key.clone(),
        provider: cfg.reformulation_provider.clone(),
        active_profile: one_based,
        profiles: cfg
            .reformulation_profiles
            .iter()
            .map(|p| reformulate::ReformulationProfileSnap {
                model: p.model.clone(),
                prompt: p.prompt.clone(),
            })
            .collect(),
        assist_model_override: cfg.reformulation_assist_model.clone(),
    }
}

/// Si la config pointe vers un tier réservé GPU alors que Whisper n’utilise pas le GPU, repasser sur `small` et persister.
fn downgrade_gpu_only_if_unavailable(cfg: &mut AppConfig) {
    let t = ModelTier::from_suggested_tier(&cfg.model_tier);
    if t.gpu_only() && !whisper_should_use_gpu() {
        cfg.model_tier = "small".to_string();
    }
}

fn load_cfg(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let p = config_file(app)?;
    if !p.exists() {
        let mut c = AppConfig::default();
        normalize_config(&mut c);
        downgrade_gpu_only_if_unavailable(&mut c);
        return Ok(c);
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let mut cfg: AppConfig = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    normalize_config(&mut cfg);
    let before = cfg.model_tier.clone();
    downgrade_gpu_only_if_unavailable(&mut cfg);
    if cfg.model_tier != before {
        save_cfg(app, &cfg)?;
    }
    Ok(cfg)
}

fn save_cfg(app: &tauri::AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let p = config_file(app)?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn position_file_pill(w: &tauri::WebviewWindow) -> Result<(), String> {
    if let Some(mon) = w.current_monitor().map_err(|e| e.to_string())? {
        let pos = mon.position();
        let size = mon.size();
        let scale = mon.scale_factor();
        let phys_w = (FILE_PILL_W * scale).round() as i32;
        let phys_h = (FILE_PILL_H * scale).round() as i32;
        let _ = w.set_size(tauri::PhysicalSize::new(phys_w as u32, phys_h as u32));
        let x = pos.x + (size.width as i32 - phys_w) / 2;
        let y = pos.y + size.height as i32 - phys_h - (56.0 * scale).round() as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
    Ok(())
}

fn show_file_pill(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("file-pill") {
        position_file_pill(&w)?;
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

fn toggle_file_pill(app: &tauri::AppHandle, managed: &Arc<Managed>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("file-pill") {
        if w.is_visible().unwrap_or(false) {
            managed
                .file_transcribe_cancel
                .store(true, Ordering::Release);
            let _ = w.hide();
        } else {
            position_file_pill(&w)?;
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    Ok(())
}

fn emit_file_pill(app: &tauri::AppHandle, status: &str, message: &str) {
    if let Some(w) = app.get_webview_window("file-pill") {
        let _ = w.emit(
            "file-transcribe-status",
            serde_json::json!({ "status": status, "message": message }),
        );
    }
}

fn check_file_transcribe_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Transcription annulée.".to_string())
    } else {
        Ok(())
    }
}

/// Sous Windows : ignore les clics hors de la zone rapportée par la webview (WebView2 ne respecte pas `pointer-events` pour le trouer).
#[cfg(windows)]
fn file_pill_update_cursor_pass_through(app: &tauri::AppHandle, managed: &Arc<Managed>) {
    use tauri::Manager;
    use winapi::shared::windef::{HWND, POINT};
    use winapi::um::winuser::{GetCursorPos, ScreenToClient};

    let Some(w) = app.get_webview_window("file-pill") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        let _ = w.set_ignore_cursor_events(false);
        return;
    }
    let Ok(hwnd_t) = w.hwnd() else {
        return;
    };
    let hwnd = hwnd_t.0 as HWND;
    let rect = match managed.file_pill_interact_rect.lock() {
        Ok(g) => *g,
        Err(_) => return,
    };
    let Some((ix, iy, iw, ih)) = rect else {
        let _ = w.set_ignore_cursor_events(false);
        return;
    };
    let Ok(scale) = w.scale_factor() else {
        let _ = w.set_ignore_cursor_events(false);
        return;
    };
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut pt) == 0 {
            return;
        }
        if ScreenToClient(hwnd, &mut pt) == 0 {
            return;
        }
    }
    let left = (ix * scale).round() as i32;
    let top = (iy * scale).round() as i32;
    let right = ((ix + iw) * scale).round() as i32;
    let bottom = ((iy + ih) * scale).round() as i32;
    let inside = pt.x >= left && pt.x < right && pt.y >= top && pt.y < bottom;
    let _ = w.set_ignore_cursor_events(!inside);
}

#[cfg(not(windows))]
fn file_pill_update_cursor_pass_through(_app: &tauri::AppHandle, _managed: &Arc<Managed>) {}

/// Clic à travers hors de la capsule dictée (même principe que file-pill).
#[cfg(windows)]
fn rec_hud_update_cursor_pass_through(app: &tauri::AppHandle, managed: &Arc<Managed>) {
    use tauri::Manager;
    use winapi::shared::windef::{HWND, POINT};
    use winapi::um::winuser::{GetCursorPos, ScreenToClient};

    let Some(w) = app.get_webview_window("rec-hud") else {
        return;
    };
    if !w.is_visible().unwrap_or(false) {
        let _ = w.set_ignore_cursor_events(false);
        return;
    }
    let Ok(hwnd_t) = w.hwnd() else {
        return;
    };
    let hwnd = hwnd_t.0 as HWND;
    let rect = match managed.rec_hud_interact_rect.lock() {
        Ok(g) => *g,
        Err(_) => return,
    };
    // Tant que la webview n’a pas encore envoyé les dimensions : tout laisser traverser.
    let Some((ix, iy, iw, ih)) = rect else {
        let _ = w.set_ignore_cursor_events(true);
        return;
    };
    let Ok(scale) = w.scale_factor() else {
        let _ = w.set_ignore_cursor_events(true);
        return;
    };
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut pt) == 0 {
            return;
        }
        if ScreenToClient(hwnd, &mut pt) == 0 {
            return;
        }
    }
    let left = (ix * scale).round() as i32;
    let top = (iy * scale).round() as i32;
    let right = ((ix + iw) * scale).round() as i32;
    let bottom = ((iy + ih) * scale).round() as i32;
    let inside = pt.x >= left && pt.x < right && pt.y >= top && pt.y < bottom;
    let _ = w.set_ignore_cursor_events(!inside);
}

#[cfg(not(windows))]
fn rec_hud_update_cursor_pass_through(_app: &tauri::AppHandle, _managed: &Arc<Managed>) {}

fn position_rec_hud(w: &tauri::WebviewWindow) -> Result<(), String> {
    if let Some(mon) = w.current_monitor().map_err(|e| e.to_string())? {
        let pos = mon.position();
        let size = mon.size();
        let scale = mon.scale_factor();
        let phys_w = (HUD_W * scale).round() as i32;
        let phys_h = (HUD_H * scale).round() as i32;
        let _ = w.set_size(tauri::PhysicalSize::new(phys_w as u32, phys_h as u32));
        let x = pos.x + (size.width as i32 - phys_w) / 2;
        let y = pos.y + size.height as i32 - phys_h - (56.0 * scale).round() as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
    Ok(())
}

fn show_rec_hud(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("rec-hud") {
        position_rec_hud(&w)?;
        let _ = w.show();
    }
    Ok(())
}

fn hide_rec_hud(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("rec-hud") {
        let _ = w.hide();
    }
}

fn position_profile_pill(w: &tauri::WebviewWindow) -> Result<(), String> {
    if let Some(mon) = w.current_monitor().map_err(|e| e.to_string())? {
        let pos = mon.position();
        let size = mon.size();
        let scale = mon.scale_factor();
        let phys_w = (PROFILE_PILL_W * scale).round() as i32;
        let phys_h = (PROFILE_PILL_H * scale).round() as i32;
        let _ = w.set_size(tauri::PhysicalSize::new(phys_w as u32, phys_h as u32));
        let x = pos.x + (size.width as i32 - phys_w) / 2;
        let y = pos.y + size.height as i32 - phys_h - (56.0 * scale).round() as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
    Ok(())
}

fn show_profile_pill(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("profile-pill") {
        position_profile_pill(&w)?;
        let _ = w.show();
    }
    Ok(())
}

/// Raccourci global `Ctrl+Alt+Shift+P` : cycle le profil actif **1 → … → n → 1** (pipeline par profil).
fn cycle_reformulation_profile(app: &tauri::AppHandle, managed: &Arc<Managed>) -> Result<(), String> {
    let (profile_index, profile_id, name, anim_forward, reformulation_enabled) = {
        let mut cfg = managed.config.lock().map_err(|e| e.to_string())?;
        let n = cfg.reformulation_profiles.len();
        if n == 0 {
            return Ok(());
        }

        let cur_idx = cfg
            .reformulation_profiles
            .iter()
            .position(|p| p.id == cfg.reformulation_active_profile_id)
            .unwrap_or(0);
        let next_idx = (cur_idx + 1) % n;
        cfg.reformulation_active_profile_id = cfg.reformulation_profiles[next_idx].id.clone();
        sync_globals_from_active_profile(&mut cfg);
        let name = cfg.reformulation_profiles[next_idx].profile_name.clone();
        let profile_id = cfg.reformulation_profiles[next_idx].id.clone();
        let reformulation_enabled = cfg.reformulation_profiles[next_idx].reformulation_enabled;
        save_cfg(app, &cfg)?;
        let next_1based = next_idx + 1;
        let anim_forward = (next_1based % 2) == 1;
        (next_1based, profile_id, name, anim_forward, reformulation_enabled)
    };
    let payload = serde_json::json!({
        "profile_index": profile_index,
        "profile_id": profile_id,
        "profile_name": name,
        "anim_forward": anim_forward,
        "reformulation_enabled": reformulation_enabled,
    });
    let app_ui = app.clone();
    std::thread::spawn(move || {
        emit_profile_cycle_ui_on_main(&app_ui, payload);
    });
    Ok(())
}

/// Notifie le HUD dictée (`rec-hud`) : `recording` | `transcribing` | `reformulating` | `idle`.
fn emit_rec_hud_phase(app: &tauri::AppHandle, phase: &str, mode: RecordingMode) {
    if let Some(w) = app.get_webview_window("rec-hud") {
        let mode_str = match mode {
            RecordingMode::Normal => "normal",
            RecordingMode::AiAssist => "ai_assist",
        };
        let _ = w.emit(
            "hud-phase",
            serde_json::json!({ "phase": phase, "mode": mode_str }),
        );
    }
}

fn emit_rec_hud_sound_enabled(app: &tauri::AppHandle, enabled: bool) {
    if let Some(w) = app.get_webview_window("rec-hud") {
        let _ = w.emit("hud-sound-enabled", serde_json::json!({ "enabled": enabled }));
    }
}

/// HUD / webview depuis un worker : planifie sur le thread principal Tauri et attend.
fn hide_rec_hud_on_main(app: &tauri::AppHandle) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let outer = app.clone();
    let inner = outer.clone();
    match outer.run_on_main_thread(move || {
        hide_rec_hud(&inner);
        let _ = tx.send(());
    }) {
        Ok(()) => {
            let _ = rx.recv();
        }
        Err(_) => {
            hide_rec_hud(app);
        }
    }
}

fn emit_rec_hud_phase_on_main(app: &tauri::AppHandle, phase: &str, mode: RecordingMode) {
    let phase_owned = phase.to_string();
    let phase_fallback = phase_owned.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let outer = app.clone();
    let inner = outer.clone();
    match outer.run_on_main_thread(move || {
        emit_rec_hud_phase(&inner, &phase_owned, mode);
        let _ = tx.send(());
    }) {
        Ok(()) => {
            let _ = rx.recv();
        }
        Err(_) => {
            emit_rec_hud_phase(app, &phase_fallback, mode);
        }
    }
}

fn emit_transcript_on_main(app: &tauri::AppHandle, payload: serde_json::Value) {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let outer = app.clone();
    let inner = outer.clone();
    let fallback = payload.clone();
    match outer.run_on_main_thread(move || {
        let _ = inner.emit("transcript", payload);
        let _ = tx.send(());
    }) {
        Ok(()) => {
            let _ = rx.recv();
        }
        Err(_) => {
            let _ = app.emit("transcript", fallback);
        }
    }
}

/// Émissions + pill profil sur le thread principal (raccourci cycle profil ne bloque pas).
fn emit_profile_cycle_ui_on_main(app: &tauri::AppHandle, payload: serde_json::Value) {
    let fallback = payload.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let outer = app.clone();
    let inner = outer.clone();
    let p = payload;
    match outer.run_on_main_thread(move || {
        let _ = inner.emit("profile-active-changed", p.clone());
        let _ = inner.emit("profile-pill-update", p);
        let _ = show_profile_pill(&inner);
        let _ = tx.send(());
    }) {
        Ok(()) => {
            let _ = rx.recv();
        }
        Err(_) => {
            let _ = app.emit("profile-active-changed", fallback.clone());
            let _ = app.emit("profile-pill-update", fallback);
            let _ = show_profile_pill(app);
        }
    }
}

fn position_palette(w: &tauri::WebviewWindow) -> Result<(), String> {
    if let Some(mon) = w.current_monitor().map_err(|e| e.to_string())? {
        let pos = mon.position();
        let size = mon.size();
        let scale = mon.scale_factor();
        let phys_w = (PALETTE_W * scale).round() as i32;
        let phys_h = (PALETTE_H * scale).round() as i32;
        let _ = w.set_size(tauri::PhysicalSize::new(phys_w as u32, phys_h as u32));
        let x = pos.x + (size.width as i32 - phys_w) / 2;
        let y = pos.y + (size.height as f32 * 0.1).round() as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
    Ok(())
}

fn toggle_palette(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.emit("palette-before-hide", ());
            std::thread::sleep(Duration::from_millis(160));
            let _ = w.hide();
        } else {
            position_palette(&w)?;
            let _ = w.show();
            let _ = w.set_focus();
            let _ = w.emit("palette-open", ());
        }
    }
    Ok(())
}

fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;
    Ok(())
}

fn simulate_copy() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('c'), Direction::Click).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;
    Ok(())
}

/// Lit le presse-papiers, simule Ctrl+C, compare avant/après, restaure l’ancien contenu.
/// Retourne `Some` seulement si le presse-papiers **a changé** après copie (copie réelle).
/// Si `before == after` (après trim), la copie n’a pas eu d’effet ou la sélection était identique au
/// presse-papiers : on retourne `None` pour ne pas envoyer un **vieux** presse-papiers (autre app, session précédente).
fn capture_ai_assist_selection(clipboard: &Clipboard) -> Option<String> {
    let before = clipboard.read_text().unwrap_or_default();
    if simulate_copy().is_err() {
        eprintln!("[Voxpill] Assist : échec simulation Ctrl+C");
        return None;
    }
    std::thread::sleep(Duration::from_millis(AI_ASSIST_POST_COPY_DELAY_MS));
    let mut after = clipboard.read_text().unwrap_or_default();
    let _ = clipboard.write_text(before.clone());

    let mut after_t = after.trim();
    let before_t = before.trim();
    if after_t == before_t {
        std::thread::sleep(Duration::from_millis(40));
        if simulate_copy().is_ok() {
            std::thread::sleep(Duration::from_millis(AI_ASSIST_POST_COPY_DELAY_MS));
            after = clipboard.read_text().unwrap_or_default();
            let _ = clipboard.write_text(before.clone());
            after_t = after.trim();
        }
    }

    if after_t.is_empty() {
        eprintln!(
            "[Voxpill] Assist sélection : presse-papiers vide après Ctrl+C — pas de contexte (rien à copier ?)"
        );
        return None;
    }
    if after_t == before_t {
        eprintln!(
            "[Voxpill] Assist sélection : presse-papiers inchangé après Ctrl+C — pas de contexte (copie sans effet ou sélection identique au presse-papiers ; évite un faux contexte issu d’une autre session)"
        );
        return None;
    }
    eprintln!(
        "[Voxpill] Assist sélection : capture OK (presse-papiers a changé, {} car.)",
        after.chars().count()
    );
    let mut s = after;
    if s.len() > AI_ASSIST_SELECTED_MAX_CHARS {
        eprintln!(
            "[Voxpill] Assist : texte sélectionné tronqué ({} → {} caractères)",
            s.len(),
            AI_ASSIST_SELECTED_MAX_CHARS
        );
        let mut t = s.chars().take(AI_ASSIST_SELECTED_MAX_CHARS).collect::<String>();
        t.push_str("\n… [truncated]");
        s = t;
    }
    Some(s)
}

fn whisper_should_use_gpu() -> bool {
    cfg!(feature = "gpu") && hardware::detect().nvidia_gpu
}

fn load_whisper_if_needed(app: &tauri::AppHandle, managed: &Arc<Managed>) -> Result<(), String> {
    let cfg = managed.config.lock().map_err(|e| e.to_string())?.clone();
    let tier = ModelTier::from_suggested_tier(&cfg.model_tier);
    let path = model::model_path_for(app, tier)?;
    if !path.exists() {
        return Err("Modèle absent — téléchargez-le depuis l'app.".to_string());
    }
    let use_gpu = whisper_should_use_gpu();
    let mut w = managed.whisper.lock().map_err(|e| e.to_string())?;
    let mut lp = managed.loaded_model_key.lock().map_err(|e| e.to_string())?;
    if let Some((ref p, g)) = lp.as_ref() {
        if w.is_some() && p == &path && *g == use_gpu {
            return Ok(());
        }
    }
    let ctx = whisper_engine::load_context(&path, use_gpu)?;
    eprintln!(
        "[Voxpill] Modèle Whisper chargé — {}",
        if use_gpu { "GPU (CUDA)" } else { "CPU" }
    );
    *w = Some(ctx);
    *lp = Some((path, use_gpu));
    Ok(())
}

fn thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(1)
        .clamp(1, 8)
}

/// Whisper local + traduction optionnelle (mêmes règles que la dictée).
fn transcribe_local_pcm_to_text(
    app: &tauri::AppHandle,
    managed: &Arc<Managed>,
    pcm_16k: &[f32],
    language: &str,
    translate_enabled: bool,
    translate_target: &str,
    decode_profile: &str,
) -> Result<(String, String), String> {
    load_whisper_if_needed(app, managed)?;
    let cfg = managed.config.lock().map_err(|e| e.to_string())?;
    let model_tier = ModelTier::from_suggested_tier(&cfg.model_tier);
    drop(cfg);
    let lang = match language {
        "auto" => None,
        l => Some(l),
    };
    let whisper_translate_en = translate_enabled
        && translate_target.eq_ignore_ascii_case("en")
        && model_tier != ModelTier::LargeV3Turbo;
    let (raw_text, whisper_detected_iso) = {
        let w = managed.whisper.lock().map_err(|e| e.to_string())?;
        let ctx = w.as_ref().ok_or_else(|| "Whisper non initialisé".to_string())?;
        whisper_engine::transcribe(
            ctx,
            pcm_16k,
            lang,
            thread_count(),
            whisper_translate_en,
            decode_profile,
        )?
    };
    let sec = pcm_16k.len() as f32 / 16_000.0;
    eprintln!(
        "[Voxpill] Transcription locale : {:.2}s audio → {} caractères",
        sec,
        raw_text.len()
    );
    if raw_text.is_empty() {
        return Ok((String::new(), raw_text));
    }
    let source_lang = if language == "auto" {
        whisper_detected_iso
            .clone()
            .unwrap_or_else(|| "en".to_string())
    } else {
        language.to_string()
    };
    let mut text = raw_text.clone();
    if translate_enabled && !whisper_translate_en {
        let tgt = translate_target.trim();
        if !tgt.is_empty() && source_lang.to_ascii_lowercase() != tgt.to_ascii_lowercase() {
            match translate::translate_mymemory(&raw_text, &source_lang, tgt) {
                Ok(t) => {
                    eprintln!(
                        "[Voxpill] Traduction {} → {} : {} caractères",
                        source_lang,
                        tgt,
                        t.len()
                    );
                    text = t;
                }
                Err(e) => eprintln!("[Voxpill] Traduction ignorée : {e}"),
            }
        }
    }
    Ok((text, raw_text))
}

/// Transcription (local ou cloud Groq) + fallback automatique local si cloud indisponible.
fn transcribe_pcm_to_text(
    app: &tauri::AppHandle,
    managed: &Arc<Managed>,
    pcm_16k: &[f32],
    language: &str,
    translate_enabled: bool,
    translate_target: &str,
    decode_profile: &str,
) -> Result<(String, String), String> {
    let cfg = managed.config.lock().map_err(|e| e.to_string())?.clone();
    if cfg.stt_mode != "cloud-groq" {
        return transcribe_local_pcm_to_text(
            app,
            managed,
            pcm_16k,
            language,
            translate_enabled,
            translate_target,
            decode_profile,
        );
    }

    let sec = pcm_16k.len() as f32 / 16_000.0;
    eprintln!(
        "[Voxpill] Cloud STT (Groq) : tentative ({:.2}s audio, modèle {})",
        sec, cfg.stt_cloud_model
    );
    match groq_stt::transcribe_pcm_16k_mono(
        &cfg.stt_groq_api_key,
        &cfg.stt_cloud_model,
        pcm_16k,
        language,
    ) {
        Ok((raw_text, detected_iso)) => {
            eprintln!(
                "[Voxpill] Cloud STT (Groq) : succès ({} caractères)",
                raw_text.len()
            );
            if raw_text.is_empty() {
                return Ok((String::new(), raw_text));
            }
            let source_lang = if language == "auto" {
                detected_iso.unwrap_or_else(|| "en".to_string())
            } else {
                language.to_string()
            };
            let mut text = raw_text.clone();
            if translate_enabled {
                let tgt = translate_target.trim();
                if !tgt.is_empty() && source_lang.to_ascii_lowercase() != tgt.to_ascii_lowercase() {
                    match translate::translate_mymemory(&raw_text, &source_lang, tgt) {
                        Ok(t) => {
                            eprintln!(
                                "[Voxpill] Traduction {} → {} : {} caractères",
                                source_lang,
                                tgt,
                                t.len()
                            );
                            text = t;
                        }
                        Err(e) => eprintln!("[Voxpill] Traduction ignorée : {e}"),
                    }
                }
            }
            Ok((text, raw_text))
        }
        Err(e) => {
            let msg = format!(
                "Cloud STT indisponible ({e}). Repli automatique sur Whisper local."
            );
            eprintln!("[Voxpill] {msg}");
            let _ = app.emit(
                "cloud-stt-status",
                serde_json::json!({ "level": "warn", "message": msg }),
            );
            transcribe_local_pcm_to_text(
                app,
                managed,
                pcm_16k,
                language,
                translate_enabled,
                translate_target,
                decode_profile,
            )
        }
    }
}

fn do_start_recording(
    managed: &Arc<Managed>,
    app: &tauri::AppHandle,
    clipboard: &Clipboard,
    mode: RecordingMode,
) -> Result<(), String> {
    if managed
        .transcription_pipeline_busy
        .load(Ordering::Acquire)
    {
        eprintln!("[Voxpill] Transcription ou collage encore en cours — démarrage dictée ignoré.");
        return Err("Transcription encore en cours.".to_string());
    }
    let mut g = managed.recording.lock().map_err(|e| e.to_string())?;
    if g.is_some() {
        return Ok(());
    }
    *managed.ai_assist_selected_text.lock().map_err(|e| e.to_string())? = None;
    *managed.recording_mode.lock().map_err(|e| e.to_string())? = mode;
    managed.mic_level.store(0, Ordering::Relaxed);
    managed.recording_active.store(true, Ordering::Release);

    // Fenêtre cible avant ouverture du micro (cpal peut prendre du temps ; le focus peut bouger).
    let focus_hwnd = foreground::capture_foreground_window();
    if let Some(hwnd) = focus_hwnd {
        *managed.paste_target_hwnd.lock().map_err(|e| e.to_string())? = Some(hwnd);
    } else if mode == RecordingMode::AiAssist {
        eprintln!(
            "[Voxpill] Assist : aucune fenêtre au premier plan — Ctrl+C et collage ciblés peuvent échouer"
        );
    }

    if mode == RecordingMode::AiAssist {
        if let Some(hwnd) = focus_hwnd {
            foreground::try_focus_window(hwnd);
            std::thread::sleep(Duration::from_millis(AI_ASSIST_FOCUS_SETTLE_MS));
        } else {
            eprintln!(
                "[Voxpill] Assist : impossible de restaurer le focus avant Ctrl+C (HWND absent)"
            );
        }
        if let Some(sel) = capture_ai_assist_selection(clipboard) {
            let n = sel.chars().count();
            let preview: String = sel.chars().take(160).collect::<String>();
            let one_line = preview.replace(['\r', '\n'], " ");
            eprintln!(
                "[Voxpill] Assist : contexte enregistré pour la session ({n} car.) — aperçu: {one_line}{}",
                if n > 160 { "…" } else { "" }
            );
            *managed.ai_assist_selected_text.lock().map_err(|e| e.to_string())? = Some(sel);
        } else {
            eprintln!("[Voxpill] Assist : aucun contexte sélectionné — le LLM ne recevra que la dictée");
        }
    }

    let session = match RecordingSession::new(managed.mic_level.clone()) {
        Ok(s) => s,
        Err(e) => {
            managed.recording_active.store(false, Ordering::Release);
            if let Ok(mut m) = managed.recording_mode.lock() {
                *m = RecordingMode::Normal;
            }
            if mode == RecordingMode::AiAssist {
                if let Ok(mut s) = managed.ai_assist_selected_text.lock() {
                    *s = None;
                }
            }
            if let Ok(mut h) = managed.paste_target_hwnd.lock() {
                *h = None;
            }
            return Err(e);
        }
    };
    *g = Some(session);
    let status_sounds_enabled = managed
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .status_sounds_enabled;
    emit_rec_hud_sound_enabled(app, status_sounds_enabled);
    show_rec_hud(app)?;
    emit_rec_hud_phase(app, "recording", mode);
    Ok(())
}

/// Premier appui sur le raccourci = démarrer, second appui = transcrire et insérer (bascule).
fn toggle_ptt(
    managed: &Arc<Managed>,
    app: &tauri::AppHandle,
    clipboard: &Clipboard,
) -> Result<(), String> {
    let recording = managed
        .recording
        .lock()
        .map_err(|e| e.to_string())?
        .is_some();
    if recording {
        let _ = do_stop_and_transcribe(managed, app, clipboard)?;
    } else {
        do_start_recording(managed, app, clipboard, RecordingMode::Normal)?;
    }
    Ok(())
}

fn toggle_ai_assist_ptt(
    managed: &Arc<Managed>,
    app: &tauri::AppHandle,
    clipboard: &Clipboard,
) -> Result<(), String> {
    let recording = managed
        .recording
        .lock()
        .map_err(|e| e.to_string())?
        .is_some();
    if recording {
        let _ = do_stop_and_transcribe(managed, app, clipboard)?;
    } else {
        do_start_recording(managed, app, clipboard, RecordingMode::AiAssist)?;
    }
    Ok(())
}

fn bind_ptt_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    shortcut: &str,
) -> Result<(), String> {
    let sc = shortcut.trim();
    if sc.is_empty() {
        return Err("Push-to-talk shortcut cannot be empty.".to_string());
    }
    app.global_shortcut()
        .on_shortcut(sc, move |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let clip = app.state::<Clipboard>();
            if let Err(e) = toggle_ptt(&managed, app, &clip) {
                eprintln!("Voxpill: {e}");
            }
        })
        .map_err(|e| e.to_string())
}

fn rebind_ptt_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    old_shortcut: &str,
    new_shortcut: &str,
) -> Result<(), String> {
    if !old_shortcut.trim().is_empty() {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }
    if let Err(e) = bind_ptt_shortcut(app, managed.clone(), new_shortcut) {
        if !old_shortcut.trim().is_empty() {
            let _ = bind_ptt_shortcut(app, managed, old_shortcut);
        }
        return Err(e);
    }
    Ok(())
}

fn bind_transcribe_file_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    shortcut: &str,
) -> Result<(), String> {
    let sc = shortcut.trim();
    if sc.is_empty() {
        return Err("Transcribe file shortcut cannot be empty.".to_string());
    }
    let m = managed.clone();
    app.global_shortcut()
        .on_shortcut(sc, move |app, _, event| {
            if event.state == ShortcutState::Pressed {
                let _ = toggle_file_pill(app, &m);
            }
        })
        .map_err(|e| e.to_string())
}

fn rebind_transcribe_file_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    old_shortcut: &str,
    new_shortcut: &str,
) -> Result<(), String> {
    if !old_shortcut.trim().is_empty() {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }
    if let Err(e) = bind_transcribe_file_shortcut(app, managed.clone(), new_shortcut) {
        if !old_shortcut.trim().is_empty() {
            let _ = bind_transcribe_file_shortcut(app, managed, old_shortcut);
        }
        return Err(e);
    }
    Ok(())
}

fn bind_cycle_profile_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    shortcut: &str,
) -> Result<(), String> {
    let sc = shortcut.trim();
    if sc.is_empty() {
        return Err("Profile cycle shortcut cannot be empty.".to_string());
    }
    let m = managed.clone();
    app.global_shortcut()
        .on_shortcut(sc, move |app, _, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = cycle_reformulation_profile(app, &m) {
                    eprintln!("Voxpill profile cycle: {e}");
                }
            }
        })
        .map_err(|e| e.to_string())
}

fn rebind_cycle_profile_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    old_shortcut: &str,
    new_shortcut: &str,
) -> Result<(), String> {
    if !old_shortcut.trim().is_empty() {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }
    if let Err(e) = bind_cycle_profile_shortcut(app, managed.clone(), new_shortcut) {
        if !old_shortcut.trim().is_empty() {
            let _ = bind_cycle_profile_shortcut(app, managed, old_shortcut);
        }
        return Err(e);
    }
    Ok(())
}

fn bind_ai_assist_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    shortcut: &str,
) -> Result<(), String> {
    let sc = shortcut.trim();
    if sc.is_empty() {
        return Err("AI assist shortcut cannot be empty.".to_string());
    }
    let m = managed.clone();
    app.global_shortcut()
        .on_shortcut(sc, move |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let clip = app.state::<Clipboard>();
            if let Err(e) = toggle_ai_assist_ptt(&m, app, &clip) {
                eprintln!("Voxpill AI assist: {e}");
            }
        })
        .map_err(|e| e.to_string())
}

fn rebind_ai_assist_shortcut(
    app: &tauri::AppHandle,
    managed: Arc<Managed>,
    old_shortcut: &str,
    new_shortcut: &str,
) -> Result<(), String> {
    if !old_shortcut.trim().is_empty() {
        let _ = app.global_shortcut().unregister(old_shortcut);
    }
    if let Err(e) = bind_ai_assist_shortcut(app, managed.clone(), new_shortcut) {
        if !old_shortcut.trim().is_empty() {
            let _ = bind_ai_assist_shortcut(app, managed, old_shortcut);
        }
        return Err(e);
    }
    Ok(())
}

/// Transcription, reformulation, collage — exécuté sur un thread dédié pour ne pas bloquer le raccourci global / menu tray.
fn run_stop_and_transcribe_job(
    managed: Arc<Managed>,
    app: tauri::AppHandle,
    samples: Vec<f32>,
    rate: u32,
    session_mode: RecordingMode,
) {
    struct ClearBusy(Arc<AtomicBool>);
    impl Drop for ClearBusy {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    let _clear_busy = ClearBusy(managed.transcription_pipeline_busy.clone());

    let clipboard = app.state::<Clipboard>();
    let run_inner = || -> Result<String, String> {
        let ai_selected = if session_mode == RecordingMode::AiAssist {
            managed
                .ai_assist_selected_text
                .lock()
                .map_err(|e| e.to_string())?
                .take()
        } else {
            None
        };
        let pcm = resample_to_16k_mono(&samples, rate);
        let cfg = managed.config.lock().map_err(|e| e.to_string())?.clone();
        let (decode_p, tr_en, tr_tgt) = active_profile_pipeline(&cfg);
        let model_tier = ModelTier::from_suggested_tier(&cfg.model_tier);
        let whisper_translate_en = tr_en
            && tr_tgt.eq_ignore_ascii_case("en")
            && model_tier != ModelTier::LargeV3Turbo;
        let (text, raw_text) = transcribe_pcm_to_text(
            &app,
            &managed,
            &pcm,
            &cfg.language,
            tr_en,
            &tr_tgt,
            &decode_p,
        )?;
        if raw_text.is_empty() {
            eprintln!("[Voxpill] Texte vide après Whisper — vérifiez le micro ou la langue.");
            return Ok(raw_text);
        }
        let translated_flag = tr_en && (whisper_translate_en || text != raw_text);
        let mut text = text;
        let mut reformulated = false;
        let params = reformulation_params_from_cfg(&cfg);
        let active_ok = active_profile_index(&cfg).is_some();
        if session_mode == RecordingMode::AiAssist {
            if active_ok && !params.api_key.trim().is_empty() {
                emit_rec_hud_phase_on_main(&app, "reformulating", session_mode);
                let sel = ai_selected.as_deref();
                if let Some(out) = reformulate::assist_from_instruction(&params, sel, &text) {
                    text = out;
                    reformulated = true;
                }
            }
        } else if params.enabled && !params.api_key.trim().is_empty() && active_ok {
            emit_rec_hud_phase_on_main(&app, "reformulating", session_mode);
            if let Some(rewritten) = reformulate::maybe_reformulate(&params, &text) {
                text = rewritten;
                reformulated = true;
            }
        }
        clipboard.write_text(text.clone()).map_err(|e: String| e)?;
        match cfg.insert_mode {
            InsertMode::Paste => {
                if let Ok(mut g) = managed.paste_target_hwnd.lock() {
                    if let Some(hwnd) = g.take() {
                        foreground::try_focus_window(hwnd);
                        std::thread::sleep(Duration::from_millis(80));
                    }
                }
                // Laisser le presse-papiers et le focus se stabiliser avant Ctrl+V.
                std::thread::sleep(Duration::from_millis(120));
                simulate_paste().map_err(|e| {
                    eprintln!("[Voxpill] Échec simulation Ctrl+V : {e}");
                    e
                })?;
                std::thread::sleep(Duration::from_millis(50));
            }
            InsertMode::Clipboard => {}
        }
        emit_transcript_on_main(
            &app,
            serde_json::json!({
                "text": text,
                "original": raw_text,
                "translated": translated_flag,
                "reformulated": reformulated,
            }),
        );
        Ok(text)
    };

    let result = run_inner();
    if let Err(ref e) = result {
        eprintln!("[Voxpill] Pipeline dictée : {e}");
    }

    if let Ok(mut m) = managed.recording_mode.lock() {
        *m = RecordingMode::Normal;
    }
    hide_rec_hud_on_main(&app);
    emit_rec_hud_phase_on_main(&app, "idle", RecordingMode::Normal);
}

fn do_stop_and_transcribe(
    managed: &Arc<Managed>,
    app: &tauri::AppHandle,
    _clipboard: &Clipboard,
) -> Result<String, String> {
    let (samples, rate) = {
        let mut g = managed.recording.lock().map_err(|e| e.to_string())?;
        let sess = g.take();
        match sess {
            None => {
                managed.recording_active.store(false, Ordering::Release);
                managed.mic_level.store(0, Ordering::Relaxed);
                if let Ok(mut h) = managed.paste_target_hwnd.lock() {
                    *h = None;
                }
                if let Ok(mut m) = managed.recording_mode.lock() {
                    *m = RecordingMode::Normal;
                }
                hide_rec_hud(app);
                emit_rec_hud_phase(app, "idle", RecordingMode::Normal);
                return Ok(String::new());
            }
            Some(s) => {
                let rate = s.sample_rate;
                let v = s.samples.lock().map_err(|e| e.to_string())?.clone();
                (v, rate)
            }
        }
    };
    let session_mode = *managed
        .recording_mode
        .lock()
        .map_err(|e| e.to_string())?;
    managed.recording_active.store(false, Ordering::Release);
    managed.mic_level.store(0, Ordering::Relaxed);
    // ~40 ms minimum (évite coupures trop agressives sur certains périphériques)
    let min_samples = (rate as usize / 25).max(320);
    if samples.len() < min_samples {
        eprintln!(
            "[Voxpill] Audio trop court : {} échantillons (< {}, ~{:.0} ms @ {} Hz)",
            samples.len(),
            min_samples,
            1000.0 * min_samples as f32 / rate as f32,
            rate
        );
        if let Ok(mut s) = managed.ai_assist_selected_text.lock() {
            *s = None;
        }
        if let Ok(mut m) = managed.recording_mode.lock() {
            *m = RecordingMode::Normal;
        }
        hide_rec_hud(app);
        emit_rec_hud_phase(app, "idle", RecordingMode::Normal);
        return Ok(String::new());
    }

    emit_rec_hud_phase(app, "transcribing", session_mode);
    let status_sounds_enabled = managed
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .status_sounds_enabled;
    emit_rec_hud_sound_enabled(app, status_sounds_enabled);

    managed
        .transcription_pipeline_busy
        .store(true, Ordering::Release);
    let managed_clone = managed.clone();
    let app_clone = app.clone();
    std::thread::spawn(move || {
        run_stop_and_transcribe_job(managed_clone, app_clone, samples, rate, session_mode);
    });

    Ok(String::new())
}

/// Annule une dictée en cours sans lancer de transcription ni collage.
fn do_cancel_recording(managed: &Arc<Managed>, app: &tauri::AppHandle) -> Result<(), String> {
    let mut g = managed.recording.lock().map_err(|e| e.to_string())?;
    if g.take().is_none() {
        return Ok(());
    }
    if let Ok(mut s) = managed.ai_assist_selected_text.lock() {
        *s = None;
    }
    managed.recording_active.store(false, Ordering::Release);
    managed.mic_level.store(0, Ordering::Relaxed);
    if let Ok(mut h) = managed.paste_target_hwnd.lock() {
        *h = None;
    }
    hide_rec_hud(app);
    emit_rec_hud_phase(app, "idle", RecordingMode::Normal);
    if let Ok(mut m) = managed.recording_mode.lock() {
        *m = RecordingMode::Normal;
    }
    Ok(())
}

#[tauri::command]
fn get_hardware_profile() -> hardware::HardwareProfile {
    hardware::detect()
}

#[tauri::command]
fn get_config(managed: tauri::State<'_, Arc<Managed>>) -> Result<AppConfig, String> {
    Ok(managed.config.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn set_config(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
    mut cfg: AppConfig,
) -> Result<(), String> {
    normalize_config(&mut cfg);
    downgrade_gpu_only_if_unavailable(&mut cfg);
    let mut c = managed.config.lock().map_err(|e| e.to_string())?;
    let old_ptt_shortcut = c.ptt_shortcut.clone();
    let old_shortcut_file = c.shortcut_transcribe_file.clone();
    let old_shortcut_cycle = c.shortcut_cycle_profile.clone();
    let old_shortcut_assist = c.shortcut_ai_assist.clone();
    let new_ptt_shortcut = cfg.ptt_shortcut.clone();
    let new_shortcut_file = cfg.shortcut_transcribe_file.clone();
    let new_shortcut_cycle = cfg.shortcut_cycle_profile.clone();
    let new_shortcut_assist = cfg.shortcut_ai_assist.clone();
    *c = cfg.clone();
    save_cfg(&app, &c)?;
    drop(c);
    let m = managed.inner().clone();
    if old_ptt_shortcut != new_ptt_shortcut {
        rebind_ptt_shortcut(&app, m.clone(), &old_ptt_shortcut, &new_ptt_shortcut)?;
    }
    if old_shortcut_file != new_shortcut_file {
        rebind_transcribe_file_shortcut(&app, m.clone(), &old_shortcut_file, &new_shortcut_file)?;
    }
    if old_shortcut_cycle != new_shortcut_cycle {
        rebind_cycle_profile_shortcut(&app, m.clone(), &old_shortcut_cycle, &new_shortcut_cycle)?;
    }
    if old_shortcut_assist != new_shortcut_assist {
        rebind_ai_assist_shortcut(&app, m, &old_shortcut_assist, &new_shortcut_assist)?;
    }
    let mut w = managed.whisper.lock().map_err(|e| e.to_string())?;
    *w = None;
    let mut lp = managed.loaded_model_key.lock().map_err(|e| e.to_string())?;
    *lp = None;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ModelStatus {
    tier: String,
    path: String,
    exists: bool,
    size_mb: u64,
}

#[tauri::command]
fn model_status(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
) -> Result<ModelStatus, String> {
    let cfg = managed.config.lock().map_err(|e| e.to_string())?;
    let tier = ModelTier::from_suggested_tier(&cfg.model_tier);
    let path = model::model_path_for(&app, tier)?;
    let exists = path.exists();
    let size_mb = if exists {
        std::fs::metadata(&path).map(|m| m.len() / 1024 / 1024).unwrap_or(0)
    } else {
        0
    };
    Ok(ModelStatus {
        tier: cfg.model_tier.clone(),
        path: path.to_string_lossy().to_string(),
        exists,
        size_mb,
    })
}

#[derive(Debug, Clone, Serialize)]
struct TierDiskStatus {
    tier: String,
    exists: bool,
    size_mb: u64,
    gpu_only: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ModelsOverview {
    selected_tier: String,
    tiers: Vec<TierDiskStatus>,
    whisper_gpu: bool,
}

/// État disque de chaque modèle + tier actuellement choisi dans les réglages.
#[tauri::command]
fn models_overview(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
) -> Result<ModelsOverview, String> {
    {
        let mut cfg = managed.config.lock().map_err(|e| e.to_string())?;
        let before = cfg.model_tier.clone();
        downgrade_gpu_only_if_unavailable(&mut cfg);
        if cfg.model_tier != before {
            save_cfg(&app, &cfg)?;
            let mut w = managed.whisper.lock().map_err(|e| e.to_string())?;
            *w = None;
            let mut lp = managed.loaded_model_key.lock().map_err(|e| e.to_string())?;
            *lp = None;
        }
    }
    let whisper_gpu = whisper_should_use_gpu();
    let selected_tier = managed
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .model_tier
        .clone();
    let mut tiers = Vec::new();
    for &tier in ModelTier::all_for_overview(whisper_gpu) {
        let path = model::model_path_for(&app, tier)?;
        let exists = path.exists();
        let size_mb = if exists {
            std::fs::metadata(&path)
                .map(|m| m.len() / 1024 / 1024)
                .unwrap_or(0)
        } else {
            0
        };
        tiers.push(TierDiskStatus {
            tier: tier.slug_str().to_string(),
            exists,
            size_mb,
            gpu_only: tier.gpu_only(),
        });
    }
    Ok(ModelsOverview {
        selected_tier,
        tiers,
        whisper_gpu,
    })
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
    tier: String,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let tier = ModelTier::from_suggested_tier(&tier);
    if tier.gpu_only() && !whisper_should_use_gpu() {
        return Err(
            "Ce modèle nécessite Whisper GPU (CUDA). Choisissez tiny / base / small, ou utilisez la build avec support GPU."
                .to_string(),
        );
    }
    let path = model::model_path_for(&app, tier)?;
    let url = tier.download_url();
    let tier_str = tier.slug_str();
    let _ = app.emit(
        "download-progress",
        serde_json::json!({ "pct": 0u32, "tier": tier_str, "indeterminate": false }),
    );
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = tokio::fs::File::create(&path).await.map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut pending_unknown = total == 0;
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if total > 0 {
            let pct = ((downloaded as f64 / total as f64) * 100.0).round() as u32;
            let _ = app.emit(
                "download-progress",
                serde_json::json!({ "pct": pct.min(100), "tier": tier_str, "indeterminate": false }),
            );
        } else if pending_unknown {
            pending_unknown = false;
            let _ = app.emit(
                "download-progress",
                serde_json::json!({ "pct": 0u32, "tier": tier_str, "indeterminate": true }),
            );
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    let _ = app.emit(
        "download-progress",
        serde_json::json!({ "pct": 100u32, "tier": tier_str, "indeterminate": false }),
    );
    {
        let mut w = managed.whisper.lock().map_err(|e| e.to_string())?;
        *w = None;
        let mut lp = managed.loaded_model_key.lock().map_err(|e| e.to_string())?;
        *lp = None;
        let mut c = managed.config.lock().map_err(|e| e.to_string())?;
        c.model_tier = tier.slug_str().to_string();
        save_cfg(&app, &c)?;
    }
    let size_mb = std::fs::metadata(&path)
        .map(|m| m.len() / 1024 / 1024)
        .unwrap_or(0);
    let payload = serde_json::json!({
        "tier": tier_str,
        "size_mb": size_mb,
    });
    let _ = app.emit("download-complete", payload.clone());
    let _ = app.emit("model-ready", payload);
    Ok(())
}

/// Supprime le fichier `.bin` du tier sur le disque. Si c’était le modèle sélectionné, repasse sur `tiny` et décharge Whisper.
fn do_transcribe_file(
    app: &tauri::AppHandle,
    managed: &Arc<Managed>,
    path: PathBuf,
    language: String,
    translate_enabled: bool,
    translate_target: String,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let on_cancel = |msg: &str| {
        emit_file_pill(app, "cancelled", msg);
    };
    if let Err(e) = check_file_transcribe_cancel(cancel) {
        on_cancel(&e);
        return Err(e);
    }
    emit_file_pill(app, "decode", "Décodage audio…");
    let (samples, rate) = audio_decode::decode_file_to_mono_f32(&path).map_err(|e| {
        emit_file_pill(app, "error", &e);
        e
    })?;
    if let Err(e) = check_file_transcribe_cancel(cancel) {
        on_cancel(&e);
        return Err(e);
    }
    let pcm = resample_to_16k_mono(&samples, rate);
    let min_samples = (16000 / 25).max(320);
    if pcm.len() < min_samples {
        let m = "Fichier trop court pour être transcrit.";
        emit_file_pill(app, "error", m);
        return Err(m.to_string());
    }
    emit_file_pill(app, "transcribe", "Transcription…");
    if let Err(e) = check_file_transcribe_cancel(cancel) {
        on_cancel(&e);
        return Err(e);
    }
    let tr_tgt = translate_target.trim().to_string();
    let (text, raw_text) = transcribe_pcm_to_text(
        app,
        managed,
        &pcm,
        &language,
        translate_enabled,
        &tr_tgt,
        FILE_TRANSCRIBE_DECODE_PROFILE,
    )
    .map_err(|e| {
        emit_file_pill(app, "error", &e);
        e
    })?;
    if let Err(e) = check_file_transcribe_cancel(cancel) {
        on_cancel(&e);
        return Err(e);
    }
    if raw_text.is_empty() {
        let m = "Transcription vide.";
        emit_file_pill(app, "error", m);
        return Err("Transcription vide — vérifiez la langue ou le contenu audio.".to_string());
    }
    let out_path = path.with_extension("txt");
    std::fs::write(&out_path, &text).map_err(|e| {
        let msg = format!("Écriture : {e}");
        emit_file_pill(app, "error", &msg);
        msg
    })?;
    let name = out_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sortie.txt".to_string());
    let done_msg = format!("OK : {name}");
    emit_file_pill(app, "done", &done_msg);
    eprintln!("[Voxpill] Transcription fichier → {}", out_path.display());
    Ok(out_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn transcribe_audio_file(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
    path: String,
    language: String,
    translate_enabled: bool,
    translate_target: String,
) -> Result<String, String> {
    managed
        .file_transcribe_cancel
        .store(false, Ordering::Release);
    let path = PathBuf::from(path);
    let app2 = app.clone();
    let managed2 = managed.inner().clone();
    let cancel = managed.file_transcribe_cancel.clone();
    let res = tokio::task::spawn_blocking(move || {
        do_transcribe_file(
            &app2,
            &managed2,
            path,
            language,
            translate_enabled,
            translate_target,
            &cancel,
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    res
}

#[tauri::command]
fn file_pill_set_interactive_rect(
    managed: tauri::State<'_, Arc<Managed>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if width <= 1.0 || height <= 1.0 {
        if let Ok(mut g) = managed.file_pill_interact_rect.lock() {
            *g = None;
        }
    } else if let Ok(mut g) = managed.file_pill_interact_rect.lock() {
        *g = Some((x, y, width, height));
    }
    Ok(())
}

#[tauri::command]
fn rec_hud_set_interactive_rect(
    managed: tauri::State<'_, Arc<Managed>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if width <= 1.0 || height <= 1.0 {
        if let Ok(mut g) = managed.rec_hud_interact_rect.lock() {
            *g = None;
        }
    } else if let Ok(mut g) = managed.rec_hud_interact_rect.lock() {
        *g = Some((x, y, width, height));
    }
    Ok(())
}

#[tauri::command]
fn delete_model(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
    tier: String,
) -> Result<(), String> {
    let tier = ModelTier::from_suggested_tier(&tier);
    let path = model::model_path_for(&app, tier)?;
    if !path.exists() {
        return Err("Ce modèle n’est pas présent sur ce PC.".to_string());
    }
    std::fs::remove_file(&path).map_err(|e| format!("Suppression impossible : {e}"))?;
    eprintln!("[Voxpill] Modèle supprimé : {}", path.display());
    let tier_slug = tier.slug_str();
    let mut c = managed.config.lock().map_err(|e| e.to_string())?;
    if c.model_tier == tier_slug {
        c.model_tier = "tiny".to_string();
        save_cfg(&app, &c)?;
        drop(c);
        let mut w = managed.whisper.lock().map_err(|e| e.to_string())?;
        *w = None;
        let mut lp = managed.loaded_model_key.lock().map_err(|e| e.to_string())?;
        *lp = None;
    }
    Ok(())
}

#[tauri::command]
fn start_recording(
    managed: tauri::State<'_, Arc<Managed>>,
    app: tauri::AppHandle,
    clipboard: tauri::State<'_, Clipboard>,
) -> Result<(), String> {
    do_start_recording(&managed, &app, &clipboard, RecordingMode::Normal)
}

#[tauri::command]
fn stop_and_transcribe(
    app: tauri::AppHandle,
    managed: tauri::State<'_, Arc<Managed>>,
    clipboard: tauri::State<'_, Clipboard>,
) -> Result<String, String> {
    do_stop_and_transcribe(&managed, &app, &clipboard)
}

#[derive(Debug, Clone, Deserialize)]
struct PromptAssistInput {
    provider: String,
    api_key: String,
    draft: String,
    model_hint: String,
}

#[tauri::command]
fn assist_reformulation_prompt(input: PromptAssistInput) -> Result<String, String> {
    reformulate::assist_prompt(&input.provider, &input.api_key, &input.draft, &input.model_hint)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let cfg = load_cfg(&handle)?;
            let managed = Arc::new(Managed {
                config: Mutex::new(cfg),
                recording: Mutex::new(None),
                whisper: Mutex::new(None),
                loaded_model_key: Mutex::new(None),
                mic_level: Arc::new(AtomicU32::new(0)),
                recording_active: Arc::new(AtomicBool::new(false)),
                paste_target_hwnd: Mutex::new(None),
                recording_mode: Mutex::new(RecordingMode::Normal),
                ai_assist_selected_text: Mutex::new(None),
                file_pill_interact_rect: Mutex::new(None),
                rec_hud_interact_rect: Mutex::new(None),
                file_transcribe_cancel: Arc::new(AtomicBool::new(false)),
                transcription_pipeline_busy: Arc::new(AtomicBool::new(false)),
            });
            app.manage(managed.clone());
            let poll_app = handle.clone();
            let poll_mic = managed.mic_level.clone();
            let poll_on = managed.recording_active.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(32));
                if !poll_on.load(Ordering::Relaxed) {
                    continue;
                }
                let v = poll_mic.load(Ordering::Relaxed);
                let _ = poll_app.emit("mic-level", serde_json::json!({ "v": v }));
            });
            let hit_app = handle.clone();
            let hit_m = managed.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(32));
                file_pill_update_cursor_pass_through(&hit_app, &hit_m);
                rec_hud_update_cursor_pass_through(&hit_app, &hit_m);
            });
            let _ = handle.autolaunch().enable();
            let open_i = MenuItem::with_id(&handle, "open", "Settings", true, None::<&str>)?;
            let transcribe_i = MenuItem::with_id(
                &handle,
                "transcribe_file",
                "Transcribe file…",
                true,
                None::<&str>,
            )?;
            let quit_i = MenuItem::with_id(&handle, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(&handle, &[&open_i, &transcribe_i, &quit_i])?;
            let icon_bytes: &[u8] = include_bytes!("../icons/32x32.png");
            let icon = Image::from_bytes(icon_bytes).map_err(|e| e.to_string())?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .tooltip("Voxpill — local dictation")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event: MenuEvent| {
                    if event.id == "open" {
                        let _ = toggle_palette(app);
                    } else if event.id == "transcribe_file" {
                        let _ = show_file_pill(app);
                    } else if event.id == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button,
                        button_state,
                        ..
                    } = event
                    {
                        if button == MouseButton::Left && button_state == MouseButtonState::Up {
                            let _ = toggle_palette(tray.app_handle());
                        }
                    }
                })
                .build(&handle)?;
            let ptt_shortcut = managed
                .config
                .lock()
                .map_err(|e| e.to_string())?
                .ptt_shortcut
                .clone();
            bind_ptt_shortcut(&handle, managed.clone(), &ptt_shortcut)?;
            let shortcut_file = managed
                .config
                .lock()
                .map_err(|e| e.to_string())?
                .shortcut_transcribe_file
                .clone();
            bind_transcribe_file_shortcut(&handle, managed.clone(), &shortcut_file)?;
            let m_cancel = managed.clone();
            handle
                .global_shortcut()
                .on_shortcut("Esc", move |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = do_cancel_recording(&m_cancel, app);
                        if let Some(w) = app.get_webview_window("file-pill") {
                            if w.is_visible().unwrap_or(false) {
                                m_cancel
                                    .file_transcribe_cancel
                                    .store(true, Ordering::Release);
                                let _ = w.hide();
                            }
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            let shortcut_cycle = managed
                .config
                .lock()
                .map_err(|e| e.to_string())?
                .shortcut_cycle_profile
                .clone();
            bind_cycle_profile_shortcut(&handle, managed.clone(), &shortcut_cycle)?;
            let shortcut_assist = managed
                .config
                .lock()
                .map_err(|e| e.to_string())?
                .shortcut_ai_assist
                .clone();
            bind_ai_assist_shortcut(&handle, managed.clone(), &shortcut_assist)?;
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = w.hide();
                let _ = position_palette(&w);
            }
            if let Some(h) = handle.get_webview_window("rec-hud") {
                let _ = h.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = h.set_focusable(false);
                let _ = h.hide();
                let _ = position_rec_hud(&h);
            }
            if let Some(fp) = handle.get_webview_window("file-pill") {
                let _ = fp.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = fp.hide();
                let _ = position_file_pill(&fp);
            }
            if let Some(pp) = handle.get_webview_window("profile-pill") {
                let _ = pp.set_background_color(Some(Color(0, 0, 0, 0)));
                let _ = pp.set_focusable(false);
                let _ = pp.hide();
                let _ = position_profile_pill(&pp);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_hardware_profile,
            get_config,
            set_config,
            model_status,
            models_overview,
            download_model,
            delete_model,
            transcribe_audio_file,
            file_pill_set_interactive_rect,
            rec_hud_set_interactive_rect,
            start_recording,
            stop_and_transcribe,
            assist_reformulation_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("Voxpill");
}
