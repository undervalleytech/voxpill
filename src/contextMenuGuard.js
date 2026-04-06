/**
 * Build release uniquement : pas de menu contextuel (clic droit).
 * En `tauri dev`, Vite met import.meta.env.PROD à false → inspection / DevTools possibles.
 */
if (import.meta.env.PROD) {
  window.addEventListener(
    "contextmenu",
    (e) => {
      e.preventDefault();
    },
    true,
  );
}
