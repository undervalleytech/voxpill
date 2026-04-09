//! Résout le dossier `cuda-redist` et le préfixe dans `PATH` pour que le
//! chargeur Windows trouve les DLL CUDA lors du `LoadLibrary` implicite.

#[cfg(all(windows, feature = "gpu"))]
pub fn prepend_cuda_dll_path(app: &tauri::App) {
    use std::path::PathBuf;
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    let mut candidates: Vec<PathBuf> = vec![];

    // 1. Ressources Tauri (install NSIS via cuda-bundle.json)
    if let Ok(p) = app.path().resolve("cuda-redist", BaseDirectory::Resource) {
        candidates.push(p);
    }
    // 2. Racine $RESOURCE (bundler ayant aplati les fichiers)
    if let Ok(p) = app.path().resolve("", BaseDirectory::Resource) {
        candidates.push(p);
    }
    // 3. Portable : exe_dir/cuda-redist
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("cuda-redist"));
        }
    }
    // 4. Debug : CARGO_MANIFEST_DIR/cuda-redist (n'existe pas chez les utilisateurs)
    #[cfg(debug_assertions)]
    {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cuda-redist");
        candidates.push(manifest);
    }

    for dir in candidates {
        if !dir.is_dir() {
            continue;
        }
        let has_cublas = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .any(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                n.starts_with("cublas64_") && n.ends_with(".dll")
            });
        if !has_cublas {
            continue;
        }
        let old = std::env::var("PATH").unwrap_or_default();
        let dir_str = dir.to_string_lossy();
        if old.split(';').any(|seg| seg.eq_ignore_ascii_case(&dir_str)) {
            eprintln!("[Voxpill] CUDA DLL path already in PATH: {}", dir.display());
            return;
        }
        std::env::set_var("PATH", format!("{};{}", dir.display(), old));
        eprintln!(
            "[Voxpill] CUDA DLL path prepended to PATH: {}",
            dir.display()
        );
        return;
    }

    eprintln!(
        "[Voxpill] No cuda-redist directory with cublas64_*.dll found; GPU may fall back to CPU."
    );
}

#[cfg(not(all(windows, feature = "gpu")))]
pub fn prepend_cuda_dll_path(_app: &tauri::App) {}
