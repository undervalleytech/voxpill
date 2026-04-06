use tauri::Manager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Tiny,
    Base,
    Small,
    Medium,
    /// Nécessite Whisper GPU (CUDA) — absent de l’UI sans GPU.
    #[serde(rename = "large-v3-turbo")]
    LargeV3Turbo,
}

impl ModelTier {
    pub fn file_name(self) -> &'static str {
        match self {
            ModelTier::Tiny => "ggml-tiny.bin",
            ModelTier::Base => "ggml-base.bin",
            ModelTier::Small => "ggml-small.bin",
            ModelTier::Medium => "ggml-medium.bin",
            ModelTier::LargeV3Turbo => "ggml-large-v3-turbo.bin",
        }
    }

    pub fn download_url(self) -> &'static str {
        match self {
            ModelTier::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            ModelTier::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            ModelTier::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            ModelTier::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            ModelTier::LargeV3Turbo => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
            }
        }
    }

    /// Slug stable pour JSON (`settings.json`, événements, UI).
    pub fn slug_str(self) -> &'static str {
        match self {
            ModelTier::Tiny => "tiny",
            ModelTier::Base => "base",
            ModelTier::Small => "small",
            ModelTier::Medium => "medium",
            ModelTier::LargeV3Turbo => "large-v3-turbo",
        }
    }

    /// Tiers lourds réservés au parcours GPU (démo / machines équipées) — sinon repli CPU + petits modèles.
    pub fn gpu_only(self) -> bool {
        matches!(self, ModelTier::Medium | ModelTier::LargeV3Turbo)
    }

    pub fn from_suggested_tier(s: &str) -> Self {
        let n = s.trim().to_ascii_lowercase().replace('_', "-");
        match n.as_str() {
            "small" => ModelTier::Small,
            "base" => ModelTier::Base,
            "medium" => ModelTier::Medium,
            "large-v3-turbo" => ModelTier::LargeV3Turbo,
            _ => ModelTier::Tiny,
        }
    }

    /// Tiers énumérés pour l’UI : toujours tiny/base/small ; medium + large-v3-turbo seulement si `include_gpu_tiers`.
    pub fn all_for_overview(include_gpu_tiers: bool) -> &'static [ModelTier] {
        if include_gpu_tiers {
            &[
                ModelTier::Tiny,
                ModelTier::Base,
                ModelTier::Small,
                ModelTier::Medium,
                ModelTier::LargeV3Turbo,
            ]
        } else {
            &[ModelTier::Tiny, ModelTier::Base, ModelTier::Small]
        }
    }
}

pub fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = base.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn model_path_for(app: &tauri::AppHandle, tier: ModelTier) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(tier.file_name()))
}
