Ce dossier reçoit **automatiquement** les DLL runtime CUDA (`cudart`, `cublas`, etc.) depuis `%CUDA_PATH%\bin` au moment du `cargo build` avec la fonctionnalité **`gpu`** (défaut).

Les fichiers `*.dll` sont ignorés par Git ; ils sont régénérés à chaque build release. L’installeur NSIS les place **à côté de `voxpill.exe`** pour que l’app démarre sans installer le CUDA Toolkit sur la machine cible.
