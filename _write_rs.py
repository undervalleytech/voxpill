from pathlib import Path
base = Path(r"E:/Développement/voxpill/src-tauri/src")
base.mkdir(parents=True, exist_ok=True)

hardware = r'''
use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct HardwareProfile {
    pub total_ram_gb: u32,
    pub logical_cpus: u32,
    pub suggested_tier: String,
    pub nvidia_gpu: bool,
}

pub fn detect() -> HardwareProfile {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_ram_gb = (sys.total_memory() / 1024 / 1024 / 1024).max(1) as u32;
    let logical_cpus = sys.cpus().len().max(1) as u32;
    let nvidia_gpu = nvidia_smi_ok();
    let suggested_tier = if total_ram_gb < 8 {
        "tiny".to_string()
    } else if total_ram_gb < 16 && !nvidia_gpu {
        "base".to_string()
    } else if nvidia_gpu {
        "small".to_string()
    } else {
        "base".to_string()
    };
    HardwareProfile {
        total_ram_gb,
        logical_cpus,
        suggested_tier,
        nvidia_gpu,
    }
}

fn nvidia_smi_ok() -> bool {
    std::process::Command::new("nvidia-smi")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
'''
(base / "hardware.rs").write_text(hardware.strip() + "\n", encoding="utf-8")
print("hardware ok")
