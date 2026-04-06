# Voxpill

Dictée vocale **100 % locale** (Whisper / whisper.cpp via [whisper-rs](https://docs.rs/whisper-rs)). Fenêtre type palette, style proche de ClipPill (sombre, Geist, animations courtes).

## Prérequis

- Rust (stable), Node.js 18+, npm  
- CMake (pour compiler whisper.cpp) — en général déjà présent si Visual Studio Build Tools est installé.

## Développement

```bash
cd E:\Développement\voxpill
npm install
npm run tauri:dev
```

## Build (installeur NSIS)

```bash
npm run tauri:build
```

Installeur : `src-tauri\target\release\bundle\nsis\Voxpill_<version>_x64-setup.exe`

## Raccourcis

- **Ctrl+Shift+Espace** : **bascule** — 1er appui pour démarrer l’enregistrement (petite pilule avec **visualisation du micro** en bas de l’écran), 2e appui pour transcrire et coller au curseur (mode « Coller ») ou seulement presse-papiers.
- **Ctrl+Shift+V** : afficher / masquer la palette.

## Données

Modèles GGML et `settings.json` : répertoire données applicatives Tauri (`com.vnzbe.voxpill`).

Premier usage : télécharger le modèle recommandé depuis l’interface (bouton).
