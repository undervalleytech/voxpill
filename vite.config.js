import { defineConfig } from "vite";
import { resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: "src",
  /** Dossier `public/` à la racine du projet (sons optionnels pour le HUD). */
  publicDir: "../public",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "src/index.html"),
        hud: resolve(__dirname, "src/hud.html"),
        "file-pill": resolve(__dirname, "src/file-pill.html"),
        "profile-pill": resolve(__dirname, "src/profile-pill.html"),
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});
