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

fn cuda_bin_search_dirs(cuda_root: &Path) -> Vec<PathBuf> {
    let bin = cuda_root.join("bin");
    let mut dirs = Vec::new();
    if bin.is_dir() {
        dirs.push(bin.clone());
        // Certains kits CUDA 12+/13 placent les DLL x64 uniquement dans bin\x64
        let x64 = bin.join("x64");
        if x64.is_dir() {
            dirs.push(x64);
        }
    }
    dirs
}

fn copy_cuda_runtime_dlls() -> Result<(), String> {
    let cuda_root = std::env::var("CUDA_PATH")
        .or_else(|_| std::env::var("CUDA_ROOT"))
        .map_err(|_| {
            "CUDA_PATH (ou CUDA_ROOT) doit pointer vers le répertoire d'installation CUDA \
             pour embarquer les DLL runtime dans l'installeur (ex. C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.x)"
                .to_string()
        })?;
    let search_dirs = cuda_bin_search_dirs(Path::new(&cuda_root));
    if search_dirs.is_empty() {
        return Err(format!(
            "Répertoire CUDA bin introuvable sous {}",
            cuda_root
        ));
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
    let mut seen_names = std::collections::HashSet::<String>::new();

    for bin in &search_dirs {
        for entry in std::fs::read_dir(bin).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".dll") && !name.ends_with(".DLL") {
                continue;
            }
            if !should_bundle_cuda_dll(name) {
                continue;
            }
            // bin\ puis bin\x64 : éviter d'écraser deux fois le même nom
            if !seen_names.insert(name.to_string()) {
                continue;
            }
            let dest = out.join(name);
            std::fs::copy(&path, &dest).map_err(|e| {
                format!("copie {} -> {}: {}", path.display(), dest.display(), e)
            })?;
            copied += 1;
        }
    }

    if copied == 0 {
        let mut listing = String::new();
        for bin in search_dirs.iter().take(2) {
            listing.push_str(&format!("\n--- {} ---\n", bin.display()));
            if let Ok(entries) = std::fs::read_dir(bin) {
                let mut names: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.to_ascii_lowercase().ends_with(".dll"))
                    .collect();
                names.sort();
                for n in names.iter().take(50) {
                    listing.push_str(n);
                    listing.push('\n');
                }
                if names.len() > 50 {
                    listing.push_str("... (tronque)\n");
                }
            }
        }
        return Err(format!(
            "Aucune DLL CUDA runtime reconnue sous CUDA_PATH (sous-dossiers bin/ et bin/x64 testés). \
             Filtre: nom contient (cudart|cublas|nvrtc|cudnn|cufft|curand|nvjitlink). \
             Liste des .dll trouvés (echantillon):{}",
            listing
        ));
    }

    eprintln!("[voxpill build] {copied} DLL CUDA copiées vers cuda-redist/ pour le bundle NSIS.");
    Ok(())
}

/// Correspondance large : CUDA 11–13 n’utilisent pas tous le préfixe cublas64_* dans bin/.
fn should_bundle_cuda_dll(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if !n.ends_with(".dll") {
        return false;
    }
    // Exclure des outils IDE / gros binaires non nécessaires au runtime Whisper
    if n.contains("nsight") || n.contains("nvvp") || n.contains("compute-sanitizer") {
        return false;
    }
    [
        "cudart", "cublas", "nvrtc", "cudnn", "cufft", "curand", "nvjitlink",
    ]
    .iter()
    .any(|k| n.contains(k))
}
