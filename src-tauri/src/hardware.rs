use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareProfile {
    pub total_ram_gb: u32,
    pub logical_cpus: u32,
    pub suggested_tier: String,
    pub nvidia_gpu: bool,
    /// Whisper utilisera le GPU si binaire compilé avec la fonctionnalité `gpu` + NVIDIA détectée.
    pub whisper_gpu: bool,
}

pub fn detect() -> HardwareProfile {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_ram_gb = (sys.total_memory() / 1024 / 1024 / 1024).max(1) as u32;
    let logical_cpus = sys.cpus().len().max(1) as u32;
    let nvidia_gpu = nvidia_smi_ok();
    let whisper_gpu = cfg!(feature = "gpu") && nvidia_gpu;
    // Heuristique conservatrice pour le grand public : pas de référence à une machine de test précise.
    // GPU + RAM : proposer des tiers lourds ; sinon petits modèles / CPU uniquement.
    let suggested_tier = if total_ram_gb < 8 {
        "tiny".to_string()
    } else if whisper_gpu {
        if total_ram_gb >= 16 {
            "large-v3-turbo".to_string()
        } else {
            "medium".to_string()
        }
    } else if total_ram_gb < 16 {
        "base".to_string()
    } else {
        "small".to_string()
    };
    HardwareProfile {
        total_ram_gb,
        logical_cpus,
        suggested_tier,
        nvidia_gpu,
        whisper_gpu,
    }
}

fn nvidia_smi_ok() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// Évite un flash de console (conhost) depuis une app GUI release.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("nvidia-smi")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("nvidia-smi")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
