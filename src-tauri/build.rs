//! Copie les DLL runtime CUDA dans `cuda-redist/` pour le bundle NSIS (à côté de voxpill.exe).
//! Sans elles, Windows affiche « cublas64_*.dll introuvable » sur les PC sans CUDA Toolkit.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_ROOT");

    if std::env::var_os("CARGO_FEATURE_GPU").is_some() {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "windows" {
            copy_cuda_runtime_dlls().unwrap_or_else(|e| panic!("{e}"));
        }
    }

    tauri_build::build()
}

fn copy_cuda_runtime_dlls() -> Result<(), String> {
    let cuda_root = std::env::var("CUDA_PATH")
        .or_else(|_| std::env::var("CUDA_ROOT"))
        .map_err(|_| {
            "CUDA_PATH (ou CUDA_ROOT) doit pointer vers le répertoire d'installation CUDA \
             pour embarquer les DLL runtime dans l'installeur (ex. C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.x)"
                .to_string()
        })?;
    let bin = Path::new(&cuda_root).join("bin");
    if !bin.is_dir() {
        return Err(format!("Répertoire CUDA introuvable: {}", bin.display()));
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = manifest_dir.join("cuda-redist");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    for entry in std::fs::read_dir(&out).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")) {
            let _ = std::fs::remove_file(&p);
        }
    }

    let mut copied = 0usize;
    for entry in std::fs::read_dir(&bin).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".dll") && !name.ends_with(".DLL") {
            continue;
        }
        if should_bundle_cuda_dll(name) {
            let dest = out.join(name);
            std::fs::copy(&path, &dest).map_err(|e| {
                format!("copie {} -> {}: {}", path.display(), dest.display(), e)
            })?;
            copied += 1;
        }
    }

    if copied == 0 {
        return Err(format!(
            "Aucune DLL CUDA runtime reconnue dans {} (attendu: cudart64_*, cublas64_*, cublasLt64_*, nvrtc64_*, cudnn64_*)",
            bin.display()
        ));
    }

    eprintln!("[voxpill build] {copied} DLL CUDA copiées vers cuda-redist/ pour le bundle NSIS.");
    Ok(())
}

fn should_bundle_cuda_dll(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("cudart64_")
        || n.starts_with("cublas64_")
        || n.starts_with("cublaslt64_")
        || n.starts_with("nvrtc64_")
        || n.starts_with("cudnn")
}
