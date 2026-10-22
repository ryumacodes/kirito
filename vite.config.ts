import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    strictPort: true,
    port: 1420,
    watch: {
      ignored: [
        "**/.tmp/**",
        "**/Bottles/**",
        "**/Logs/**",
        "**/Installers/**",
        "**/src-tauri/target/**"
      ]
    }
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    minify: "esbuild"
  }
});
